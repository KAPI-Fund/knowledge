use std::collections::BTreeSet;
use std::fs;
use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use knowledge_core::project::root::ProjectRoot;

use crate::app::state::AppState;
use crate::projects::file_history;
use crate::providers::{OpenAiCompatibleProvider, ProviderTextRequest};

use super::cancel::AgentCancellationToken;
use super::context::{
    AgentContextInput, AgentConversationMessage, BuiltAgentContext, build_agent_context,
    collapse_whitespace, load_explicit_context_files, load_project_context, trim_chars,
};
use super::events::AgentEvent;
use super::permissions::{AgentCapability, PermissionPolicy};
use super::skills::AgentSkill;
use super::tools::{self, ToolContext};
use super::types::{
    AgentMode, AgentReference, AgentSkillMode, AgentUserInputField, AgentUserInputOption,
    AgentUserInputRequest,
};

// These limits are enforced in the backend Agent rather than the admin UI.
// API callers bypass the UI, so safety and cost boundaries must live here.
const DEFAULT_CHAT_SEARCH_RESULTS: usize = 5;
const MAX_CHAT_SEARCH_RESULTS: usize = 10;
const MAX_AGENT_TOOL_ITERATIONS: usize = 8;
const AGENT_STRUCTURED_MAX_TOKENS: u32 = 8192;
const AGENT_SKILL_STRUCTURED_MAX_TOKENS: u32 = 16384;
const MAX_SKILL_REFERENCE_BYTES: u64 = 256 * 1024;
const MAX_USER_INPUT_FIELDS: usize = 12;
const MAX_USER_INPUT_OPTIONS: usize = 8;
const MAX_USER_INPUT_TEXT_CHARS: usize = 400;

