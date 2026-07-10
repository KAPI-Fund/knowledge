use std::fs;
use std::fs::OpenOptions;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use knowledge_core::graph::{build_graph_view, neighbors_for_node};
use knowledge_core::project::root::ProjectRoot;
use knowledge_core::search::SearchOptions;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::timeout;

use crate::app::state::AppState;
use crate::canvas::executor::{ShellExecFile, ShellExecRequest};
use crate::retrieval::service::search_project_hybrid;
use crate::web_search::config::load_web_search_config;
use crate::web_search::provider::web_search;

use super::types::AgentReference;
use super::workspace::{AGENT_WORKSPACE_DIR, agent_workspace_path};

// Tool I/O limits are backend security boundaries. Do not relax them only in
// the UI: API callers can invoke the same tools without going through React
// components.
const MAX_READ_PAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_WRITE_PAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_WORKSPACE_WRITE_BYTES: usize = 2 * 1024 * 1024;
// Rollback snapshots feed fileChanged events for the current run only. Bound
// them independently from write size so a large Agent artifact cannot multiply
// event and in-memory chat costs merely to enable Undo previews.
const MAX_WORKSPACE_ROLLBACK_BYTES: u64 = 512 * 1024;
const MAX_SOURCE_SEARCH_FILES: usize = 10_000;
const MAX_SOURCE_SNIPPET_CHARS: usize = 500;
const WEB_SEARCH_TIMEOUT_SECS: u64 = 30;
// shell.exec limits ported from upstream llm_wiki tools.rs. Unlike upstream,
// the command runs in a CubeSandbox micro VM via the skill-runner sidecar, so
// the agent workspace is synced up before the run and changed files are synced
// back afterwards.
pub(crate) const SHELL_EXEC_TIMEOUT_SECS: u64 = 30;
pub(crate) const MAX_SHELL_COMMAND_CHARS: usize = 4_000;
const MAX_SHELL_OUTPUT_CHARS: usize = 20_000;
const MAX_SHELL_GENERATED_FILES: usize = 50;
const MAX_SHELL_SYNC_FILES: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffect {
    Read,
    Write,
    Network,
    Process,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub effects: Vec<ToolEffect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct ToolContext<'a> {
    pub state: &'a AppState,
    pub project_id: &'a str,
    pub project_root: &'a ProjectRoot,
}

