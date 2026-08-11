//! The 9 MCP tools, ported from upstream_llm_wiki/mcp-server/src/index.ts
//! (schemas L31-158, dispatch L160-246). Upstream proxies over HTTP to the
//! desktop API; here each tool calls the same internal functions the HTTP
//! handlers use. Adaptations: tools are `knowledge_*`, `project_id` is a UUID
//! defaulting to the API token's scoped project, chat's `session_id` becomes
//! `conversation_id`, and the desktop-only `local_first` mode / `anytxt`
//! options are dropped.

use serde_json::{Value, json};
use uuid::Uuid;

use knowledge_core::graph::build_graph_view;
use knowledge_core::project::files::{
  ProjectFileListOptions, clamp_max_files, list_project_files, parse_project_file_root,
  read_project_file_content,
};
use knowledge_core::project::reviews::load_reviews;
use knowledge_core::search::SearchOptions;

use crate::agent::context::AgentConversationMessage;
use crate::agent::permissions::PermissionPolicy;
use crate::agent::runtime::{AgentLoopRequest, run_agent_loop};
use crate::agent::skills::load_skills;
use crate::agent::types::{AgentMode, AgentSkillMode};
use crate::app::state::AppState;
use crate::auth::principal::Principal;
use crate::chat::store::{append_agent_message, append_message, find_conversation, list_messages};
use crate::projects::audit::{CreateAuditLog, append_audit_log};
use crate::projects::service::{list_accessible_projects_for_user, project_root_for_id};
use crate::projects::tasks::{CreateTaskRecord, create_queued_task};
use crate::providers::load_active_connection;
use crate::retrieval::service::search_project_hybrid;
use crate::tenancy::access::{AccessRole, project_access_role};

use super::format;

const MAX_HISTORY_MESSAGES: usize = 10;

#[derive(Debug)]
pub enum ToolError {
  /// Bad arguments — surfaced as JSON-RPC -32602.
  InvalidParams(String),
  /// Unknown tool name — surfaced as JSON-RPC -32602.
  UnknownTool(String),
  /// The tool ran and failed — surfaced as a `result` with `isError: true`
  /// per the MCP spec, so the model can see and react to the failure.
  Execution(String),
}

/// Tool declarations for `tools/list` (upstream index.ts L31-158).
pub fn tool_list() -> Vec<Value> {
  let project_id_prop = json!({
    "type": "string",
    "description": "Project UUID. Defaults to the API token's scoped project.",
  });
  vec![
    json!({
      "name": "knowledge_status",
      "description": "Check whether the knowledge server is reachable and list accessible projects.",
      "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
    }),
    json!({
      "name": "knowledge_projects",
      "description": "List knowledge projects accessible to the caller.",
      "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
    }),
    json!({
      "name": "knowledge_files",
      "description": "List files from a project using the caller's API permissions.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "project_id": project_id_prop,
          "root": { "type": "string", "enum": ["wiki", "sources", "all"], "description": "Tree root to list. Defaults to wiki." },
          "recursive": { "type": "boolean", "description": "Whether to list recursively. Defaults to true." },
          "max_files": { "type": "number", "description": "Maximum files returned. Max 10000." },
        },
        "additionalProperties": false,
      },
    }),
    json!({
      "name": "knowledge_read_file",
      "description": "Read a text file from a project. Only public project paths such as wiki/ and raw/sources/ are allowed.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "project_id": project_id_prop,
          "path": { "type": "string", "description": "Project-relative file path, for example wiki/index.md." },
        },
        "required": ["path"],
        "additionalProperties": false,
      },
    }),
    json!({
      "name": "knowledge_reviews",
      "description": "List review items from a project. Defaults to unresolved items so agent clients can help manage pending wiki review work.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "project_id": project_id_prop,
          "status": { "type": "string", "enum": ["unresolved", "resolved", "all"], "description": "Review status filter. Defaults to unresolved." },
          "type": { "type": "string", "description": "Optional review item type filter, for example missing-page, duplicate, contradiction, confirm, or suggestion." },
          "limit": { "type": "number", "description": "Maximum review items returned." },
        },
        "additionalProperties": false,
      },
    }),
    json!({
      "name": "knowledge_search",
      "description": "Search a project using the same hybrid keyword/vector/graph retrieval used by the HTTP API.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "project_id": project_id_prop,
          "query": { "type": "string", "description": "Search query." },
          "top_k": { "type": "number", "description": "Maximum results. The server clamps to its configured maximum." },
          "include_content": { "type": "boolean", "description": "Include full page content in results." },
        },
        "required": ["query"],
        "additionalProperties": false,
      },
    }),
    json!({
      "name": "knowledge_chat",
      "description": "Ask the knowledge server Agent a question about a project. Returns the answer with references and tool events.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "project_id": project_id_prop,
          "message": { "type": "string", "description": "User message or question." },
          "conversation_id": { "type": "string", "description": "Optional existing conversation id. When set, history is loaded and the exchange is persisted; otherwise the run is one-shot." },
          "mode": { "type": "string", "enum": ["fast", "standard", "deep"], "description": "Agent mode. Defaults to standard." },
          "web": { "type": "boolean", "description": "Enable backend web.search when the Agent decides external search is useful. Defaults to false." },
          "skills": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Optional project skills to inject.",
          },
        },
        "required": ["message"],
        "additionalProperties": false,
      },
    }),
    json!({
      "name": "knowledge_graph",
      "description": "Query the project knowledge graph.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "project_id": project_id_prop,
          "q": { "type": "string", "description": "Optional text filter." },
          "node_type": { "type": "string", "description": "Optional node type filter." },
          "limit": { "type": "number", "description": "Maximum nodes." },
        },
        "additionalProperties": false,
      },
    }),
    json!({
      "name": "knowledge_rescan_sources",
      "description": "Queue a source rescan task for a project. Requires editor access.",
      "inputSchema": {
        "type": "object",
        "properties": { "project_id": project_id_prop },
        "additionalProperties": false,
      },
    }),
  ]
}