pub type AgentEventSink = Arc<dyn Fn(AgentEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct AgentLoopRequest {
    pub query: String,
    pub session_id: String,
    pub mode: AgentMode,
    pub skill_mode: AgentSkillMode,
    pub web_enabled: bool,
    pub history: Vec<AgentConversationMessage>,
    pub skills: Vec<AgentSkill>,
    pub context_files: Vec<String>,
    // Session whitelist for shell.exec, carried by the client on every agent
    // request (matching upstream). Commands are approved through the
    // userInputRequired confirm flow and re-sent on resume.
    pub approved_shell_commands: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AgentLoopOutcome {
    pub message: String,
    pub references: Vec<AgentReference>,
    pub events: Vec<AgentEvent>,
    pub user_input_request: Option<AgentUserInputRequest>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentLoopAction {
    #[serde(default)]
    action: String,
    #[serde(default)]
    tool: Option<String>,
    #[serde(default)]
    answer: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    skill: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    allow_overwrite: Option<bool>,
    #[serde(default)]
    include_content: Option<bool>,
    #[serde(default)]
    top_k: Option<usize>,
    #[serde(default)]
    fields: Option<Value>,
    #[serde(default)]
    questions: Option<Value>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
struct AgentObservation {
    tool: String,
    summary: String,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub async fn run_agent_loop(
    state: &AppState,
    project_id: &str,
    project_root: &ProjectRoot,
    provider: &OpenAiCompatibleProvider,
    permission_policy: &PermissionPolicy,
    request: &AgentLoopRequest,
    event_sink: Option<AgentEventSink>,
    cancellation: Option<&AgentCancellationToken>,
) -> Result<AgentLoopOutcome, String> {
    let session_id = request.session_id.clone();
    let mut events = Vec::<AgentEvent>::new();
    let mut references = Vec::<AgentReference>::new();
    emit_event(
        &mut events,
        &event_sink,
        AgentEvent::AgentStart {
            session_id: session_id.clone(),
        },
    );
    emit_event(
        &mut events,
        &event_sink,
        AgentEvent::TurnStart {
            mode: request.mode.label().to_string(),
        },
    );

    let project_path = project_root.as_path();
    let project_context = load_project_context(project_path);
    let explicit_files = load_explicit_context_files(project_path, &request.context_files).await;
    if !request.context_files.is_empty() {
        let detail = format!(
            "{} of {} selected file(s) attached",
            explicit_files.len(),
            request.context_files.len().min(8)
        );
        emit_event(
            &mut events,
            &event_sink,
            AgentEvent::tool_end("context.attach", Some(detail)),
        );
    }

    let skills = request.skills.as_slice();
    let tool_context = ToolContext {
        state,
        project_id,
        project_root,
    };
    let mut observations = Vec::<AgentObservation>::new();
    let mut executed_retrievals = BTreeSet::<String>::new();
    let mut retrieval_steps = 0usize;
    let mut force_final_next = false;
    let has_explicit_skills = request.skill_mode == AgentSkillMode::Explicit && !skills.is_empty();
    let max_iterations = agent_loop_iteration_budget(request.mode, !skills.is_empty());
    let retrieval_budget = agent_loop_retrieval_budget(request.mode, has_explicit_skills);
    let max_tokens = agent_structured_max_tokens(!skills.is_empty());

    for iteration in 0..max_iterations {
        check_cancel(cancellation)?;
        let must_finalize = force_final_next || retrieval_steps >= retrieval_budget;
        let built_context = fit_context_to_model(
            build_agent_context(AgentContextInput {
                query: &request.query,
                project: &project_context,
                web_enabled: request.web_enabled,
                history: &request.history,
                skills,
                skill_mode: request.skill_mode,
                references: &references,
                // Loop observations are appended below in the loop-specific
                // user block. Passing them through the generic retrieval
                // summary as well would duplicate large tool outputs on
                // every iteration.
                retrieval_summary: "",
                explicit_files: &explicit_files,
            }),
            None,
        );
        let (system, user) = if must_finalize {
            (
                build_agent_final_system(&built_context.system),
                build_agent_final_user(&built_context.user, &observations),
            )
        } else {
            (
                build_agent_loop_system(&built_context.system),
                build_agent_loop_user(
                    &built_context.user,
                    request,
                    skills,
                    &observations,
                    iteration,
                    max_iterations,
                    references
                        .iter()
                        .any(|reference| reference.kind == "workspace"),
                ),
            )
        };
        let raw =
            match generate_with_cancellation(provider, &system, &user, max_tokens, cancellation)
                .await
            {
                Ok(raw) => raw,
                Err(err) => {
                    emit_event(
                        &mut events,
                        &event_sink,
                        AgentEvent::Error {
                            message: err.clone(),
                        },
                    );
                    return Err(err);
                }
            };
        let action = parse_agent_loop_action(&raw);

        if must_finalize {
            let answer = forced_final_answer(&raw, &action, &references);
            if event_sink.is_some() {
                emit_event(
                    &mut events,
                    &event_sink,
                    AgentEvent::MessageDelta {
                        text: answer.clone(),
                    },
                );
            }
            emit_event(
                &mut events,
                &event_sink,
                AgentEvent::Done {
                    session_id: session_id.clone(),
                },
            );
            return Ok(AgentLoopOutcome {
                message: answer,
                references,
                events,
                user_input_request: None,
            });
        }

        if action.action.eq_ignore_ascii_case("invalid_tool_json") {
            observations.push(record_loop_tool_rejection(
                "agent.action",
                action.answer.unwrap_or_else(|| {
                    "Invalid tool JSON. Return a corrected compact JSON action.".to_string()
                }),
                &mut events,
                &event_sink,
            ));
            continue;
        }

        if action.action.eq_ignore_ascii_case("final") {
            let answer = action
                .answer
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| raw.trim().to_string());
            if event_sink.is_some() {
                emit_event(
                    &mut events,
                    &event_sink,
                    AgentEvent::MessageDelta {
                        text: answer.clone(),
                    },
                );
            }
            emit_event(
                &mut events,
                &event_sink,
                AgentEvent::Done {
                    session_id: session_id.clone(),
                },
            );
            return Ok(AgentLoopOutcome {
                message: answer,
                references,
                events,
                user_input_request: None,
            });
        }

        if !action.action.eq_ignore_ascii_case("tool") {
            // Some smaller/local models ignore the JSON envelope and answer
            // directly. Treat non-JSON or unknown actions as final text so
            // chat does not get stuck in a repair loop.
            let answer = raw.trim().to_string();
            if event_sink.is_some() && !answer.is_empty() {
                emit_event(
                    &mut events,
                    &event_sink,
                    AgentEvent::MessageDelta {
                        text: answer.clone(),
                    },
                );
            }
            emit_event(
                &mut events,
                &event_sink,
                AgentEvent::Done {
                    session_id: session_id.clone(),
                },
            );
            return Ok(AgentLoopOutcome {
                message: answer,
                references,
                events,
                user_input_request: None,
            });
        }

        if action
            .tool
            .as_deref()
            .map(is_user_ask_tool)
            .unwrap_or(false)
        {
            let request_form = match sanitize_user_input_request(&action) {
                Ok(request_form) => request_form,
                Err(err) => {
                    observations.push(record_loop_tool_rejection(
                        "user.ask",
                        format!(
                            "{err}. Return a corrected user.ask schema or answer without asking."
                        ),
                        &mut events,
                        &event_sink,
                    ));
                    continue;
                }
            };
            emit_event(
                &mut events,
                &event_sink,
                AgentEvent::UserInputRequired {
                    request: request_form.clone(),
                },
            );
            emit_event(
                &mut events,
                &event_sink,
                AgentEvent::Done {
                    session_id: session_id.clone(),
                },
            );
            let answer = request_form.description.clone().unwrap_or_else(|| {
                "Please provide the requested information to continue.".to_string()
            });
            return Ok(AgentLoopOutcome {
                message: answer,
                references,
                events,
                user_input_request: Some(request_form),
            });
        }

        if action.tool.as_deref() == Some("shell.exec") {
            let Some(command) = shell_command_from_action(&action) else {
                observations.push(record_loop_tool_rejection(
                    "shell.exec",
                    "shell.exec command is empty".to_string(),
                    &mut events,
                    &event_sink,
                ));
                continue;
            };
            if skills.is_empty() {
                observations.push(record_loop_tool_rejection(
                    "shell.exec",
                    "shell.exec is only available when at least one skill is active for this turn"
                        .to_string(),
                    &mut events,
                    &event_sink,
                ));
                continue;
            }
            if let Err(err) = permission_policy.require(AgentCapability::Process) {
                observations.push(record_loop_tool_rejection(
                    "shell.exec",
                    err,
                    &mut events,
                    &event_sink,
                ));
                continue;
            }
            if !is_shell_command_approved(&command, &request.approved_shell_commands) {
                emit_event(
                    &mut events,
                    &event_sink,
                    AgentEvent::tool_start("shell.exec", Some(command.clone())),
                );
                emit_event(
                    &mut events,
                    &event_sink,
                    AgentEvent::tool_end(
                        "shell.exec",
                        Some(format!("approval required: {command}")),
                    ),
                );
                let request_form = shell_approval_request(&command);
                emit_event(
                    &mut events,
                    &event_sink,
                    AgentEvent::UserInputRequired {
                        request: request_form.clone(),
                    },
                );
                emit_event(
                    &mut events,
                    &event_sink,
                    AgentEvent::Done {
                        session_id: session_id.clone(),
                    },
                );
                let answer = format!(
                    "The Agent needs approval before it can run this command:\n\n`{command}`\n\nApprove the command if you want the Agent to continue with this skill."
                );
                return Ok(AgentLoopOutcome {
                    message: answer,
                    references,
                    events,
                    user_input_request: Some(request_form),
                });
            }
        }

        if action.tool.as_deref().is_some_and(is_agent_retrieval_tool) {
            let tool = action.tool.as_deref().unwrap_or_default();
            if let Ok(input) = agent_loop_tool_input(tool, &action) {
                let signature = format!("{tool}:{}", canonical_json(&input));
                if !executed_retrievals.insert(signature) {
                    observations.push(record_loop_tool_rejection(
                        tool,
                        "duplicate retrieval skipped; use the existing observation and answer the user"
                            .to_string(),
                        &mut events,
                        &event_sink,
                    ));
                    force_final_next = true;
                    continue;
                }
            }
            retrieval_steps += 1;
        }

        let observation = execute_agent_loop_tool(
            &tool_context,
            request,
            &action,
            permission_policy,
            skills,
            &mut references,
            &mut events,
            &event_sink,
            cancellation,
        )
        .await?;
        observations.push(observation);
    }

    let answer = agent_iteration_limit_answer(max_iterations, observations.len(), &references);
    if event_sink.is_some() {
        emit_event(
            &mut events,
            &event_sink,
            AgentEvent::MessageDelta {
                text: answer.clone(),
            },
        );
    }
    emit_event(
        &mut events,
        &event_sink,
        AgentEvent::Done {
            session_id: session_id.clone(),
        },
    );
    Ok(AgentLoopOutcome {
        message: answer,
        references,
        events,
        user_input_request: None,
    })
}

#[allow(clippy::too_many_arguments)]
async fn execute_agent_loop_tool(
    tool_context: &ToolContext<'_>,
    request: &AgentLoopRequest,
    action: &AgentLoopAction,
    permission_policy: &PermissionPolicy,
    skills: &[AgentSkill],
    references: &mut Vec<AgentReference>,
    events: &mut Vec<AgentEvent>,
    event_sink: &Option<AgentEventSink>,
    cancellation: Option<&AgentCancellationToken>,
) -> Result<AgentObservation, String> {
    let tool = match action
        .tool
        .as_deref()
        .map(str::trim)
        .filter(|tool| !tool.is_empty())
    {
        Some(tool) => tool,
        None => {
            return Ok(AgentObservation {
                tool: "agent.action".to_string(),
                summary: "invalid tool action: missing tool name".to_string(),
            });
        }
    };

    let input = match agent_loop_tool_input(tool, action) {
        Ok(input) => input,
        Err(err) => {
            return Ok(record_loop_tool_rejection(tool, err, events, event_sink));
        }
    };
    let input_detail = summarize_tool_input(tool, &input);

    if tool == "skill.read_file" {
        if let Err(err) = permission_policy.require(AgentCapability::ReadProject) {
            return Ok(record_loop_tool_rejection(tool, err, events, event_sink));
        }
        emit_event(
            events,
            event_sink,
            AgentEvent::tool_start(tool, input_detail),
        );
        return match read_active_skill_file(skills, &input) {
            Ok(value) => {
                let summary = record_loop_tool_success(tool, value, references, events, event_sink)?;
                emit_event(
                    events,
                    event_sink,
                    AgentEvent::tool_end(tool, Some(summary.clone())),
                );
                Ok(AgentObservation {
                    tool: tool.to_string(),
                    summary,
                })
            }
            Err(err) => {
                emit_event(
                    events,
                    event_sink,
                    AgentEvent::tool_end(tool, Some(format!("failed: {err}"))),
                );
                Ok(AgentObservation {
                    tool: tool.to_string(),
                    summary: format!("failed: {err}"),
                })
            }
        };
    }

    if let Err(err) = require_tool_permission(tool, request.web_enabled, permission_policy) {
        return Ok(record_loop_tool_rejection(tool, err, events, event_sink));
    }
    emit_event(
        events,
        event_sink,
        AgentEvent::tool_start(tool, input_detail),
    );

    let result = execute_tool_with_cancellation(
        tools::execute_tool(tool, &input, tool_context.clone()),
        cancellation,
    )
    .await;

    match result {
        Ok(value) => {
            let events_before = events.len();
            let summary = record_loop_tool_success(tool, value, references, events, event_sink)?;
            record_file_versions_for_events(tool_context, &events[events_before..]).await;
            emit_event(
                events,
                event_sink,
                AgentEvent::tool_end(tool, Some(summary.clone())),
            );
            Ok(AgentObservation {
                tool: tool.to_string(),
                summary,
            })
        }
        Err(err) => {
            emit_event(
                events,
                event_sink,
                AgentEvent::tool_end(tool, Some(format!("failed: {err}"))),
            );
            Ok(AgentObservation {
                tool: tool.to_string(),
                summary: format!("failed: {err}"),
            })
        }
    }
}

// Single history chokepoint for every agent write path: wiki.write_page,
// workspace.write_file/append_file, and shell.exec generated files all emit
// FileChanged, so recording off those events cannot double-count. Mirrors
// upstream fs.rs write_file (L982-989): baseline snapshot of the pre-write
// content, then the post-write state; best-effort, never fails the tool.
async fn record_file_versions_for_events(context: &ToolContext<'_>, new_events: &[AgentEvent]) {
    for event in new_events {
        let AgentEvent::FileChanged {
            path,
            tool,
            existed_before,
            previous_content,
        } = event
        else {
            continue;
        };
        if *existed_before {
            if let Some(previous) = previous_content {
                if let Err(error) = file_history::record_content_version(
                    &context.state.pool,
                    context.project_id,
                    path,
                    "baseline",
                    &format!("before.{tool}"),
                    previous,
                )
                .await
                {
                    tracing::warn!(?error, path, "failed to record baseline file version");
                }
            }
        }
        if let Err(error) = file_history::record_disk_version(
            &context.state.pool,
            context.project_id,
            context.project_root,
            path,
            "agent",
            tool,
        )
        .await
        {
            tracing::warn!(?error, path, "failed to record agent file version");
        }
    }
}

fn agent_loop_tool_input(tool: &str, action: &AgentLoopAction) -> Result<Value, String> {
    let top_k = action
        .top_k
        .unwrap_or(DEFAULT_CHAT_SEARCH_RESULTS)
        .clamp(1, MAX_CHAT_SEARCH_RESULTS);
    match tool {
        "wiki.search" | "source.search" | "graph.search" | "web.search" => {
            let query = action
                .query
                .as_deref()
                .map(str::trim)
                .filter(|query| !query.is_empty())
                .ok_or_else(|| format!("{tool} requires query"))?;
            Ok(serde_json::json!({
                "query": query,
                "topK": top_k,
                "includeContent": action.include_content.unwrap_or(false),
            }))
        }
        "wiki.read_page" => {
            let path = action
                .path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .ok_or_else(|| "wiki.read_page requires path".to_string())?;
            Ok(serde_json::json!({ "path": path }))
        }
        "skill.read_file" => {
            let path = action
                .path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .ok_or_else(|| "skill.read_file requires path".to_string())?;
            Ok(serde_json::json!({
                "skill": action.skill.as_deref().map(str::trim).filter(|skill| !skill.is_empty()),
                "path": path,
            }))
        }
        "wiki.write_page" => {
            let path = action
                .path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .ok_or_else(|| "wiki.write_page requires path".to_string())?;
            let content = action
                .content
                .as_deref()
                .map(str::trim)
                .filter(|content| !content.is_empty())
                .ok_or_else(|| "wiki.write_page requires content".to_string())?;
            Ok(serde_json::json!({
                "path": path,
                "content": content,
                "allowOverwrite": action.allow_overwrite.unwrap_or(false),
            }))
        }
        "workspace.write_file" | "workspace.append_file" => {
            let tool_name = action.tool.as_deref().unwrap_or("workspace.write_file");
            let path = action
                .path
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .ok_or_else(|| format!("{tool_name} requires path"))?;
            let content = action
                .content
                .as_deref()
                .ok_or_else(|| format!("{tool_name} requires content"))?;
            Ok(serde_json::json!({
                "path": path,
                "content": content,
            }))
        }
        "shell.exec" => {
            let command = shell_command_from_action(action)
                .ok_or_else(|| "shell.exec command is empty".to_string())?;
            let timeout_seconds = action
                .timeout_seconds
                .unwrap_or(tools::SHELL_EXEC_TIMEOUT_SECS)
                .clamp(1, tools::SHELL_EXEC_TIMEOUT_SECS);
            Ok(serde_json::json!({
                "command": command,
                "timeoutSeconds": timeout_seconds,
            }))
        }
        other => Err(format!("Unknown Agent tool: {other}")),
    }
}

fn record_loop_tool_success(
    tool: &str,
    value: Value,
    references: &mut Vec<AgentReference>,
    events: &mut Vec<AgentEvent>,
    event_sink: &Option<AgentEventSink>,
) -> Result<String, String> {
    match tool {
        "wiki.search" => {
            let search: tools::WikiSearchToolOutput = serde_json::from_value(value)
                .map_err(|err| format!("Invalid wiki.search result: {err}"))?;
            let count = search.references.len();
            let added = search
                .references
                .into_iter()
                .filter(|reference| {
                    push_unique_reference(references, events, event_sink, reference.clone())
                })
                .count();
            Ok(format!(
                "{count} result(s), {added} new, mode={}, tokenHits={}, vectorHits={}",
                search.mode, search.token_hits, search.vector_hits
            ))
        }
        "source.search" | "graph.search" | "web.search" => {
            let found: Vec<AgentReference> = serde_json::from_value(value)
                .map_err(|err| format!("Invalid {tool} result: {err}"))?;
            let count = found.len();
            let added = found
                .into_iter()
                .filter(|reference| {
                    push_unique_reference(references, events, event_sink, reference.clone())
                })
                .count();
            Ok(format!("{count} result(s), {added} new"))
        }
        "wiki.read_page" => {
            let path = value
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("wiki page");
            let content = value.get("content").and_then(Value::as_str).unwrap_or("");
            Ok(format!(
                "read {path}\n{}",
                trim_chars(&collapse_whitespace(content), 4_000)
            ))
        }
        "skill.read_file" => {
            let skill = value
                .get("skill")
                .and_then(Value::as_str)
                .unwrap_or("skill");
            let path = value.get("path").and_then(Value::as_str).unwrap_or("file");
            let content = value.get("content").and_then(Value::as_str).unwrap_or("");
            Ok(format!(
                "read {skill}:{path}\n{}",
                trim_chars(&collapse_whitespace(content), 4_000)
            ))
        }
        "wiki.write_page" => {
            let output: tools::WikiWriteOutput = serde_json::from_value(value)
                .map_err(|err| format!("Invalid wiki.write_page result: {err}"))?;
            emit_event(
                events,
                event_sink,
                AgentEvent::FileChanged {
                    path: output.reference.path.clone(),
                    tool: "wiki.write_page".to_string(),
                    existed_before: output.existed_before,
                    previous_content: output.previous_content,
                },
            );
            let reference = output.reference;
            let path = reference.path.clone();
            push_unique_reference(references, events, event_sink, reference);
            Ok(format!("wrote {path}"))
        }
        "workspace.write_file" | "workspace.append_file" => {
            let output: tools::WorkspaceWriteOutput = serde_json::from_value(value)
                .map_err(|err| format!("Invalid {tool} result: {err}"))?;
            let path = output.path.as_str();
            let bytes = output.bytes as u64;
            let action = if tool == "workspace.append_file" {
                "updated"
            } else {
                "written"
            };
            emit_event(
                events,
                event_sink,
                AgentEvent::FileChanged {
                    path: output.path.clone(),
                    tool: tool.to_string(),
                    existed_before: output.existed_before,
                    previous_content: output.previous_content,
                },
            );
            let reference = AgentReference {
                title: Path::new(path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(path)
                    .to_string(),
                path: path.to_string(),
                kind: "workspace".to_string(),
                snippet: Some(format!("Generated file {action} by Agent ({bytes} bytes).")),
                score: None,
            };
            push_unique_reference(references, events, event_sink, reference);
            Ok(format!(
                "{} {path} ({bytes} bytes)",
                if tool == "workspace.append_file" {
                    "appended"
                } else {
                    "wrote"
                }
            ))
        }
        "shell.exec" => {
            let output: tools::ShellExecToolOutput = serde_json::from_value(value)
                .map_err(|err| format!("Invalid shell.exec result: {err}"))?;
            for generated in &output.generated_files {
                let path = generated.path.as_str();
                let bytes = generated.bytes as u64;
                emit_event(
                    events,
                    event_sink,
                    AgentEvent::FileChanged {
                        path: generated.path.clone(),
                        tool: "shell.exec".to_string(),
                        existed_before: generated.existed_before,
                        previous_content: generated.previous_content.clone(),
                    },
                );
                let reference = AgentReference {
                    title: Path::new(path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(path)
                        .to_string(),
                    path: path.to_string(),
                    kind: "workspace".to_string(),
                    snippet: Some(format!(
                        "Generated file written by shell.exec ({bytes} bytes)."
                    )),
                    score: None,
                };
                push_unique_reference(references, events, event_sink, reference);
            }
            Ok(format!(
                "shell.exec `{}` exit={:?} timedOut={}\nstdout:\n{}\nstderr:\n{}",
                output.command,
                output.exit_code,
                output.timed_out,
                output.stdout,
                output.stderr
            ))
        }
        _ => Ok(value.to_string()),
    }
}

fn agent_structured_max_tokens(has_skills: bool) -> u32 {
    if has_skills {
        AGENT_SKILL_STRUCTURED_MAX_TOKENS
    } else {
        AGENT_STRUCTURED_MAX_TOKENS
    }
}

fn read_active_skill_file(skills: &[AgentSkill], input: &Value) -> Result<Value, String> {
    if skills.is_empty() {
        return Err("skill.read_file requires an active skill".to_string());
    }
    let requested_path = input
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| "skill.read_file requires path".to_string())?;
    let (skill, relative_path) = resolve_skill_read_target(
        skills,
        input.get("skill").and_then(Value::as_str),
        requested_path,
    )?;
    if !is_safe_relative_skill_path(&relative_path) {
        return Err(
            "skill.read_file path must be a safe relative path inside the skill directory"
                .to_string(),
        );
    }
    let base = PathBuf::from(&skill.base_dir);
    let base_canon = base
        .canonicalize()
        .map_err(|err| format!("Failed to resolve skill directory: {err}"))?;
    let target = base.join(&relative_path);
    let target_canon = target
        .canonicalize()
        .map_err(|err| format!("Failed to resolve skill file: {err}"))?;
    if !target_canon.starts_with(&base_canon) {
        return Err("skill.read_file cannot read outside the active skill directory".to_string());
    }
    let meta = fs::symlink_metadata(&target_canon)
        .map_err(|err| format!("Failed to inspect skill file: {err}"))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err("skill.read_file target is not a regular file".to_string());
    }
    if meta.len() > MAX_SKILL_REFERENCE_BYTES {
        return Err(format!(
            "skill.read_file target is too large (max {MAX_SKILL_REFERENCE_BYTES} bytes)"
        ));
    }
    let content = fs::read_to_string(&target_canon)
        .map_err(|err| format!("Failed to read skill file as UTF-8 text: {err}"))?;
    Ok(serde_json::json!({
        "skill": skill.name,
        "path": relative_path,
        "content": content,
    }))
}

fn resolve_skill_read_target<'a>(
    skills: &'a [AgentSkill],
    requested_skill: Option<&str>,
    requested_path: &str,
) -> Result<(&'a AgentSkill, String), String> {
    if let Some(requested) = requested_skill
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let skill = select_active_skill_for_read(skills, Some(requested))?;
        let rel = normalize_requested_path_for_skill(skill, requested_path)?;
        return Ok((skill, rel));
    }
    if let Some((skill, rel)) = resolve_absolute_skill_path(skills, requested_path)? {
        return Ok((skill, rel));
    }
    if let Some(skill) = resolve_unique_existing_skill_path(skills, requested_path)? {
        return Ok((skill, requested_path.to_string()));
    }
    if let Some((skill, rel)) = resolve_prefixed_skill_path(skills, requested_path)? {
        return Ok((skill, rel));
    }
    let skill = select_active_skill_for_read(skills, requested_skill)?;
    Ok((skill, requested_path.to_string()))
}

fn normalize_requested_path_for_skill(
    skill: &AgentSkill,
    requested_path: &str,
) -> Result<String, String> {
    let path = Path::new(requested_path);
    if path.is_absolute() {
        let target = path
            .canonicalize()
            .map_err(|err| format!("Failed to resolve skill file: {err}"))?;
        let base = Path::new(&skill.base_dir)
            .canonicalize()
            .map_err(|err| format!("Failed to resolve skill directory: {err}"))?;
        if !target.starts_with(&base) {
            return Err(
                "skill.read_file absolute path does not belong to requested skill".to_string(),
            );
        }
        return target
            .strip_prefix(&base)
            .map_err(|err| format!("Failed to normalize skill path: {err}"))
            .map(|rel| rel.to_string_lossy().replace('\\', "/"));
    }
    if let Some((prefix, rest)) = requested_path.trim().replace('\\', "/").split_once('/') {
        if skill_matches_requested_name(skill, prefix) {
            return Ok(rest.to_string());
        }
        if skill_name_like(prefix) {
            return Err("skill.read_file path prefix does not match requested skill".to_string());
        }
    }
    if let Some((prefix, rest)) = requested_path.trim().replace('\\', "/").split_once(':') {
        if skill_matches_requested_name(skill, prefix) {
            return Ok(rest.trim_start_matches('/').to_string());
        }
        if skill_name_like(prefix) {
            return Err("skill.read_file path prefix does not match requested skill".to_string());
        }
    }
    Ok(requested_path.to_string())
}

fn resolve_absolute_skill_path<'a>(
    skills: &'a [AgentSkill],
    requested_path: &str,
) -> Result<Option<(&'a AgentSkill, String)>, String> {
    let path = Path::new(requested_path);
    if !path.is_absolute() {
        return Ok(None);
    }
    let target = path
        .canonicalize()
        .map_err(|err| format!("Failed to resolve skill file: {err}"))?;
    for skill in skills {
        let base = Path::new(&skill.base_dir)
            .canonicalize()
            .map_err(|err| format!("Failed to resolve skill directory: {err}"))?;
        if target.starts_with(&base) {
            let rel = target
                .strip_prefix(&base)
                .map_err(|err| format!("Failed to normalize skill path: {err}"))?
                .to_string_lossy()
                .replace('\\', "/");
            if !rel.is_empty() {
                return Ok(Some((skill, rel)));
            }
        }
    }
    Ok(None)
}

fn resolve_prefixed_skill_path<'a>(
    skills: &'a [AgentSkill],
    requested_path: &str,
) -> Result<Option<(&'a AgentSkill, String)>, String> {
    let normalized = requested_path.trim().replace('\\', "/");
    let Some((prefix, rest)) = split_skill_path_prefix(&normalized) else {
        return Ok(None);
    };
    if rest.trim().is_empty() {
        return Ok(None);
    }
    let matched = skills
        .iter()
        .filter(|skill| skill_matches_requested_name(skill, prefix))
        .filter(|skill| Path::new(&skill.base_dir).join(rest).exists())
        .collect::<Vec<_>>();
    match matched.as_slice() {
        [skill] => Ok(Some((*skill, rest.to_string()))),
        [] => Ok(None),
        _ => Err(format!("skill.read_file prefix is ambiguous: {prefix}")),
    }
}