pub async fn execute_tool(
    name: &str,
    input: &Value,
    context: ToolContext<'_>,
) -> Result<Value, String> {
    let project_path = context.project_root.as_path();
    match name {
        "wiki.write_page" => {
            let path = input
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "wiki.write_page requires path".to_string())?;
            let content = input
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| "wiki.write_page requires content".to_string())?;
            let allow_overwrite = input
                .get("allowOverwrite")
                .or_else(|| input.get("allow_overwrite"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            serde_json::to_value(write_wiki_page_with_activity(
                project_path,
                path,
                content,
                allow_overwrite,
            )?)
            .map_err(|err| format!("Failed to serialize wiki.write_page result: {err}"))
        }
        "wiki.search" => {
            let query = tool_query(input, "wiki.search")?;
            let top_k = tool_top_k(input);
            let include_content = input
                .get("includeContent")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            serde_json::to_value(
                run_wiki_search(
                    context.state,
                    context.project_id,
                    context.project_root,
                    query,
                    top_k,
                    include_content,
                )
                .await?,
            )
            .map_err(|err| format!("Failed to serialize wiki.search result: {err}"))
        }
        "wiki.read_page" => {
            let path = input
                .get("path")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .ok_or_else(|| "wiki.read_page requires path".to_string())?;
            let content = read_wiki_page(project_path, path)?;
            serde_json::to_value(json!({
                "path": path,
                "content": content,
            }))
            .map_err(|err| format!("Failed to serialize wiki.read_page result: {err}"))
        }
        "workspace.write_file" => {
            let path = input
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "workspace.write_file requires path".to_string())?;
            let content = input
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| "workspace.write_file requires content".to_string())?;
            serde_json::to_value(write_workspace_file(project_path, path, content)?)
                .map_err(|err| format!("Failed to serialize workspace.write_file result: {err}"))
        }
        "workspace.append_file" => {
            let path = input
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| "workspace.append_file requires path".to_string())?;
            let content = input
                .get("content")
                .and_then(Value::as_str)
                .ok_or_else(|| "workspace.append_file requires content".to_string())?;
            serde_json::to_value(append_workspace_file(project_path, path, content)?)
                .map_err(|err| format!("Failed to serialize workspace.append_file result: {err}"))
        }
        "source.search" => {
            let query = tool_query(input, "source.search")?.to_string();
            let project_path = project_path.to_path_buf();
            let top_k = tool_top_k(input);
            // `search_sources` walks the filesystem synchronously. Keep it off
            // Tokio worker threads so a large source tree cannot stall
            // unrelated Agent/API work.
            let references =
                tokio::task::spawn_blocking(move || search_sources(&project_path, &query, top_k))
                    .await
                    .map_err(|err| format!("source.search worker failed: {err}"))??;
            serde_json::to_value(references)
                .map_err(|err| format!("Failed to serialize source.search result: {err}"))
        }
        "graph.search" => {
            let query = tool_query(input, "graph.search")?.to_string();
            let project_path = project_path.to_path_buf();
            let top_k = tool_top_k(input);
            // Graph search also performs synchronous markdown walks. Run it in
            // the blocking pool for the same reason as `source.search`.
            let references =
                tokio::task::spawn_blocking(move || search_graph(&project_path, &query, top_k))
                    .await
                    .map_err(|err| format!("graph.search worker failed: {err}"))??;
            serde_json::to_value(references)
                .map_err(|err| format!("Failed to serialize graph.search result: {err}"))
        }
        "web.search" => {
            let query = tool_query(input, "web.search")?;
            serde_json::to_value(run_web_search(context.state, query, tool_top_k(input)).await?)
                .map_err(|err| format!("Failed to serialize web.search result: {err}"))
        }
        "shell.exec" => {
            let command = input
                .get("command")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|command| !command.is_empty())
                .ok_or_else(|| "shell.exec requires command".to_string())?;
            let timeout_secs = input
                .get("timeoutSeconds")
                .or_else(|| input.get("timeout_seconds"))
                .and_then(Value::as_u64)
                .unwrap_or(SHELL_EXEC_TIMEOUT_SECS)
                .clamp(1, SHELL_EXEC_TIMEOUT_SECS);
            serde_json::to_value(run_shell_exec(&context, command, timeout_secs).await?)
                .map_err(|err| format!("Failed to serialize shell.exec result: {err}"))
        }
        other => Err(format!("Unknown Agent tool: {other}")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiSearchToolOutput {
    pub mode: String,
    pub token_hits: usize,
    pub vector_hits: usize,
    pub references: Vec<AgentReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWriteOutput {
    pub path: String,
    pub bytes: usize,
    #[serde(default)]
    pub existed_before: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellExecToolOutput {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    #[serde(default)]
    pub generated_files: Vec<WorkspaceWriteOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiWriteOutput {
    #[serde(flatten)]
    pub reference: AgentReference,
    #[serde(default)]
    pub existed_before: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_content: Option<String>,
}

pub fn builtin_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "wiki.search".to_string(),
            description: "Search generated wiki pages using backend keyword/vector retrieval."
                .to_string(),
            effects: vec![ToolEffect::Read],
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "topK": { "type": "integer", "minimum": 1, "maximum": 10 }
                },
                "required": ["query"]
            })),
        },
        ToolSpec {
            name: "wiki.read_page".to_string(),
            description: "Read a project wiki markdown page by project-relative path.".to_string(),
            effects: vec![ToolEffect::Read],
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            })),
        },
        ToolSpec {
            name: "source.search".to_string(),
            description:
                "Search raw source files stored under raw/sources for exact keyword snippets."
                    .to_string(),
            effects: vec![ToolEffect::Read],
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "topK": { "type": "integer", "minimum": 1, "maximum": 10 }
                },
                "required": ["query"]
            })),
        },
        ToolSpec {
            name: "web.search".to_string(),
            description: "Search external web sources when the user enables web search."
                .to_string(),
            effects: vec![ToolEffect::Network],
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "topK": { "type": "integer", "minimum": 1, "maximum": 10 }
                },
                "required": ["query"]
            })),
        },
        ToolSpec {
            name: "graph.search".to_string(),
            description: "Retrieve graph relationships, neighbors, backlinks, dependencies, and connections between project entities. Use concise entity or concept names rather than a full question."
                .to_string(),
            effects: vec![ToolEffect::Read],
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "topK": { "type": "integer", "minimum": 1, "maximum": 10 }
                },
                "required": ["query"]
            })),
        },
        ToolSpec {
            name: "wiki.write_page".to_string(),
            description:
                "Create a Markdown wiki page under wiki/ with project-bound path checks. Existing files require allowOverwrite=true."
                    .to_string(),
            effects: vec![ToolEffect::Write],
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Project-relative path such as wiki/queries/new-page.md"
                    },
                    "content": { "type": "string" },
                    "allowOverwrite": {
                        "type": "boolean",
                        "description": "Defaults to false. Set true only when the user explicitly asks to overwrite an existing wiki page."
                    }
                },
                "required": ["path", "content"]
            })),
        },
        ToolSpec {
            name: "skill.read_file".to_string(),
            description:
                "Read a text reference file from an active skill directory by relative path."
                    .to_string(),
            effects: vec![ToolEffect::Read],
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "skill": {
                        "type": "string",
                        "description": "Optional active skill name; required when multiple skills are active."
                    },
                    "path": {
                        "type": "string",
                        "description": "Relative path inside the active skill directory, such as references/types.md."
                    }
                },
                "required": ["path"]
            })),
        },
        ToolSpec {
            name: "workspace.write_file".to_string(),
            description:
                "Write a generated artifact file under the visible agent-workspace directory."
                    .to_string(),
            effects: vec![ToolEffect::Write],
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path under agent-workspace, such as cover-image/cover.svg."
                    },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            })),
        },
        ToolSpec {
            name: "workspace.append_file".to_string(),
            description:
                "Append generated artifact content under agent-workspace. Use after workspace.write_file for large HTML/PPT files."
                    .to_string(),
            effects: vec![ToolEffect::Write],
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path under agent-workspace, matching the file being appended."
                    },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            })),
        },
        ToolSpec {
            name: "shell.exec".to_string(),
            description:
                "Run a sandboxed shell command requested by an active skill instruction."
                    .to_string(),
            effects: vec![ToolEffect::Read, ToolEffect::Process],
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "timeoutSeconds": { "type": "integer", "minimum": 1, "maximum": SHELL_EXEC_TIMEOUT_SECS }
                },
                "required": ["command"]
            })),
        },
    ]
}