pub async fn call_tool(
  state: &AppState,
  principal: &Principal,
  name: &str,
  args: &Value,
) -> Result<String, ToolError> {
  match name {
    // status is exempt from the mcp_enabled gate (upstream index.ts L164-170):
    // it exists to diagnose exactly that switch.
    "knowledge_status" => status(state, principal).await,
    "knowledge_projects" => {
      check_mcp_enabled(state).await?;
      projects(state, principal).await
    }
    "knowledge_files" => {
      check_mcp_enabled(state).await?;
      files(state, principal, args).await
    }
    "knowledge_read_file" => {
      check_mcp_enabled(state).await?;
      read_file(state, principal, args).await
    }
    "knowledge_reviews" => {
      check_mcp_enabled(state).await?;
      reviews(state, principal, args).await
    }
    "knowledge_search" => {
      check_mcp_enabled(state).await?;
      search(state, principal, args).await
    }
    "knowledge_chat" => {
      check_mcp_enabled(state).await?;
      chat(state, principal, args).await
    }
    "knowledge_graph" => {
      check_mcp_enabled(state).await?;
      graph(state, principal, args).await
    }
    "knowledge_rescan_sources" => {
      check_mcp_enabled(state).await?;
      rescan_sources(state, principal, args).await
    }
    other => Err(ToolError::UnknownTool(format!("Unknown tool: {other}"))),
  }
}

async fn check_mcp_enabled(state: &AppState) -> Result<(), ToolError> {
  let enabled =
    sqlx::query_scalar::<_, bool>("SELECT mcp_enabled FROM system_settings WHERE id = 1")
      .fetch_optional(&state.pool)
      .await
      .map_err(|error| ToolError::Execution(error.to_string()))?
      .unwrap_or(false);
  if enabled {
    Ok(())
  } else {
    Err(ToolError::Execution(
      "Knowledge MCP access is disabled. Enable it in Settings -> MCP.".to_string(),
    ))
  }
}

fn resolve_project_id(args: &Value, principal: &Principal) -> Result<String, ToolError> {
  if let Some(project_id) = optional_string_arg(args.get("project_id")) {
    return Ok(project_id);
  }
  principal.project_id.clone().ok_or_else(|| {
    ToolError::InvalidParams(
      "project_id is required (or use a project-scoped API token)".to_string(),
    )
  })
}