fn split_skill_path_prefix(path: &str) -> Option<(&str, &str)> {
    if let Some((prefix, rest)) = path.split_once(':') {
        if skill_name_like(prefix) {
            return Some((prefix, rest.trim_start_matches('/')));
        }
    }
    let (prefix, rest) = path.split_once('/')?;
    Some((prefix, rest))
}

fn resolve_unique_existing_skill_path<'a>(
    skills: &'a [AgentSkill],
    requested_path: &str,
) -> Result<Option<&'a AgentSkill>, String> {
    if !is_safe_relative_skill_path(requested_path) {
        return Ok(None);
    }
    let mut matches = Vec::new();
    for skill in skills {
        let base = Path::new(&skill.base_dir);
        let candidate = base.join(requested_path);
        if candidate.exists() {
            let base_canon = base
                .canonicalize()
                .map_err(|err| format!("Failed to resolve skill directory: {err}"))?;
            let candidate_canon = candidate
                .canonicalize()
                .map_err(|err| format!("Failed to resolve skill file: {err}"))?;
            if candidate_canon.starts_with(base_canon) {
                matches.push(skill);
            }
        }
    }
    match matches.as_slice() {
        [skill] => Ok(Some(*skill)),
        [] => Ok(None),
        _ => Err(format!(
            "skill.read_file path is ambiguous: {requested_path}"
        )),
    }
}