fn tool_query<'a>(input: &'a Value, tool: &str) -> Result<&'a str, String> {
    input
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .ok_or_else(|| format!("{tool} requires query"))
}

fn tool_top_k(input: &Value) -> usize {
    input
        .get("topK")
        .or_else(|| input.get("top_k"))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(5)
        .clamp(1, 10)
}

fn write_wiki_page_with_activity(
    project_path: &Path,
    rel_path: &str,
    content: &str,
    allow_overwrite: bool,
) -> Result<WikiWriteOutput, String> {
    if content.len() > MAX_WRITE_PAGE_BYTES {
        return Err("wiki.write_page content is too large".to_string());
    }
    let rel = normalize_wiki_write_path(rel_path)?;
    let path = safe_project_join(project_path, &rel)?;
    if let Some(parent) = path.parent() {
        // Check the deepest existing ancestor before creating directories. If a
        // project already contains a symlink under `wiki/`, this prevents even
        // empty intermediate directories from being created outside the project.
        ensure_existing_ancestor_bound(project_path, parent)?;
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create wiki page directory: {err}"))?;
        ensure_project_bound_path(project_path, parent)?;
    }
    // Create-only by default. Prompt injection in retrieved context must not be
    // able to silently truncate an existing wiki page.
    if path.exists() && !allow_overwrite {
        return Err(
            "wiki.write_page refuses to overwrite an existing page without allowOverwrite=true"
                .to_string(),
        );
    }
    let existed_before = path.is_file();
    let previous_content = workspace_rollback_snapshot(&path);
    fs::write(&path, content).map_err(|err| format!("Failed to write wiki page: {err}"))?;
    Ok(WikiWriteOutput {
        reference: AgentReference {
            title: extract_markdown_title(content).unwrap_or_else(|| {
                Path::new(&rel)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Wiki page")
                    .replace('-', " ")
            }),
            path: rel.clone(),
            kind: "wiki".to_string(),
            snippet: Some(trim_text(&collapse_markdown_preview(content), 500))
                .filter(|value| !value.trim().is_empty()),
            score: None,
        },
        existed_before,
        previous_content,
    })
}

fn write_workspace_file(
    project_path: &Path,
    rel_path: &str,
    content: &str,
) -> Result<WorkspaceWriteOutput, String> {
    if content.len() > MAX_WORKSPACE_WRITE_BYTES {
        return Err("workspace.write_file content is too large".to_string());
    }
    let (rel, path) =
        resolve_workspace_write_target(project_path, rel_path, "workspace.write_file")?;
    if path
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err("workspace.write_file refuses to overwrite a symlink".to_string());
    }
    let existed_before = path.is_file();
    let previous_content = workspace_rollback_snapshot(&path);
    fs::write(&path, content).map_err(|err| format!("workspace.write_file failed: {err}"))?;
    Ok(WorkspaceWriteOutput {
        path: format!("{AGENT_WORKSPACE_DIR}/{rel}"),
        bytes: content.len(),
        existed_before,
        previous_content,
    })
}

fn append_workspace_file(
    project_path: &Path,
    rel_path: &str,
    content: &str,
) -> Result<WorkspaceWriteOutput, String> {
    if content.len() > MAX_WORKSPACE_WRITE_BYTES {
        return Err("workspace.append_file content is too large".to_string());
    }
    let (rel, path) =
        resolve_workspace_write_target(project_path, rel_path, "workspace.append_file")?;
    if path
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err("workspace.append_file refuses to overwrite a symlink".to_string());
    }
    let existed_before = path.is_file();
    let previous_content = workspace_rollback_snapshot(&path);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| {
            use std::io::Write;
            file.write_all(content.as_bytes())
        })
        .map_err(|err| format!("workspace.append_file failed: {err}"))?;
    let bytes = fs::metadata(&path)
        .map(|metadata| metadata.len() as usize)
        .unwrap_or(content.len());
    Ok(WorkspaceWriteOutput {
        path: format!("{AGENT_WORKSPACE_DIR}/{rel}"),
        bytes,
        existed_before,
        previous_content,
    })
}