async fn check_project_access(
  state: &AppState,
  project_id: &str,
  principal: &Principal,
  required: AccessRole,
) -> Result<AccessRole, ToolError> {
  if !principal.permits_project(project_id) {
    return Err(ToolError::Execution(
      "api token is not scoped to this project".to_string(),
    ));
  }
  let role = project_access_role(&state.pool, project_id, &principal.user_id)
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?
    .ok_or_else(|| ToolError::Execution("project not found or no access".to_string()))?;
  if role.satisfies(required) {
    Ok(role)
  } else {
    Err(ToolError::Execution(
      "insufficient permissions for this project".to_string(),
    ))
  }
}

async fn status(state: &AppState, principal: &Principal) -> Result<String, ToolError> {
  let mcp_enabled =
    sqlx::query_scalar::<_, bool>("SELECT mcp_enabled FROM system_settings WHERE id = 1")
      .fetch_optional(&state.pool)
      .await
      .map_err(|error| ToolError::Execution(error.to_string()))?
      .unwrap_or(false);
  // Projects are best-effort here (upstream swallows the projects error in
  // status): status must still answer when project listing fails.
  let projects = accessible_projects_json(state, principal).await.unwrap_or_default();
  let payload = json!({
    "ok": true,
    "service": "knowledge-server",
    "version": env!("CARGO_PKG_VERSION"),
    "mcpEnabled": mcp_enabled,
    "projects": projects,
  });
  serde_json::to_string_pretty(&payload).map_err(|error| ToolError::Execution(error.to_string()))
}

async fn projects(state: &AppState, principal: &Principal) -> Result<String, ToolError> {
  let projects = accessible_projects_json(state, principal)
    .await
    .map_err(ToolError::Execution)?;
  serde_json::to_string_pretty(&json!({ "projects": projects }))
    .map_err(|error| ToolError::Execution(error.to_string()))
}

async fn accessible_projects_json(
  state: &AppState,
  principal: &Principal,
) -> Result<Vec<Value>, String> {
  let projects = list_accessible_projects_for_user(state, &principal.user_id)
    .await
    .map_err(|error| error.to_string())?;
  Ok(
    projects
      .iter()
      // A project-scoped token must not enumerate beyond its scope.
      .filter(|project| principal.permits_project(&project.id))
      .map(|project| {
        json!({
          "id": project.id,
          "name": project.name,
          "createdAt": project.created_at,
        })
      })
      .collect(),
  )
}

async fn files(state: &AppState, principal: &Principal, args: &Value) -> Result<String, ToolError> {
  let project_id = resolve_project_id(args, principal)?;
  check_project_access(state, &project_id, principal, AccessRole::Viewer).await?;
  let root = project_root_for_id(state, &project_id)
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?;
  let root_kind = enum_arg(args.get("root"), &["wiki", "sources", "all"], "wiki");
  let options = ProjectFileListOptions {
    root: parse_project_file_root(Some(root_kind))
      .map_err(|error| ToolError::Execution(error.to_string()))?,
    recursive: bool_arg(args.get("recursive"), true),
    max_files: clamp_max_files(number_arg(args.get("max_files")).map(|value| value as usize)),
  };
  let result = tokio::task::spawn_blocking(move || list_project_files(&root, &options))
    .await
    .map_err(|_| ToolError::Execution("file listing task failed".to_string()))?
    .map_err(|error| ToolError::Execution(error.to_string()))?;
  Ok(format::format_file_tree(&result.files, result.truncated))
}

async fn read_file(
  state: &AppState,
  principal: &Principal,
  args: &Value,
) -> Result<String, ToolError> {
  let project_id = resolve_project_id(args, principal)?;
  check_project_access(state, &project_id, principal, AccessRole::Viewer).await?;
  let relative_path = string_arg(args.get("path"), "path")?;
  let root = project_root_for_id(state, &project_id)
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?;
  let content = tokio::task::spawn_blocking(move || read_project_file_content(&root, &relative_path))
    .await
    .map_err(|_| ToolError::Execution("file read task failed".to_string()))?
    .map_err(|error| ToolError::Execution(error.to_string()))?;
  Ok(format!(
    "# {}\n\n{}",
    content.path,
    format::truncate_text(&content.content, format::MAX_TEXT_BYTES)
  ))
}