fn select_active_skill_for_read<'a>(
    skills: &'a [AgentSkill],
    requested: Option<&str>,
) -> Result<&'a AgentSkill, String> {
    let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) else {
        return if skills.len() == 1 {
            Ok(&skills[0])
        } else {
            Err("skill.read_file requires skill when multiple skills are active".to_string())
        };
    };
    skills
        .iter()
        .find(|skill| skill_matches_requested_name(skill, requested))
        .ok_or_else(|| format!("Active skill not found: {requested}"))
}

fn skill_matches_requested_name(skill: &AgentSkill, requested: &str) -> bool {
    let requested_lower = requested.to_ascii_lowercase();
    if skill.name.eq_ignore_ascii_case(requested) {
        return true;
    }
    let folder_matches = |path: &str| {
        Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| {
                let name_lower = name.to_ascii_lowercase();
                name_lower == requested_lower
                    || name_lower.ends_with(&format!("-{requested_lower}"))
            })
            .unwrap_or(false)
    };
    folder_matches(&skill.base_dir)
        || Path::new(&skill.location)
            .parent()
            .and_then(|parent| parent.to_str())
            .map(folder_matches)
            .unwrap_or(false)
}

fn skill_name_like(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.contains('-')
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn is_safe_relative_skill_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn agent_loop_iteration_budget(mode: AgentMode, has_skills: bool) -> usize {
    let base = match mode {
        AgentMode::Fast => 4,
        AgentMode::Standard => MAX_AGENT_TOOL_ITERATIONS,
        AgentMode::Deep => 12,
    };
    if !has_skills {
        return base;
    }
    match mode {
        AgentMode::Fast => 8,
        AgentMode::Standard => 16,
        AgentMode::Deep => 20,
    }
}

fn agent_loop_retrieval_budget(mode: AgentMode, has_explicit_skills: bool) -> usize {
    let base = match mode {
        AgentMode::Fast => 2,
        AgentMode::Standard => 4,
        AgentMode::Deep => 8,
    };
    if has_explicit_skills { base + 4 } else { base }
}

fn is_agent_retrieval_tool(tool: &str) -> bool {
    matches!(
        tool,
        "wiki.search" | "wiki.read_page" | "source.search" | "graph.search" | "web.search"
    )
}

fn canonical_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn build_agent_loop_system(base_system: &str) -> String {
    format!(
        "{base_system}\n\nAgent loop protocol:\n\
Return only compact JSON. Do not wrap it in markdown.\n\
Choose exactly one action per turn:\n\
1. {{\"action\":\"tool\",\"tool\":\"wiki.search\",\"query\":\"...\"}}\n\
2. {{\"action\":\"tool\",\"tool\":\"user.ask\",\"title\":\"...\",\"description\":\"...\",\"fields\":[{{\"id\":\"choice\",\"type\":\"single\",\"label\":\"...\",\"options\":[{{\"label\":\"...\",\"value\":\"...\",\"recommended\":true}}]}}]}}\n\
3. {{\"action\":\"tool\",\"tool\":\"workspace.write_file\",\"path\":\"cover-image/cover.svg\",\"content\":\"...\"}}\n\
3b. {{\"action\":\"tool\",\"tool\":\"workspace.append_file\",\"path\":\"deck/index.html\",\"content\":\"...\"}}\n\
4. {{\"action\":\"final\",\"answer\":\"...\"}}\n\
Use tools when they are useful, then wait for the observation in the next turn before deciding the next step.\n\
Do not return natural-language plans such as \"I need to read...\" or \"Let me search...\". If you intend to read/search/write something, return the matching tool JSON action in this turn.\n\
Do not merely announce that you will use a skill or tool. If a skill references a file under its own directory, call skill.read_file with a relative path.\n\
Use skill.read_file for skill Markdown/reference files; do not use shell.exec to inspect skill files or optional preference files. Only use shell.exec when active skill instructions require command-line work after workspace.write_file has prepared the needed inputs. shell.exec runs in an isolated sandbox and requires user approval for every command.\n\
Use user.ask when a skill or task needs structured user choices, text input, confirmations, or multiple fields before it can continue. Do not simulate this by writing plain-text questions when a structured form is appropriate.\n\
Use workspace.write_file for generated artifacts such as SVG, HTML, Markdown drafts, JSON, or scripts. For large HTML/PPT files, first call workspace.write_file with an empty or small opening chunk, then call workspace.append_file with subsequent chunks, and finish only after the file contains closing tags such as </html>.\n\
Do not claim that a generated file exists until a workspace.write_file observation confirms it. In the final answer, mention only observed generated file paths.\n\
Converge quickly. Do not keep reading optional references, running optional validation, or polishing after the requested deliverable has been written. Prefer final as soon as the core user request is satisfied.\n\
Use wiki.write_page only when the user explicitly asks to create or update a wiki page. Existing pages are create-only unless allowOverwrite is explicitly justified by the user's request."
    )
}

fn build_agent_final_system(base_system: &str) -> String {
    format!(
        "{base_system}\n\nThe retrieval phase is complete. No tools are available now. Answer the user's latest request directly using the project context and tool observations already provided. Return only compact JSON in the form {{\"action\":\"final\",\"answer\":\"...\"}}. Do not request, announce, or simulate another search or file read."
    )
}

fn build_agent_final_user(base_user: &str, observations: &[AgentObservation]) -> String {
    let mut out = String::new();
    out.push_str(base_user);
    if !observations.is_empty() {
        out.push_str("\n\nCompleted tool observations:\n");
        out.push_str(&render_observations(observations));
    }
    out.push_str("\n\nProvide the final answer now. No further tool action is permitted.");
    out
}

fn forced_final_answer(
    raw: &str,
    action: &AgentLoopAction,
    references: &[AgentReference],
) -> String {
    if action.action.eq_ignore_ascii_case("final") {
        if let Some(answer) = action
            .answer
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            return answer.to_string();
        }
    }
    let trimmed = raw.trim();
    if !trimmed.is_empty() && !looks_like_agent_tool_json(trimmed) {
        return trimmed.to_string();
    }
    if references.is_empty() {
        return "I could not find enough project context to answer this request reliably."
            .to_string();
    }
    let mut answer = String::from("I found the following relevant project context:\n");
    for reference in references.iter().take(8) {
        answer.push_str("- ");
        answer.push_str(&reference.title);
        answer.push_str(" (");
        answer.push_str(&reference.path);
        answer.push(')');
        if let Some(snippet) = reference.snippet.as_deref() {
            answer.push_str(": ");
            answer.push_str(&trim_chars(&collapse_whitespace(snippet), 320));
        }
        answer.push('\n');
    }
    answer
}