fn workspace_rollback_snapshot(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_WORKSPACE_ROLLBACK_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

async fn run_shell_exec(
    context: &ToolContext<'_>,
    command: &str,
    timeout_secs: u64,
) -> Result<ShellExecToolOutput, String> {
    let command = command.trim();
    if command.is_empty() {
        return Err("shell.exec command is empty".to_string());
    }
    if command.chars().count() > MAX_SHELL_COMMAND_CHARS {
        return Err("shell.exec command is too long".to_string());
    }
    let project_path = context.project_root.as_path().to_path_buf();
    if !project_path.is_dir() {
        return Err("shell.exec project directory is not available".to_string());
    }
    let workspace = agent_workspace_path(&project_path);
    let files = tokio::task::spawn_blocking(move || collect_workspace_sync_files(&workspace))
        .await
        .map_err(|err| format!("shell.exec worker failed: {err}"))??;

    let response = context
        .state
        .executor
        .exec_shell(ShellExecRequest {
            command: command.to_string(),
            timeout_seconds: timeout_secs,
            files,
        })
        .await?;

    let stdout = trim_shell_output(&response.stdout);
    let stderr = trim_shell_output(&response.stderr);
    let changed = response.files;
    let generated_files = {
        let project_path = project_path.clone();
        tokio::task::spawn_blocking(move || write_shell_generated_files(&project_path, changed))
            .await
            .map_err(|err| format!("shell.exec worker failed: {err}"))??
    };

    Ok(ShellExecToolOutput {
        command: command.to_string(),
        exit_code: response.exit_code,
        stdout,
        stderr,
        timed_out: response.timed_out,
        generated_files,
    })
}

/// Upload snapshot of the agent workspace for the sandbox run. Bounded so a
/// large workspace cannot turn every shell command into a huge transfer.
fn collect_workspace_sync_files(workspace: &Path) -> Result<Vec<ShellExecFile>, String> {
    let mut files = Vec::new();
    if !workspace.is_dir() {
        return Ok(files);
    }
    let mut stack = vec![workspace.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .map_err(|err| format!("shell.exec failed to read workspace: {err}"))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("shell.exec failed to read workspace: {err}"))?;
            let path = entry.path();
            let meta = match fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push(path);
                continue;
            }
            if !meta.is_file() || meta.len() as usize > MAX_WORKSPACE_WRITE_BYTES {
                continue;
            }
            if files.len() >= MAX_SHELL_SYNC_FILES {
                return Ok(files);
            }
            let Ok(rel) = path.strip_prefix(workspace) else {
                continue;
            };
            let Some(rel) = rel.to_str().map(|value| value.replace('\\', "/")) else {
                continue;
            };
            let bytes =
                fs::read(&path).map_err(|err| format!("shell.exec failed to read {rel}: {err}"))?;
            files.push(ShellExecFile {
                path: rel,
                content_b64: BASE64_STANDARD.encode(bytes),
            });
        }
    }
    Ok(files)
}

/// Write files the sandbox command created or changed back into the project's
/// agent workspace, through the same path sandbox as workspace.write_file.
fn write_shell_generated_files(
    project_path: &Path,
    changed: Vec<ShellExecFile>,
) -> Result<Vec<WorkspaceWriteOutput>, String> {
    let mut outputs = Vec::new();
    for file in changed.into_iter().take(MAX_SHELL_GENERATED_FILES) {
        let bytes = BASE64_STANDARD
            .decode(file.content_b64.as_bytes())
            .map_err(|err| format!("shell.exec returned malformed file {}: {err}", file.path))?;
        if bytes.len() > MAX_WORKSPACE_WRITE_BYTES {
            continue;
        }
        let (rel, path) = resolve_workspace_write_target(project_path, &file.path, "shell.exec")?;
        if path
            .symlink_metadata()
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err("shell.exec refuses to overwrite a symlink".to_string());
        }
        let existed_before = path.is_file();
        let previous_content = workspace_rollback_snapshot(&path);
        fs::write(&path, &bytes).map_err(|err| format!("shell.exec failed to write {rel}: {err}"))?;
        outputs.push(WorkspaceWriteOutput {
            path: format!("{AGENT_WORKSPACE_DIR}/{rel}"),
            bytes: bytes.len(),
            existed_before,
            previous_content,
        });
    }
    Ok(outputs)
}

fn trim_shell_output(output: &str) -> String {
    if output.chars().count() <= MAX_SHELL_OUTPUT_CHARS {
        return output.to_string();
    }
    let truncated: String = output.chars().take(MAX_SHELL_OUTPUT_CHARS).collect();
    format!("{truncated}...")
}