async fn reviews(
  state: &AppState,
  principal: &Principal,
  args: &Value,
) -> Result<String, ToolError> {
  let project_id = resolve_project_id(args, principal)?;
  check_project_access(state, &project_id, principal, AccessRole::Viewer).await?;
  let root = project_root_for_id(state, &project_id)
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?;
  let status = enum_arg(args.get("status"), &["unresolved", "resolved", "all"], "unresolved")
    .to_string();
  let type_filter = optional_string_arg(args.get("type"));
  let limit = number_arg(args.get("limit")).map(|value| value as usize);
  let store = tokio::task::spawn_blocking(move || load_reviews(&root))
    .await
    .map_err(|_| ToolError::Execution("review load task failed".to_string()))?
    .map_err(|error| ToolError::Execution(error.to_string()))?;
  let mut items = store
    .reviews
    .into_iter()
    .filter(|review| match status.as_str() {
      "resolved" => review.status == "resolved",
      "all" => true,
      // unresolved (mirrors review_status_matches in projects/routes.rs)
      _ => review.status != "resolved" && review.status != "dismissed",
    })
    .filter(|review| {
      type_filter
        .as_deref()
        .is_none_or(|wanted| review.review_type == wanted)
    })
    .collect::<Vec<_>>();
  if let Some(limit) = limit {
    items.truncate(limit);
  }
  Ok(format::format_reviews(&items, &status))
}

async fn search(
  state: &AppState,
  principal: &Principal,
  args: &Value,
) -> Result<String, ToolError> {
  let project_id = resolve_project_id(args, principal)?;
  check_project_access(state, &project_id, principal, AccessRole::Viewer).await?;
  let query = string_arg(args.get("query"), "query")?;
  let root = project_root_for_id(state, &project_id)
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?;
  let response = search_project_hybrid(
    state,
    &project_id,
    &root,
    &query,
    SearchOptions {
      top_k: number_arg(args.get("top_k")).map_or(10, |value| value as usize),
      include_content: bool_arg(args.get("include_content"), false),
    },
  )
  .await
  .map_err(|error| ToolError::Execution(error.to_string()))?;
  Ok(format::format_search_results(&query, &response))
}

/// In-process port of the chat handler's agent path (chat/routes.rs
/// send_agent_message L309-506), minus SSE streaming: the loop runs to
/// completion and the outcome is formatted as one text block. With a
/// `conversation_id` the exchange is persisted (upstream sessionId /
/// persistSession semantics); without one the run is one-shot.
async fn chat(state: &AppState, principal: &Principal, args: &Value) -> Result<String, ToolError> {
  let project_id = resolve_project_id(args, principal)?;
  let role = check_project_access(state, &project_id, principal, AccessRole::Viewer).await?;
  let message = string_arg(args.get("message"), "message")?;
  let conversation_id = optional_string_arg(args.get("conversation_id"));
  let mode = match enum_arg(args.get("mode"), &["fast", "standard", "deep"], "standard") {
    "fast" => AgentMode::Fast,
    "deep" => AgentMode::Deep,
    _ => AgentMode::Standard,
  };
  let web_enabled = bool_arg(args.get("web"), false);
  let requested_skills = string_array_arg(args.get("skills"));

  let permission_policy = PermissionPolicy::for_role(role);
  let connection = load_active_connection(state)
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?;
  let provider = connection.provider();
  let root = project_root_for_id(state, &project_id)
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?;

  let history = if let Some(conversation_id) = &conversation_id {
    find_conversation(&state.pool, &project_id, conversation_id, &principal.user_id)
      .await
      .map_err(|error| ToolError::Execution(error.to_string()))?
      .ok_or_else(|| ToolError::Execution("conversation not found".to_string()))?;
    let history = list_messages(&state.pool, conversation_id)
      .await
      .map_err(|error| ToolError::Execution(error.to_string()))?
      .iter()
      .rev()
      .take(MAX_HISTORY_MESSAGES)
      .rev()
      .map(|record| AgentConversationMessage {
        role: record.role.clone(),
        content: record.content.clone(),
      })
      .collect::<Vec<_>>();
    append_message(&state.pool, conversation_id, "user", &message, None)
      .await
      .map_err(|error| ToolError::Execution(error.to_string()))?;
    history
  } else {
    Vec::new()
  };

  let skills = if requested_skills.is_empty() {
    Vec::new()
  } else {
    let project_path = root.as_path().to_path_buf();
    let global_dir = state.global_skills_dir.clone();
    let requested = requested_skills.clone();
    let skills = tokio::task::spawn_blocking(move || {
      load_skills(&project_path, global_dir.as_deref(), &requested)
    })
    .await
    .map_err(|_| ToolError::Execution("failed to load agent skills".to_string()))?;
    if skills.is_empty() {
      return Err(ToolError::Execution("requested agent skill was not found".to_string()));
    }
    skills
  };

  let session_id = conversation_id
    .clone()
    .unwrap_or_else(|| Uuid::new_v4().to_string());
  let loop_request = AgentLoopRequest {
    query: message,
    session_id,
    mode,
    skill_mode: AgentSkillMode::Explicit,
    web_enabled,
    history,
    skills,
    context_files: Vec::new(),
    approved_shell_commands: Vec::new(),
  };

  let mut outcome = run_agent_loop(
    state,
    &project_id,
    &root,
    &provider,
    &permission_policy,
    &loop_request,
    None,
    None,
  )
  .await
  .map_err(ToolError::Execution)?;

  for event in &mut outcome.events {
    event.redact_for_external_api();
  }

  // A user.ask / shell-approval pause cannot be resumed over MCP (no
  // interactive channel); mirror the chat route by persisting nothing and
  // returning the pending request to the caller.
  if let Some(request) = &outcome.user_input_request {
    let rendered = serde_json::to_string_pretty(request)
      .unwrap_or_else(|_| "(unrenderable request)".to_string());
    return Ok(format!(
      "The agent paused and is waiting for user input, which MCP cannot provide \
       interactively. Nothing was persisted. Pending request:\n\n{rendered}\n\n{}",
      outcome.message
    ));
  }

  if let Some(conversation_id) = &conversation_id {
    let events_value =
      serde_json::to_value(&outcome.events).unwrap_or_else(|_| Value::Array(Vec::new()));
    append_agent_message(
      &state.pool,
      conversation_id,
      "assistant",
      &outcome.message,
      None,
      Some(mode.label()),
      Some(&events_value),
    )
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?;
  }

  Ok(format::format_chat_response(
    &outcome.message,
    &outcome.references,
    &outcome.events,
    conversation_id.as_deref(),
    mode.label(),
    &project_id,
  ))
}