fn build_agent_loop_user(
    base_user: &str,
    request: &AgentLoopRequest,
    skills: &[AgentSkill],
    observations: &[AgentObservation],
    iteration: usize,
    max_iterations: usize,
    has_generated_workspace_file: bool,
) -> String {
    let mut out = String::new();
    out.push_str(base_user);
    out.push_str("\n\nAvailable Agent tools for this turn:\n");
    out.push_str("- wiki.search: retrieve wiki pages for factual or topical questions.\n");
    out.push_str("- wiki.read_page: read a specific wiki markdown page by path.\n");
    out.push_str("- source.search: search raw source snippets.\n");
    out.push_str("- graph.search: retrieve relationships, neighbors, backlinks, dependencies, and connections between entities. Prefer it for relational questions and query with concise entity or concept names.\n");
    out.push_str("- wiki.write_page: create a wiki markdown page when explicitly requested.\n");
    if request.web_enabled {
        out.push_str("- web.search: search external web sources.\n");
    }
    if !skills.is_empty() {
        out.push_str("- skill.read_file: read a Markdown/reference file from an active skill directory by relative path.\n");
        out.push_str("- user.ask: pause and show the user a structured form with single-choice, multi-choice, text, textarea, or confirmation fields when an active skill needs user input.\n");
        out.push_str("- workspace.write_file: write generated artifacts under agent-workspace by relative path. For large HTML/PPT, initialize the file and then append chunks.\n");
        out.push_str("- workspace.append_file: append content to a generated artifact under agent-workspace. Prefer this for large HTML/PPT after workspace.write_file.\n");
        out.push_str("- shell.exec: run a command from the Agent workspace in an isolated sandbox when an active skill requires command-line work; commands require user approval. JSON: {\"action\":\"tool\",\"tool\":\"shell.exec\",\"command\":\"...\",\"timeoutSeconds\":30}\n");
    }
    if observations.is_empty() {
        out.push_str("\nTool observations so far: none.\n");
    } else {
        out.push_str("\nTool observations so far:\n");
        out.push_str(&render_observations(observations));
        out.push('\n');
    }
    let step_number = iteration + 1;
    let remaining_after_this = max_iterations.saturating_sub(step_number);
    out.push_str(&format!(
        "\nIteration budget: step {step_number} of {max_iterations}. You have {remaining_after_this} step(s) after this response.\n"
    ));
    if has_generated_workspace_file {
        out.push_str("A generated workspace file has already been observed. If it satisfies the core request, return final now with the file path. Do not spend remaining steps on optional validation or extra reference reads unless the user explicitly required them.\n");
    }
    if remaining_after_this <= 2 {
        out.push_str("Budget is nearly exhausted. Return final now if any useful answer or generated file exists. Use at most one more tool only when it is strictly required to complete or close an already-started file; skip optional checks, optional reads, and style polishing.\n");
    }
    out.push_str(
        "\nReturn the next JSON action now. Prefer {\"action\":\"final\",\"answer\":\"...\"} whenever the core request is already satisfied.",
    );
    out
}

fn parse_agent_loop_action(raw: &str) -> AgentLoopAction {
    let trimmed = raw.trim();
    if let Ok(action) = serde_json::from_str::<AgentLoopAction>(trimmed) {
        return normalize_agent_loop_action(action);
    }
    if let Some(json) = extract_json_object(trimmed) {
        if let Ok(action) = serde_json::from_str::<AgentLoopAction>(json) {
            return normalize_agent_loop_action(action);
        }
    }
    if looks_like_agent_tool_json(trimmed) {
        return AgentLoopAction {
            action: "invalid_tool_json".to_string(),
            answer: Some(
                "Invalid or truncated Agent tool JSON. Return one complete compact JSON object. For large generated files, initialize with workspace.write_file and continue with workspace.append_file chunks instead of putting the whole file in one JSON object."
                    .to_string(),
            ),
            ..AgentLoopAction::default()
        };
    }
    AgentLoopAction {
        action: "invalid_tool_json".to_string(),
        answer: Some(
            "Agent loop responses must be compact JSON. Return either a tool action like {\"action\":\"tool\",\"tool\":\"wiki.read_page\",\"path\":\"...\"} or a final action like {\"action\":\"final\",\"answer\":\"...\"}. Do not return plain text."
                .to_string(),
        ),
        ..AgentLoopAction::default()
    }
}

fn looks_like_agent_tool_json(value: &str) -> bool {
    let trimmed = value.trim_start();
    trimmed.starts_with('{')
        && (trimmed.contains("\"action\"")
            || trimmed.contains("\"tool\"")
            || trimmed.contains("\"command\""))
}

fn normalize_agent_loop_action(mut action: AgentLoopAction) -> AgentLoopAction {
    let trimmed_action = action.action.trim();
    if action.tool.is_none() && is_agent_loop_tool_name(trimmed_action) {
        action.tool = Some(trimmed_action.to_string());
        action.action = "tool".to_string();
    }
    if action.tool.as_deref().is_some_and(is_agent_loop_tool_name) {
        action.action = "tool".to_string();
    }
    if action.action.trim().is_empty() {
        action.action = if action.tool.is_some() {
            "tool".to_string()
        } else {
            "final".to_string()
        };
    }
    if let Some(tool) = action.tool.as_deref() {
        if is_user_ask_tool(tool) {
            action.tool = Some("user.ask".to_string());
        }
    }
    action
}

fn is_agent_loop_tool_name(value: &str) -> bool {
    matches!(
        value,
        "wiki.search"
            | "wiki.read_page"
            | "wiki.write_page"
            | "source.search"
            | "graph.search"
            | "web.search"
            | "skill.read_file"
            | "workspace.write_file"
            | "workspace.append_file"
            | "shell.exec"
    ) || is_user_ask_tool(value)
}

fn is_user_ask_tool(tool: &str) -> bool {
    matches!(
        tool.trim(),
        "user.ask" | "user_input.ask" | "askUserQuestion" | "AskUserQuestion" | "ask_user_question"
    )
}

fn shell_command_from_action(action: &AgentLoopAction) -> Option<String> {
    action
        .command
        .as_deref()
        .or(action.query.as_deref())
        .or(action.content.as_deref())
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(str::to_string)
}

fn is_shell_command_approved(command: &str, approved: &[String]) -> bool {
    let command = command.trim();
    !command.is_empty() && approved.iter().any(|item| item.trim() == command)
}

fn shell_approval_request(command: &str) -> AgentUserInputRequest {
    AgentUserInputRequest {
        request_id: format!("shell-approval:{}", Uuid::new_v4()),
        title: "Approve shell command".to_string(),
        description: Some(command.to_string()),
        fields: vec![AgentUserInputField {
            id: "approve".to_string(),
            field_type: "confirm".to_string(),
            label: "Allow this command to run in the sandbox".to_string(),
            description: Some(command.to_string()),
            placeholder: None,
            options: Vec::new(),
            default_value: Some(Value::Bool(false)),
        }],
    }
}