fn resolve_workspace_write_target(
    project_path: &Path,
    rel_path: &str,
    tool_name: &str,
) -> Result<(String, PathBuf), String> {
    let rel = normalize_workspace_write_path(rel_path)
        .map_err(|err| err.replace("workspace.write_file", tool_name))?;
    if !project_path.is_dir() {
        return Err(format!("{tool_name} project directory is not available"));
    }
    let workspace = agent_workspace_path(project_path);
    fs::create_dir_all(&workspace)
        .map_err(|err| format!("{tool_name} failed to create workspace: {err}"))?;
    ensure_project_bound_path(project_path, &workspace)?;
    let path = workspace.join(&rel);
    if let Some(parent) = path.parent() {
        ensure_existing_ancestor_bound(project_path, parent)?;
        fs::create_dir_all(parent)
            .map_err(|err| format!("{tool_name} failed to create directory: {err}"))?;
        ensure_project_bound_path(project_path, parent)?;
    }
    Ok((rel, path))
}

async fn run_wiki_search(
    state: &AppState,
    project_id: &str,
    project_root: &ProjectRoot,
    query: &str,
    top_k: usize,
    include_content: bool,
) -> Result<WikiSearchToolOutput, String> {
    let search = search_project_hybrid(
        state,
        project_id,
        project_root,
        query,
        SearchOptions {
            top_k,
            include_content,
        },
    )
    .await
    .map_err(|err| format!("wiki.search failed: {err}"))?;
    let references = search
        .results
        .iter()
        .map(|result| AgentReference {
            title: result.title.clone(),
            path: result.path.clone(),
            kind: "wiki".to_string(),
            snippet: Some(result.snippet.clone()).filter(|s| !s.trim().is_empty()),
            score: Some(result.score),
        })
        .collect();
    Ok(WikiSearchToolOutput {
        mode: search.mode,
        token_hits: search.token_hits,
        vector_hits: search.vector_hits,
        references,
    })
}

async fn run_web_search(
    state: &AppState,
    query: &str,
    top_k: usize,
) -> Result<Vec<AgentReference>, String> {
    let config = load_web_search_config(state)
        .await
        .map_err(|err| format!("web.search failed to load configuration: {err}"))?
        .ok_or_else(|| "web.search is not configured; set a search provider in Settings".to_string())?;
    let results = timeout(
        Duration::from_secs(WEB_SEARCH_TIMEOUT_SECS),
        web_search(&config, query, top_k),
    )
    .await
    .map_err(|_| format!("web.search timed out after {WEB_SEARCH_TIMEOUT_SECS}s"))?
    .map_err(|err| format!("web.search failed: {err}"))?;
    Ok(results
        .into_iter()
        .take(top_k)
        .map(|result| AgentReference {
            title: if result.title.trim().is_empty() {
                result.url.clone()
            } else {
                result.title
            },
            path: result.url,
            kind: "web".to_string(),
            snippet: Some(result.snippet).filter(|s| !s.trim().is_empty()),
            score: None,
        })
        .collect())
}

pub fn read_wiki_page(project_path: &Path, rel_path: &str) -> Result<String, String> {
    let rel = normalize_rel_path(rel_path);
    if !is_public_read_rel(&rel) || !rel.to_ascii_lowercase().starts_with("wiki/") {
        return Err("wiki.read_page path must stay under wiki/".to_string());
    }
    let path = safe_project_join(project_path, &rel)?;
    let meta = fs::metadata(&path).map_err(|err| format!("Failed to read page metadata: {err}"))?;
    if !meta.is_file() {
        return Err("wiki.read_page path is not a file".to_string());
    }
    if meta.len() as usize > MAX_READ_PAGE_BYTES {
        return Err("wiki.read_page file is too large".to_string());
    }
    fs::read_to_string(path).map_err(|err| format!("Failed to read wiki page: {err}"))
}

fn search_graph(
    project_path: &Path,
    query: &str,
    top_k: usize,
) -> Result<Vec<AgentReference>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let top_k = top_k.clamp(1, 10);
    let (seeds, _edges) = build_graph_view(project_path, Some(query), None, Some(top_k))
        .map_err(|err| format!("graph.search failed: {err}"))?;
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    let mut refs = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for seed in &seeds {
        if seen.insert(seed.path.clone()) {
            refs.push(AgentReference {
                title: seed.label.clone(),
                path: seed.path.clone(),
                kind: "graph".to_string(),
                snippet: Some(format!(
                    "matched entity; {} related link(s)",
                    seed.link_count
                )),
                score: Some(10_000.0 + seed.link_count as f64),
            });
        }
        let Ok(neighborhood) = neighbors_for_node(project_path, &seed.id) else {
            continue;
        };
        for neighbor in neighborhood.neighbors {
            if !seen.insert(neighbor.path.clone()) {
                continue;
            }
            refs.push(AgentReference {
                title: neighbor.label,
                path: neighbor.path,
                kind: "graph".to_string(),
                snippet: Some(format!(
                    "direct neighbor; {} related link(s)",
                    neighbor.link_count
                )),
                score: Some(5_000.0 + neighbor.link_count as f64),
            });
        }
    }
    refs.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });
    refs.truncate(top_k);
    Ok(refs)
}