async fn graph(state: &AppState, principal: &Principal, args: &Value) -> Result<String, ToolError> {
  let project_id = resolve_project_id(args, principal)?;
  check_project_access(state, &project_id, principal, AccessRole::Viewer).await?;
  let root = project_root_for_id(state, &project_id)
    .await
    .map_err(|error| ToolError::Execution(error.to_string()))?;
  let query = optional_string_arg(args.get("q"));
  let node_type = optional_string_arg(args.get("node_type"));
  let limit = number_arg(args.get("limit")).map(|value| value as usize);
  let (nodes, edges) = tokio::task::spawn_blocking(move || {
    build_graph_view(root.as_path(), query.as_deref(), node_type.as_deref(), limit)
  })
  .await
  .map_err(|_| ToolError::Execution("graph task failed".to_string()))?
  .map_err(|error| ToolError::Execution(error.to_string()))?;
  Ok(format::format_graph(&nodes, &edges))
}

async fn rescan_sources(
  state: &AppState,
  principal: &Principal,
  args: &Value,
) -> Result<String, ToolError> {
  let project_id = resolve_project_id(args, principal)?;
  check_project_access(state, &project_id, principal, AccessRole::Editor).await?;
  let task = create_queued_task(
    state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "project.rescan_sources".to_string(),
      title: "Rescanned sources".to_string(),
      relative_path: None,
      detail: json!({}),
      created_by: principal.user_id.clone(),
    },
    json!({}),
  )
  .await
  .map_err(|error| ToolError::Execution(error.to_string()))?;
  append_audit_log(
    state,
    CreateAuditLog {
      project_id: Some(project_id.clone()),
      actor_id: principal.user_id.clone(),
      action: "sources.rescan.enqueued".to_string(),
      target_type: "project".to_string(),
      target_id: project_id,
      task_id: Some(task.id.clone()),
      summary: "Queued project source rescan".to_string(),
      metadata: json!({ "via": "mcp" }),
    },
  )
  .await
  .map_err(|error| ToolError::Execution(error.to_string()))?;
  serde_json::to_string_pretty(&json!({ "taskId": task.id, "status": task.status }))
    .map_err(|error| ToolError::Execution(error.to_string()))
}

// Argument helpers, ported from upstream index.ts L265-300.