fn sanitize_user_input_request(action: &AgentLoopAction) -> Result<AgentUserInputRequest, String> {
    let raw_fields = action
        .fields
        .as_ref()
        .or(action.questions.as_ref())
        .ok_or_else(|| "user.ask requires fields or questions".to_string())?;
    let values = raw_fields
        .as_array()
        .ok_or_else(|| "user.ask fields must be an array".to_string())?;
    let mut fields = Vec::new();
    let mut used_field_ids = BTreeSet::new();
    for (idx, value) in values.iter().take(MAX_USER_INPUT_FIELDS).enumerate() {
        if let Some(mut field) = sanitize_user_input_field(value, idx)? {
            field.id = unique_user_input_key(&mut used_field_ids, &field.id, idx);
            fields.push(field);
        }
    }
    if fields.is_empty() {
        return Err("user.ask requires at least one valid field".to_string());
    }
    Ok(AgentUserInputRequest {
        request_id: Uuid::new_v4().to_string(),
        title: clean_user_input_text(action.title.as_deref().unwrap_or("Input required"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Input required".to_string()),
        description: clean_user_input_text(
            action
                .description
                .as_deref()
                .unwrap_or("Please provide the requested information so the Agent can continue."),
        ),
        fields,
    })
}

fn sanitize_user_input_field(
    value: &Value,
    idx: usize,
) -> Result<Option<AgentUserInputField>, String> {
    let Some(obj) = value.as_object() else {
        return Ok(None);
    };
    let raw_type = obj
        .get("type")
        .or_else(|| obj.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("single");
    let Ok(field_type) = normalize_user_input_field_type(raw_type) else {
        return Ok(None);
    };
    let id = obj
        .get("id")
        .or_else(|| obj.get("name"))
        .and_then(Value::as_str)
        .and_then(clean_user_input_id)
        .unwrap_or_else(|| format!("field_{}", idx + 1));
    let label = obj
        .get("label")
        .or_else(|| obj.get("question"))
        .or_else(|| obj.get("header"))
        .and_then(Value::as_str)
        .and_then(clean_user_input_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("Question {}", idx + 1));
    let description = obj
        .get("description")
        .and_then(Value::as_str)
        .and_then(clean_user_input_text);
    let placeholder = obj
        .get("placeholder")
        .and_then(Value::as_str)
        .and_then(clean_user_input_text);
    let mut used_option_values = BTreeSet::new();
    let options = obj
        .get("options")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(MAX_USER_INPUT_OPTIONS)
                .enumerate()
                .filter_map(|(option_idx, item)| {
                    let mut option = sanitize_user_input_option(item)?;
                    option.value =
                        unique_user_input_key(&mut used_option_values, &option.value, option_idx);
                    Some(option)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if matches!(field_type.as_str(), "single" | "multi") && options.is_empty() {
        return Ok(None);
    }
    let default_value = obj
        .get("defaultValue")
        .or_else(|| obj.get("default"))
        .cloned()
        .filter(|value| validate_user_input_default(&field_type, value, &options));
    Ok(Some(AgentUserInputField {
        id,
        field_type,
        label,
        description,
        placeholder,
        options,
        default_value,
    }))
}

fn normalize_user_input_field_type(value: &str) -> Result<String, String> {
    match value.trim() {
        "single" | "singleChoice" | "radio" | "select" => Ok("single".to_string()),
        "multi" | "multiChoice" | "checkbox" | "checkboxes" => Ok("multi".to_string()),
        "text" | "input" => Ok("text".to_string()),
        "textarea" | "longText" => Ok("textarea".to_string()),
        "confirm" | "boolean" | "switch" => Ok("confirm".to_string()),
        other => Err(format!("Unsupported user.ask field type: {other}")),
    }
}

fn sanitize_user_input_option(value: &Value) -> Option<AgentUserInputOption> {
    let obj = value.as_object()?;
    let label = obj
        .get("label")
        .or_else(|| obj.get("title"))
        .and_then(Value::as_str)
        .and_then(clean_user_input_text)
        .filter(|value| !value.is_empty())?;
    let value_text = obj
        .get("value")
        .and_then(Value::as_str)
        .and_then(clean_user_input_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| label.clone());
    let description = obj
        .get("description")
        .and_then(Value::as_str)
        .and_then(clean_user_input_text);
    let recommended = obj.get("recommended").and_then(Value::as_bool);
    Some(AgentUserInputOption {
        label,
        value: value_text,
        description,
        recommended,
    })
}

fn unique_user_input_key(used: &mut BTreeSet<String>, base: &str, idx: usize) -> String {
    if used.insert(base.to_string()) {
        return base.to_string();
    }
    for suffix in 2..=MAX_USER_INPUT_FIELDS + MAX_USER_INPUT_OPTIONS + 2 {
        let candidate = format!("{base}_{suffix}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    let fallback = format!("field_{}", idx + 1);
    if used.insert(fallback.clone()) {
        fallback
    } else {
        format!("field_{}_{}", idx + 1, used.len() + 1)
    }
}

fn validate_user_input_default(
    field_type: &str,
    value: &Value,
    options: &[AgentUserInputOption],
) -> bool {
    match field_type {
        "single" => value
            .as_str()
            .map(|default| options.iter().any(|option| option.value == default))
            .unwrap_or(false),
        "multi" => value
            .as_array()
            .map(|defaults| {
                defaults.iter().all(|item| {
                    item.as_str()
                        .map(|default| options.iter().any(|option| option.value == default))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false),
        "text" | "textarea" => value.as_str().is_some(),
        "confirm" => value.as_bool().is_some(),
        _ => false,
    }
}

fn clean_user_input_text(value: &str) -> Option<String> {
    let cleaned = value
        .chars()
        .filter(|ch| !matches!(ch, '<' | '>') && !ch.is_control())
        .take(MAX_USER_INPUT_TEXT_CHARS)
        .collect::<String>()
        .trim()
        .to_string();
    if cleaned.is_empty() { None } else { Some(cleaned) }
}

fn clean_user_input_id(value: &str) -> Option<String> {
    let cleaned = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
        .take(64)
        .collect::<String>();
    if cleaned.is_empty() { None } else { Some(cleaned) }
}

fn render_observations(observations: &[AgentObservation]) -> String {
    if observations.is_empty() {
        return String::new();
    }
    observations
        .iter()
        .enumerate()
        .map(|(idx, observation)| {
            format!(
                "{}. {}:\n{}",
                idx + 1,
                observation.tool,
                trim_chars(&observation.summary, 8_000)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn summarize_tool_input(tool: &str, input: &Value) -> Option<String> {
    match tool {
        "wiki.search" | "source.search" | "graph.search" | "web.search" => input
            .get("query")
            .and_then(Value::as_str)
            .map(str::to_string),
        "wiki.read_page" | "wiki.write_page" | "workspace.write_file" | "workspace.append_file" => {
            input
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string)
        }
        "skill.read_file" => input
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_string),
        "shell.exec" => input
            .get("command")
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

fn record_loop_tool_rejection(
    tool: &str,
    error: String,
    events: &mut Vec<AgentEvent>,
    event_sink: &Option<AgentEventSink>,
) -> AgentObservation {
    emit_event(
        events,
        event_sink,
        AgentEvent::tool_end(tool, Some(format!("rejected: {error}"))),
    );
    AgentObservation {
        tool: tool.to_string(),
        summary: format!("rejected: {error}"),
    }
}

fn push_unique_reference(
    references: &mut Vec<AgentReference>,
    events: &mut Vec<AgentEvent>,
    event_sink: &Option<AgentEventSink>,
    reference: AgentReference,
) -> bool {
    if references
        .iter()
        .any(|existing| existing.kind == reference.kind && existing.path == reference.path)
    {
        return false;
    }
    emit_event(
        events,
        event_sink,
        AgentEvent::ReferenceAdded {
            reference: reference.clone(),
        },
    );
    references.push(reference);
    true
}

fn agent_iteration_limit_answer(
    max_iterations: usize,
    observation_count: usize,
    references: &[AgentReference],
) -> String {
    let mut seen_paths = BTreeSet::new();
    let workspace_paths: Vec<String> = references
        .iter()
        .filter(|reference| reference.kind == "workspace")
        .filter_map(|reference| {
            if reference.path.trim().is_empty() {
                None
            } else if seen_paths.insert(reference.path.clone()) {
                Some(reference.path.clone())
            } else {
                None
            }
        })
        .collect();

    if workspace_paths.is_empty() {
        return format!(
            "The Agent reached the tool-iteration limit after {max_iterations} step(s). It gathered {observation_count} tool observation(s), but did not produce a final answer. Please narrow the request or ask it to continue from the latest result."
        );
    }

    // Skills can successfully write deliverables and then spend the remaining
    // budget on optional validation or reference reads. Report confirmed files
    // instead of turning a completed write into a false failure.
    let mut answer = format!(
        "The Agent reached the tool-iteration budget after {max_iterations} step(s), but it did generate file(s).\n\nGenerated files:\n"
    );
    for path in workspace_paths {
        answer.push_str("- ");
        answer.push_str(&path);
        answer.push('\n');
    }
    answer.push_str(
        "\nOpen the generated file reference(s) to preview them. Some optional validation or follow-up steps may not have completed.",
    );
    answer
}

fn require_tool_permission(
    tool: &str,
    web_enabled: bool,
    permission_policy: &PermissionPolicy,
) -> Result<(), String> {
    match tool {
        "wiki.search" => permission_policy.require(AgentCapability::SearchWiki),
        "wiki.read_page" | "graph.search" => permission_policy.require(AgentCapability::ReadProject),
        "source.search" => permission_policy.require(AgentCapability::ReadSource),
        "wiki.write_page" => permission_policy.require(AgentCapability::WriteWiki),
        "workspace.write_file" | "workspace.append_file" => {
            permission_policy.require(AgentCapability::WriteWiki)
        }
        "web.search" => {
            if !web_enabled {
                return Err("web.search is disabled for this turn".to_string());
            }
            permission_policy.require(AgentCapability::Network)
        }
        "skill.read_file" => permission_policy.require(AgentCapability::ReadProject),
        "shell.exec" => permission_policy.require(AgentCapability::Process),
        other => Err(format!("Unknown Agent tool: {other}")),
    }
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&raw[start..=end])
}

fn check_cancel(cancellation: Option<&AgentCancellationToken>) -> Result<(), String> {
    if let Some(token) = cancellation {
        token.check()?;
    }
    Ok(())
}

fn fit_context_to_model(
    mut context: BuiltAgentContext,
    max_context_size: Option<usize>,
) -> BuiltAgentContext {
    let Some(max_context_size) = max_context_size else {
        return context;
    };
    let max_chars = max_context_size.clamp(8_000, 400_000);
    let total_chars = context.system.chars().count() + context.user.chars().count();
    if total_chars <= max_chars {
        return context;
    }
    let minimum_user_budget = 4_000;
    let system_budget = max_chars.saturating_sub(minimum_user_budget);
    if context.system.chars().count() > system_budget {
        context.system = trim_chars(&context.system, system_budget);
    }
    let user_budget = max_chars
        .saturating_sub(context.system.chars().count())
        .max(minimum_user_budget);
    context.user = trim_chars(&context.user, user_budget);
    context
}

// Tool futures may include network I/O or blocking-pool filesystem scans.
// Cancelling the turn should stop waiting for them immediately. A blocking task
// already running in Tokio's blocking pool cannot be force-killed, so the
// contract is "stop the Agent turn promptly", not "terminate the OS work".
async fn execute_tool_with_cancellation<F>(
    future: F,
    cancellation: Option<&AgentCancellationToken>,
) -> Result<Value, String>
where
    F: Future<Output = Result<Value, String>>,
{
    if let Some(token) = cancellation {
        tokio::select! {
            biased;
            _ = token.cancelled() => Err("Agent turn cancelled".to_string()),
            result = future => result,
        }
    } else {
        future.await
    }
}

fn emit_event(
    events: &mut Vec<AgentEvent>,
    event_sink: &Option<AgentEventSink>,
    event: AgentEvent,
) {
    if let Some(sink) = event_sink {
        sink(event.clone());
    }
    events.push(event);
}

async fn generate_with_cancellation(
    provider: &OpenAiCompatibleProvider,
    system: &str,
    user: &str,
    max_tokens: u32,
    cancellation: Option<&AgentCancellationToken>,
) -> Result<String, String> {
    let request = ProviderTextRequest {
        system_prompt: system.to_string(),
        user_prompt: user.to_string(),
        max_tokens: Some(max_tokens),
    };
    let future = provider.complete_text(request);
    if let Some(token) = cancellation {
        tokio::select! {
            biased;
            _ = token.cancelled() => Err("Agent turn cancelled".to_string()),
            result = future => result.map(|response| response.text).map_err(|err| err.message().to_string()),
        }
    } else {
        future
            .await
            .map(|response| response.text)
            .map_err(|err| err.message().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_skill(name: &str, files: &[(&str, &str)]) -> (PathBuf, AgentSkill) {
        let root = std::env::temp_dir().join(format!(
            "knowledge-agent-runtime-{name}-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        for (rel, content) in files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, content).unwrap();
        }
        let skill = AgentSkill {
            name: name.to_string(),
            description: "test skill".to_string(),
            instructions: "instructions".to_string(),
            base_dir: root.to_string_lossy().replace('\\', "/"),
            location: root
                .join("SKILL.md")
                .to_string_lossy()
                .replace('\\', "/"),
        };
        (root, skill)
    }

    #[test]
    fn parse_action_accepts_direct_and_wrapped_json() {
        let direct = parse_agent_loop_action(r#"{"action":"tool","tool":"wiki.search","query":"alpha"}"#);
        assert_eq!(direct.action, "tool");
        assert_eq!(direct.tool.as_deref(), Some("wiki.search"));

        let wrapped = parse_agent_loop_action(
            "Sure, here is the action:\n```json\n{\"action\":\"final\",\"answer\":\"done\"}\n```",
        );
        assert_eq!(wrapped.action, "final");
        assert_eq!(wrapped.answer.as_deref(), Some("done"));
    }

    #[test]
    fn parse_action_flags_broken_tool_json_for_retry() {
        let broken = parse_agent_loop_action(r#"{"action":"tool","tool":"wiki.search","query":"#);
        assert_eq!(broken.action, "invalid_tool_json");
        assert!(broken.answer.unwrap().contains("compact JSON"));

        let plain = parse_agent_loop_action("plain text without json");
        assert_eq!(plain.action, "invalid_tool_json");
    }

    #[test]
    fn normalize_action_recovers_tool_name_in_action_field() {
        let action = parse_agent_loop_action(r#"{"action":"wiki.search","query":"alpha"}"#);
        assert_eq!(action.action, "tool");
        assert_eq!(action.tool.as_deref(), Some("wiki.search"));

        let ask = parse_agent_loop_action(r#"{"action":"tool","tool":"AskUserQuestion","fields":[]}"#);
        assert_eq!(ask.tool.as_deref(), Some("user.ask"));

        let bare_tool = parse_agent_loop_action(r#"{"tool":"graph.search","query":"beta"}"#);
        assert_eq!(bare_tool.action, "tool");

        let empty_action = parse_agent_loop_action(r#"{"answer":"hi"}"#);
        assert_eq!(empty_action.action, "final");
    }

    #[test]
    fn iteration_and_retrieval_budgets_match_upstream() {
        assert_eq!(agent_loop_iteration_budget(AgentMode::Fast, false), 4);
        assert_eq!(agent_loop_iteration_budget(AgentMode::Standard, false), 8);
        assert_eq!(agent_loop_iteration_budget(AgentMode::Deep, false), 12);
        assert_eq!(agent_loop_iteration_budget(AgentMode::Fast, true), 8);
        assert_eq!(agent_loop_iteration_budget(AgentMode::Standard, true), 16);
        assert_eq!(agent_loop_iteration_budget(AgentMode::Deep, true), 20);

        assert_eq!(agent_loop_retrieval_budget(AgentMode::Fast, false), 2);
        assert_eq!(agent_loop_retrieval_budget(AgentMode::Standard, false), 4);
        assert_eq!(agent_loop_retrieval_budget(AgentMode::Deep, false), 8);
        assert_eq!(agent_loop_retrieval_budget(AgentMode::Deep, true), 12);

        assert_eq!(agent_structured_max_tokens(false), 8192);
        assert_eq!(agent_structured_max_tokens(true), 16384);
    }

    #[test]
    fn sanitize_user_input_request_normalizes_fields() {
        let action = parse_agent_loop_action(
            r#"{"action":"tool","tool":"user.ask","title":"Pick <a> style","fields":[
                {"id":"style","type":"radio","label":"Style","options":[
                    {"label":"Dark","value":"dark","recommended":true},
                    {"label":"Light"}
                ],"default":"dark"},
                {"id":"style","type":"text","label":"Notes"},
                {"type":"unknown","label":"dropped"},
                {"type":"single","label":"no options -> dropped"}
            ]}"#,
        );
        let form = sanitize_user_input_request(&action).unwrap();
        assert_eq!(form.title, "Pick a style");
        assert_eq!(form.fields.len(), 2);
        assert_eq!(form.fields[0].field_type, "single");
        assert_eq!(form.fields[0].options.len(), 2);
        assert_eq!(form.fields[0].options[1].value, "Light");
        assert_eq!(
            form.fields[0].default_value,
            Some(Value::String("dark".to_string()))
        );
        // Duplicate id gets a unique suffix.
        assert_eq!(form.fields[1].id, "style_2");
    }

    #[test]
    fn sanitize_user_input_request_requires_valid_fields() {
        let missing = parse_agent_loop_action(r#"{"action":"tool","tool":"user.ask"}"#);
        assert!(sanitize_user_input_request(&missing).is_err());

        let empty = parse_agent_loop_action(r#"{"action":"tool","tool":"user.ask","fields":[]}"#);
        assert!(sanitize_user_input_request(&empty).is_err());
    }

    #[test]
    fn forced_final_answer_prefers_action_then_plain_text_then_references() {
        let final_action =
            parse_agent_loop_action(r#"{"action":"final","answer":"the answer"}"#);
        assert_eq!(
            forced_final_answer("raw", &final_action, &[]),
            "the answer"
        );

        let non_json = parse_agent_loop_action("plain prose answer");
        assert_eq!(
            forced_final_answer("plain prose answer", &non_json, &[]),
            "plain prose answer"
        );

        let tool_json = r#"{"action":"tool","tool":"wiki.search","query":"x"}"#;
        let tool_action = parse_agent_loop_action(tool_json);
        let references = vec![AgentReference {
            title: "Alpha".to_string(),
            path: "wiki/alpha.md".to_string(),
            kind: "wiki".to_string(),
            snippet: Some("alpha snippet".to_string()),
            score: None,
        }];
        let answer = forced_final_answer(tool_json, &tool_action, &references);
        assert!(answer.contains("Alpha (wiki/alpha.md)"));

        let empty_refs = forced_final_answer(tool_json, &tool_action, &[]);
        assert!(empty_refs.contains("could not find enough project context"));
    }

    #[test]
    fn iteration_limit_answer_reports_generated_workspace_files() {
        let plain = agent_iteration_limit_answer(8, 8, &[]);
        assert!(plain.contains("tool-iteration limit"));

        let references = vec![AgentReference {
            title: "cover.svg".to_string(),
            path: "agent-workspace/cover.svg".to_string(),
            kind: "workspace".to_string(),
            snippet: None,
            score: None,
        }];
        let with_files = agent_iteration_limit_answer(8, 8, &references);
        assert!(with_files.contains("agent-workspace/cover.svg"));
        assert!(with_files.contains("Generated files"));
    }

    #[test]
    fn tool_permissions_gate_web_and_unknown_tools() {
        let editor = PermissionPolicy::for_role(crate::tenancy::access::AccessRole::Editor);
        assert!(require_tool_permission("wiki.search", false, &editor).is_ok());
        assert!(require_tool_permission("wiki.write_page", false, &editor).is_ok());
        assert!(require_tool_permission("web.search", false, &editor).is_err());
        assert!(require_tool_permission("web.search", true, &editor).is_ok());
        assert!(require_tool_permission("shell.exec", true, &editor).is_ok());
        assert!(require_tool_permission("nope.tool", true, &editor).is_err());

        let viewer = PermissionPolicy::for_role(crate::tenancy::access::AccessRole::Viewer);
        assert!(require_tool_permission("wiki.write_page", false, &viewer).is_err());
        assert!(require_tool_permission("workspace.write_file", false, &viewer).is_err());
        assert!(require_tool_permission("shell.exec", false, &viewer).is_err());
        assert!(require_tool_permission("wiki.search", false, &viewer).is_ok());
    }

    #[test]
    fn tool_input_validates_and_clamps() {
        let action = parse_agent_loop_action(
            r#"{"action":"tool","tool":"wiki.search","query":"alpha","topK":99}"#,
        );
        let input = agent_loop_tool_input("wiki.search", &action).unwrap();
        assert_eq!(input["topK"], 10);
        assert_eq!(input["query"], "alpha");

        let missing_query =
            parse_agent_loop_action(r#"{"action":"tool","tool":"wiki.search"}"#);
        assert!(agent_loop_tool_input("wiki.search", &missing_query).is_err());

        let write = parse_agent_loop_action(
            r#"{"action":"tool","tool":"wiki.write_page","path":"wiki/a.md","content":"body"}"#,
        );
        let input = agent_loop_tool_input("wiki.write_page", &write).unwrap();
        assert_eq!(input["allowOverwrite"], false);
    }

    #[test]
    fn shell_exec_input_clamps_timeout_and_falls_back_for_command() {
        let action = parse_agent_loop_action(
            r#"{"action":"tool","tool":"shell.exec","command":"python make.py","timeoutSeconds":999}"#,
        );
        let input = agent_loop_tool_input("shell.exec", &action).unwrap();
        assert_eq!(input["command"], "python make.py");
        assert_eq!(input["timeoutSeconds"], 30);
        assert_eq!(
            summarize_tool_input("shell.exec", &input).as_deref(),
            Some("python make.py")
        );

        // Upstream fallback order: command -> query -> content.
        let via_query =
            parse_agent_loop_action(r#"{"action":"tool","tool":"shell.exec","query":"ls -la"}"#);
        assert_eq!(shell_command_from_action(&via_query).as_deref(), Some("ls -la"));
        let via_content =
            parse_agent_loop_action(r#"{"action":"tool","tool":"shell.exec","content":" pwd "}"#);
        assert_eq!(shell_command_from_action(&via_content).as_deref(), Some("pwd"));

        let missing = parse_agent_loop_action(r#"{"action":"tool","tool":"shell.exec"}"#);
        assert!(shell_command_from_action(&missing).is_none());
        assert!(agent_loop_tool_input("shell.exec", &missing).is_err());
    }

    #[test]
    fn shell_command_approval_requires_exact_trimmed_match() {
        let approved = vec!["python make.py".to_string(), "  ls -la  ".to_string()];
        assert!(is_shell_command_approved("python make.py", &approved));
        assert!(is_shell_command_approved("ls -la", &approved));
        assert!(!is_shell_command_approved("python make.py --force", &approved));
        assert!(!is_shell_command_approved("", &approved));
        assert!(!is_shell_command_approved("python make.py", &[]));
    }

    #[test]
    fn shell_approval_request_carries_command_as_confirm_field() {
        let form = shell_approval_request("python make.py");
        assert!(form.request_id.starts_with("shell-approval:"));
        assert_eq!(form.title, "Approve shell command");
        assert_eq!(form.description.as_deref(), Some("python make.py"));
        assert_eq!(form.fields.len(), 1);
        assert_eq!(form.fields[0].id, "approve");
        assert_eq!(form.fields[0].field_type, "confirm");
        assert_eq!(form.fields[0].description.as_deref(), Some("python make.py"));
        assert_eq!(form.fields[0].default_value, Some(Value::Bool(false)));
    }

    #[test]
    fn skill_read_file_is_scoped_to_skill_directory() {
        let (root, skill) = temp_skill(
            "reader",
            &[
                ("SKILL.md", "---\nname: reader\ndescription: d\n---\nbody"),
                ("references/guide.md", "guide body"),
            ],
        );
        let skills = vec![skill];

        let ok = read_active_skill_file(
            &skills,
            &serde_json::json!({"path": "references/guide.md"}),
        )
        .unwrap();
        assert_eq!(ok["content"], "guide body");
        assert_eq!(ok["path"], "references/guide.md");

        let escape = read_active_skill_file(
            &skills,
            &serde_json::json!({"path": "../outside.md"}),
        );
        assert!(escape.is_err());

        let missing = read_active_skill_file(&skills, &serde_json::json!({"path": "nope.md"}));
        assert!(missing.is_err());

        assert!(read_active_skill_file(&[], &serde_json::json!({"path": "x"})).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn skill_read_file_resolves_prefixed_paths_and_named_skills() {
        let (root_a, skill_a) = temp_skill(
            "cover-image",
            &[
                ("SKILL.md", "---\nname: cover-image\ndescription: d\n---\nbody"),
                ("refs/palette.md", "palette"),
            ],
        );
        let (root_b, skill_b) = temp_skill(
            "deck-builder",
            &[("SKILL.md", "---\nname: deck-builder\ndescription: d\n---\nbody")],
        );
        let skills = vec![skill_a.clone(), skill_b];

        // Unique existing path resolves without naming the skill.
        let unique =
            read_active_skill_file(&skills, &serde_json::json!({"path": "refs/palette.md"}))
                .unwrap();
        assert_eq!(unique["skill"], "cover-image");

        // Explicit skill parameter.
        let named = read_active_skill_file(
            &skills,
            &serde_json::json!({"skill": "cover-image", "path": "refs/palette.md"}),
        )
        .unwrap();
        assert_eq!(named["content"], "palette");

        // skill-prefixed path form "name/rest".
        let base_name = Path::new(&skill_a.base_dir)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let prefixed = read_active_skill_file(
            &skills,
            &serde_json::json!({"path": format!("{base_name}/refs/palette.md")}),
        )
        .unwrap();
        assert_eq!(prefixed["content"], "palette");

        // Multiple active skills without a hint require the skill parameter.
        let ambiguous = read_active_skill_file(&skills, &serde_json::json!({"path": "SKILL.md"}));
        assert!(ambiguous.is_err());

        let _ = fs::remove_dir_all(root_a);
        let _ = fs::remove_dir_all(root_b);
    }

    #[test]
    fn safe_relative_skill_path_rejects_traversal() {
        assert!(is_safe_relative_skill_path("refs/guide.md"));
        assert!(is_safe_relative_skill_path("./refs/guide.md"));
        assert!(!is_safe_relative_skill_path("../guide.md"));
        assert!(!is_safe_relative_skill_path("/etc/passwd"));
        assert!(!is_safe_relative_skill_path(""));
    }

    #[test]
    fn fit_context_trims_system_then_user() {
        let context = BuiltAgentContext {
            system: "s".repeat(20_000),
            user: "u".repeat(20_000),
        };
        let fitted = fit_context_to_model(context.clone(), Some(10_000));
        assert!(fitted.system.chars().count() <= 6_000);
        assert!(fitted.user.chars().count() >= 4_000);
        assert!(
            fitted.system.chars().count() + fitted.user.chars().count() <= 10_000 + 3
        );

        let unchanged = fit_context_to_model(context.clone(), None);
        assert_eq!(unchanged.system.chars().count(), 20_000);

        let fits = fit_context_to_model(
            BuiltAgentContext {
                system: "small".to_string(),
                user: "user".to_string(),
            },
            Some(10_000),
        );
        assert_eq!(fits.system, "small");
    }

    #[test]
    fn extract_json_object_finds_embedded_braces() {
        assert_eq!(
            extract_json_object("prefix {\"a\":1} suffix"),
            Some("{\"a\":1}")
        );
        assert_eq!(extract_json_object("no json"), None);
        assert_eq!(extract_json_object("} {"), None);
    }

    #[test]
    fn retrieval_tool_set_matches_available_tools() {
        for tool in [
            "wiki.search",
            "wiki.read_page",
            "source.search",
            "graph.search",
            "web.search",
        ] {
            assert!(is_agent_retrieval_tool(tool));
        }
        assert!(!is_agent_retrieval_tool("wiki.write_page"));
        assert!(!is_agent_retrieval_tool("workspace.write_file"));
    }

    #[test]
    fn loop_prompts_carry_budget_and_tool_availability() {
        let request = AgentLoopRequest {
            query: "q".to_string(),
            session_id: "s".to_string(),
            mode: AgentMode::Standard,
            skill_mode: AgentSkillMode::Auto,
            web_enabled: false,
            history: Vec::new(),
            skills: Vec::new(),
            context_files: Vec::new(),
            approved_shell_commands: Vec::new(),
        };
        let user = build_agent_loop_user("base", &request, &[], &[], 0, 8, false);
        assert!(user.contains("step 1 of 8"));
        assert!(user.contains("wiki.search"));
        assert!(!user.contains("web.search"));
        assert!(!user.contains("workspace.write_file"));

        let mut web_request = request.clone();
        web_request.web_enabled = true;
        let user = build_agent_loop_user("base", &web_request, &[], &[], 6, 8, true);
        assert!(user.contains("web.search"));
        assert!(user.contains("Budget is nearly exhausted"));
        assert!(user.contains("generated workspace file has already been observed"));

        let system = build_agent_loop_system("base");
        assert!(system.contains("Agent loop protocol"));
        assert!(system.contains("workspace.append_file"));

        let final_system = build_agent_final_system("base");
        assert!(final_system.contains("No tools are available now"));
        let final_user = build_agent_final_user(
            "base",
            &[AgentObservation {
                tool: "wiki.search".to_string(),
                summary: "2 result(s)".to_string(),
            }],
        );
        assert!(final_user.contains("Completed tool observations"));
        assert!(final_user.contains("wiki.search"));
    }
}