fn search_sources(
    project_path: &Path,
    query: &str,
    top_k: usize,
) -> Result<Vec<AgentReference>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("source.search query is required".to_string());
    }
    let root = project_path.join("raw").join("sources");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let lower_query = query.to_lowercase();
    let query_terms = source_query_terms(&lower_query);
    let mut files = Vec::new();
    let mut seen_files = 0usize;
    collect_source_files(&root, &mut files, &mut seen_files);
    if seen_files > MAX_SOURCE_SEARCH_FILES {
        tracing::warn!(
            "source.search stopped after {MAX_SOURCE_SEARCH_FILES} files in {}",
            project_path.display()
        );
    }
    let top_k = top_k.clamp(1, 10);
    let mut refs = Vec::new();
    for file in files {
        let Some(ext) = file
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
        else {
            continue;
        };
        if !matches!(
            ext.as_str(),
            "md" | "markdown" | "txt" | "json" | "csv" | "tsv" | "yaml" | "yml" | "xml" | "html"
        ) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        let lower = content.to_lowercase();
        let matched = std::iter::once(lower_query.as_str())
            .chain(query_terms.iter().map(String::as_str))
            .find_map(|term| lower.find(term).map(|idx| (idx, term.len())));
        let Some((byte_idx, _matched_len)) = matched else {
            continue;
        };
        let rel = relative_to_project(project_path, &file);
        if is_hidden_rel(&rel) {
            continue;
        }
        refs.push(AgentReference {
            title: file
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&rel)
                .to_string(),
            path: rel,
            kind: "source".to_string(),
            snippet: Some(snippet_around_byte(
                &content,
                byte_idx,
                MAX_SOURCE_SNIPPET_CHARS,
            )),
            score: None,
        });
        if refs.len() >= top_k {
            break;
        }
    }
    Ok(refs)
}

fn collect_source_files(dir: &Path, files: &mut Vec<PathBuf>, seen: &mut usize) {
    if *seen > MAX_SOURCE_SEARCH_FILES {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        // file_type() does not follow symlinks, so symlinked files and
        // directories are skipped and cannot escape the sources tree.
        if file_type.is_dir() {
            collect_source_files(&entry.path(), files, seen);
        } else if file_type.is_file() {
            *seen += 1;
            if *seen > MAX_SOURCE_SEARCH_FILES {
                return;
            }
            files.push(entry.path());
        }
    }
}

fn source_query_terms(query: &str) -> Vec<String> {
    query
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | '，' | ';' | '；' | ':' | '：'))
        .map(str::trim)
        .filter(|term| term.chars().count() >= 2)
        .filter(|term| {
            !matches!(
                *term,
                "raw"
                    | "source"
                    | "sources"
                    | "file"
                    | "files"
                    | "原始资料"
                    | "原始文件"
                    | "源文件"
            )
        })
        .map(ToString::to_string)
        .collect()
}

fn safe_project_join(project_path: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute()
        || rel_path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err("path must be project-relative".to_string());
    }
    let joined = project_path.join(rel_path);
    if joined.exists() {
        let root_canon = project_path
            .canonicalize()
            .map_err(|err| format!("Failed to resolve project path: {err}"))?;
        let joined_canon = joined
            .canonicalize()
            .map_err(|err| format!("Failed to resolve requested path: {err}"))?;
        if !joined_canon.starts_with(root_canon) {
            return Err("path escapes project directory".to_string());
        }
    }
    Ok(joined)
}

fn ensure_existing_ancestor_bound(project_path: &Path, path: &Path) -> Result<(), String> {
    let mut cursor = path;
    while !cursor.exists() {
        cursor = cursor
            .parent()
            .ok_or_else(|| "path must have an existing project ancestor".to_string())?;
    }
    ensure_project_bound_path(project_path, cursor)
}

fn ensure_project_bound_path(project_path: &Path, path: &Path) -> Result<(), String> {
    let root_canon = project_path
        .canonicalize()
        .map_err(|err| format!("Failed to resolve project path: {err}"))?;
    let path_canon = path
        .canonicalize()
        .map_err(|err| format!("Failed to resolve requested path: {err}"))?;
    if !path_canon.starts_with(root_canon) {
        return Err("path escapes project directory".to_string());
    }
    Ok(())
}

fn is_public_read_rel(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    if lower.split('/').any(|segment| segment.starts_with('.')) {
        return false;
    }
    lower == "overview.md"
        || lower == "schema.md"
        || lower.starts_with("wiki/")
        || lower.starts_with("raw/sources/")
}

fn is_hidden_rel(rel: &str) -> bool {
    normalize_rel_path(rel)
        .split('/')
        .any(|segment| segment.starts_with('.'))
}

fn normalize_rel_path(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}