fn string_arg(value: Option<&Value>, name: &str) -> Result<String, ToolError> {
  match value.and_then(Value::as_str) {
    Some(text) if !text.trim().is_empty() => Ok(text.to_string()),
    _ => Err(ToolError::InvalidParams(format!("{name} is required"))),
  }
}

fn optional_string_arg(value: Option<&Value>) -> Option<String> {
  value
    .and_then(Value::as_str)
    .filter(|text| !text.trim().is_empty())
    .map(str::to_string)
}

fn bool_arg(value: Option<&Value>, fallback: bool) -> bool {
  value.and_then(Value::as_bool).unwrap_or(fallback)
}

fn number_arg(value: Option<&Value>) -> Option<f64> {
  value.and_then(Value::as_f64).filter(|number| number.is_finite())
}

fn enum_arg<'a>(value: Option<&Value>, allowed: &[&'a str], fallback: &'a str) -> &'a str {
  match value.and_then(Value::as_str) {
    Some(text) => allowed.iter().find(|item| **item == text).copied().unwrap_or(fallback),
    None => fallback,
  }
}

fn string_array_arg(value: Option<&Value>) -> Vec<String> {
  value
    .and_then(Value::as_array)
    .map(|items| {
      items
        .iter()
        .filter_map(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
        .collect()
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tool_list_declares_all_nine_tools() {
    let tools = tool_list();
    let names = tools
      .iter()
      .map(|tool| tool["name"].as_str().unwrap_or_default())
      .collect::<Vec<_>>();
    assert_eq!(
      names,
      vec![
        "knowledge_status",
        "knowledge_projects",
        "knowledge_files",
        "knowledge_read_file",
        "knowledge_reviews",
        "knowledge_search",
        "knowledge_chat",
        "knowledge_graph",
        "knowledge_rescan_sources",
      ]
    );
    for tool in &tools {
      assert!(tool["description"].is_string());
      assert_eq!(tool["inputSchema"]["type"], "object");
    }
  }

  #[test]
  fn string_arg_rejects_missing_and_blank() {
    assert!(string_arg(Some(&json!("ok")), "path").is_ok());
    assert!(matches!(string_arg(None, "path"), Err(ToolError::InvalidParams(_))));
    assert!(matches!(string_arg(Some(&json!("  ")), "path"), Err(ToolError::InvalidParams(_))));
    assert!(matches!(string_arg(Some(&json!(42)), "path"), Err(ToolError::InvalidParams(_))));
  }

  #[test]
  fn enum_arg_falls_back_for_unknown_values() {
    assert_eq!(enum_arg(Some(&json!("sources")), &["wiki", "sources", "all"], "wiki"), "sources");
    assert_eq!(enum_arg(Some(&json!("bogus")), &["wiki", "sources", "all"], "wiki"), "wiki");
    assert_eq!(enum_arg(None, &["wiki", "sources", "all"], "wiki"), "wiki");
  }

  #[test]
  fn string_array_arg_filters_non_strings_and_blanks() {
    assert_eq!(
      string_array_arg(Some(&json!(["a", "", 3, "b"]))),
      vec!["a".to_string(), "b".to_string()]
    );
    assert!(string_array_arg(Some(&json!("not-array"))).is_empty());
    assert!(string_array_arg(None).is_empty());
  }

  #[test]
  fn number_and_bool_args_apply_fallbacks() {
    assert_eq!(number_arg(Some(&json!(5))), Some(5.0));
    assert_eq!(number_arg(Some(&json!("5"))), None);
    assert!(bool_arg(Some(&json!(true)), false));
    assert!(bool_arg(None, true));
  }

  #[test]
  fn resolve_project_id_prefers_args_then_token_scope() {
    let scoped = Principal {
      user_id: "u1".to_string(),
      csrf_token: None,
      scope: crate::auth::principal::AuthScope::ApiToken,
      token_id: Some("t1".to_string()),
      project_id: Some("p-scoped".to_string()),
    };
    assert_eq!(
      resolve_project_id(&json!({ "project_id": "p-explicit" }), &scoped).unwrap(),
      "p-explicit"
    );
    assert_eq!(resolve_project_id(&json!({}), &scoped).unwrap(), "p-scoped");

    let unscoped = Principal { project_id: None, ..scoped };
    assert!(matches!(
      resolve_project_id(&json!({}), &unscoped),
      Err(ToolError::InvalidParams(_))
    ));
  }
}