fn normalize_wiki_write_path(path: &str) -> Result<String, String> {
    let rel = normalize_rel_path(path);
    let lower = rel.to_ascii_lowercase();
    if !lower.starts_with("wiki/") || !lower.ends_with(".md") {
        return Err("wiki.write_page path must be a Markdown file under wiki/".to_string());
    }
    if lower.split('/').any(|segment| segment.starts_with('.')) {
        return Err("wiki.write_page cannot write hidden paths".to_string());
    }
    let rel_path = Path::new(&rel);
    if rel_path.is_absolute()
        || rel_path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err("wiki.write_page path must stay inside the project".to_string());
    }
    for segment in rel.split('/') {
        validate_portable_path_segment(segment)?;
    }
    Ok(rel)
}

fn normalize_workspace_write_path(path: &str) -> Result<String, String> {
    let rel = normalize_rel_path(path);
    let lower = rel.to_ascii_lowercase();
    if rel.is_empty()
        || lower.starts_with("wiki/")
        || lower.starts_with("raw/")
        || lower.split('/').any(|segment| segment.starts_with('.'))
    {
        return Err(
            "workspace.write_file path must be a relative file under agent-workspace".to_string(),
        );
    }
    let rel_path = Path::new(&rel);
    if rel_path.is_absolute()
        || rel_path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err("workspace.write_file path must stay inside agent-workspace".to_string());
    }
    for segment in rel.split('/') {
        validate_workspace_path_segment(segment)?;
    }
    Ok(rel)
}

fn validate_workspace_path_segment(segment: &str) -> Result<(), String> {
    validate_portable_path_segment(segment)
        .map_err(|err| err.replace("wiki.write_page", "workspace.write_file"))
}

fn validate_portable_path_segment(segment: &str) -> Result<(), String> {
    if segment.is_empty() {
        return Err("wiki.write_page path contains an empty segment".to_string());
    }
    if segment.ends_with([' ', '.']) {
        return Err(
            "wiki.write_page path contains a segment ending with a space or dot, which is not portable to Windows"
                .to_string(),
        );
    }
    if segment
        .chars()
        .any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*') || ch <= '\u{1f}')
    {
        return Err(
            "wiki.write_page path contains characters that are invalid on Windows".to_string(),
        );
    }
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    if matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        return Err("wiki.write_page path uses a Windows reserved device name".to_string());
    }
    Ok(())
}

fn extract_markdown_title(content: &str) -> Option<String> {
    for line in content.lines().take(80) {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("title:") {
            let title = title.trim().trim_matches('"').trim_matches('\'');
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
        if let Some(heading) = trimmed.strip_prefix("# ") {
            let heading = heading.trim();
            if !heading.is_empty() {
                return Some(heading.to_string());
            }
        }
    }
    None
}

fn collapse_markdown_preview(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && trimmed != "---" && !trimmed.starts_with("title:")
        })
        .take(12)
        .collect::<Vec<_>>()
        .join(" ")
}

fn relative_to_project(project_path: &Path, path: &Path) -> String {
    path.strip_prefix(project_path)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}

fn snippet_around_byte(content: &str, byte_idx: usize, max_chars: usize) -> String {
    let char_idx = content[..byte_idx.min(content.len())].chars().count();
    let start = char_idx.saturating_sub(max_chars / 2);
    let mut snippet = content
        .chars()
        .skip(start)
        .take(max_chars)
        .collect::<String>();
    if start > 0 {
        snippet.insert_str(0, "...");
    }
    if content.chars().count() > start + max_chars {
        snippet.push_str("...");
    }
    snippet.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn trim_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        format!("{}...", value.chars().take(max_chars).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "knowledge-agent-tools-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("wiki")).unwrap();
        root
    }

    #[test]
    fn wiki_write_rejects_traversal_and_non_wiki_paths() {
        assert!(normalize_wiki_write_path("../evil.md").is_err());
        assert!(normalize_wiki_write_path("wiki/../evil.md").is_err());
        assert!(normalize_wiki_write_path("notes/page.md").is_err());
        assert!(normalize_wiki_write_path("wiki/.hidden/page.md").is_err());
        assert!(normalize_wiki_write_path("wiki/page.txt").is_err());
        assert!(normalize_wiki_write_path("wiki/CON.md").is_err());
        assert!(normalize_wiki_write_path("wiki/dir. /page.md").is_err());
        assert_eq!(
            normalize_wiki_write_path("wiki\\topic\\Page.md").unwrap(),
            "wiki/topic/Page.md"
        );
    }

    #[test]
    fn workspace_write_rejects_wiki_raw_and_hidden_paths() {
        assert!(normalize_workspace_write_path("wiki/page.md").is_err());
        assert!(normalize_workspace_write_path("raw/sources/file.txt").is_err());
        assert!(normalize_workspace_write_path(".hidden/file.txt").is_err());
        assert!(normalize_workspace_write_path("../escape.txt").is_err());
        assert!(normalize_workspace_write_path("").is_err());
        assert_eq!(
            normalize_workspace_write_path("cover-image/cover.svg").unwrap(),
            "cover-image/cover.svg"
        );
    }

    #[test]
    fn wiki_write_page_is_create_only_by_default() {
        let root = temp_project();
        let first = write_wiki_page_with_activity(&root, "wiki/alpha.md", "# Alpha", false)
            .expect("first write succeeds");
        assert!(!first.existed_before);
        assert!(first.previous_content.is_none());
        assert_eq!(first.reference.title, "Alpha");
        assert_eq!(first.reference.path, "wiki/alpha.md");

        let denied = write_wiki_page_with_activity(&root, "wiki/alpha.md", "# Replaced", false);
        assert!(denied.is_err());

        let overwritten = write_wiki_page_with_activity(&root, "wiki/alpha.md", "# Replaced", true)
            .expect("overwrite with allowOverwrite");
        assert!(overwritten.existed_before);
        assert_eq!(overwritten.previous_content.as_deref(), Some("# Alpha"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn workspace_write_and_append_stay_in_workspace() {
        let root = temp_project();
        let written = write_workspace_file(&root, "report/summary.txt", "hello").unwrap();
        assert_eq!(written.path, "agent-workspace/report/summary.txt");
        assert_eq!(written.bytes, 5);
        assert!(!written.existed_before);

        let appended = append_workspace_file(&root, "report/summary.txt", " world").unwrap();
        assert!(appended.existed_before);
        assert_eq!(appended.previous_content.as_deref(), Some("hello"));
        let content =
            fs::read_to_string(root.join("agent-workspace/report/summary.txt")).unwrap();
        assert_eq!(content, "hello world");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn read_wiki_page_rejects_paths_outside_wiki() {
        let root = temp_project();
        fs::write(root.join("wiki/alpha.md"), "# Alpha body").unwrap();
        fs::create_dir_all(root.join(".knowledge")).unwrap();
        fs::write(root.join(".knowledge/secret.md"), "secret").unwrap();

        assert_eq!(
            read_wiki_page(&root, "wiki/alpha.md").unwrap(),
            "# Alpha body"
        );
        assert!(read_wiki_page(&root, ".knowledge/secret.md").is_err());
        assert!(read_wiki_page(&root, "../outside.md").is_err());
        assert!(read_wiki_page(&root, "overview.md").is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn source_search_finds_keyword_snippets() {
        let root = temp_project();
        fs::create_dir_all(root.join("raw/sources/docs")).unwrap();
        fs::write(
            root.join("raw/sources/docs/notes.txt"),
            "alpha beta gamma coal-mine safety regulation details",
        )
        .unwrap();
        fs::write(root.join("raw/sources/docs/skip.bin"), "binary").unwrap();

        let refs = search_sources(&root, "coal-mine safety", 5).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, "source");
        assert_eq!(refs[0].path, "raw/sources/docs/notes.txt");
        assert!(refs[0].snippet.as_deref().unwrap().contains("coal-mine"));

        let empty = search_sources(&root, "zz-no-match-zz", 5).unwrap();
        assert!(empty.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn graph_search_returns_matched_entities_and_neighbors() {
        let root = temp_project();
        fs::write(
            root.join("wiki/alpha.md"),
            "# Alpha\n\nLinks to [[beta]] for details.",
        )
        .unwrap();
        fs::write(root.join("wiki/beta.md"), "# Beta\n\nStandalone page.").unwrap();

        let refs = search_graph(&root, "alpha", 5).unwrap();
        assert!(!refs.is_empty());
        assert_eq!(refs[0].kind, "graph");
        assert_eq!(refs[0].path, "wiki/alpha.md");
        assert!(refs[0].score.unwrap() >= 10_000.0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn markdown_title_prefers_frontmatter_then_heading() {
        assert_eq!(
            extract_markdown_title("---\ntitle: \"From Frontmatter\"\n---\n# Heading"),
            Some("From Frontmatter".to_string())
        );
        assert_eq!(
            extract_markdown_title("intro\n# Heading Title"),
            Some("Heading Title".to_string())
        );
        assert_eq!(extract_markdown_title("no title here"), None);
    }

    #[test]
    fn tool_top_k_clamps_and_defaults() {
        assert_eq!(tool_top_k(&json!({})), 5);
        assert_eq!(tool_top_k(&json!({"topK": 3})), 3);
        assert_eq!(tool_top_k(&json!({"top_k": 99})), 10);
        assert_eq!(tool_top_k(&json!({"topK": 0})), 1);
    }

    #[test]
    fn trim_shell_output_truncates_past_limit() {
        let short = "a".repeat(MAX_SHELL_OUTPUT_CHARS);
        assert_eq!(trim_shell_output(&short), short);

        let long = "b".repeat(MAX_SHELL_OUTPUT_CHARS + 5);
        let trimmed = trim_shell_output(&long);
        assert_eq!(trimmed.chars().count(), MAX_SHELL_OUTPUT_CHARS + 3);
        assert!(trimmed.ends_with("..."));
        assert!(trimmed.starts_with('b'));
    }
}
