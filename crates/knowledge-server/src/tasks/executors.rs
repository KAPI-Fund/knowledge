use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::providers::{OpenAiCompatibleProvider, ProviderError, ProviderTextRequest};
use crate::projects::audit::{append_audit_log, CreateAuditLog};
use crate::projects::service::project_root_for_id;
use crate::query::{execute_project_query, ExecuteProjectQueryInput, QueryExecutionError};
use crate::tasks::model::TaskRecord;
use crate::tasks::store;
use knowledge_core::ingest::{
  AnalysisResult, analyze_source, build_analysis_prompt, build_analysis_user_prompt,
  build_generation_prompt, build_generation_user_prompt, build_review_suggestion_prompt,
  check_ingest_cache, generate_wiki_from_analysis, parse_generation_file_blocks,
  render_generation_file_blocks, should_run_dedicated_review_stage, source_summary_path,
};
use knowledge_core::project::page_merge::{
  build_page_merge_prompts, finalize_page_merge, prepare_page_merge, PageMergePlan,
};
use knowledge_core::project::enrich::{apply_enrich_links, build_enrich_prompt, parse_enrich_response};
use knowledge_core::project::lint::{
  build_semantic_lint_prompt, parse_semantic_lint_response, run_structural_lint,
};
use knowledge_core::project::queries::{save_query_page, SaveQueryPageInput, SavedQueryCitation};
use knowledge_core::project::reviews::{
  build_review_sweep_prompt, parse_review_resolution_ids, resolve_review_ids,
  sweep_resolved_reviews, update_review_status,
};
use knowledge_core::project::source_text::read_source_text;
use knowledge_core::project::sources::{delete_source, import_source, rescan_sources};

const REVIEW_SWEEP_MAX_PAGES: usize = 300;
const REVIEW_SWEEP_SYSTEM_PROMPT: &str = "You judge whether stale wiki review items have already been resolved by the current wiki state. Return JSON only.";

pub async fn run_task_executor(state: &AppState, task: &TaskRecord) -> Result<(), ApiError> {
  let task = store::mark_task_running_attempt(state, &task.id).await?;

  let result: Result<Value, TaskExecutionError> = match task.task_type.as_str() {
    "query.answer" => execute_query(state, &task).await,
    "query.save_answer" => run_save_query_answer_executor(state, &task).await.map_err(Into::into),
    "project.import_source" => run_import_source_executor(state, &task).await.map_err(Into::into),
    "project.rescan_sources" => run_rescan_sources_executor(state, &task).await.map_err(Into::into),
    "project.delete_source" => run_delete_source_executor(state, &task).await.map_err(Into::into),
    "project.run_lint" => run_lint_executor(state, &task).await.map_err(Into::into),
    "project.ingest_source" => run_ingest_source_executor(state, &task).await.map_err(Into::into),
    "project.sweep_reviews" => run_manual_review_sweep_executor(state, &task).await.map_err(Into::into),
    "project.update_review" => run_update_review_executor(state, &task).await.map_err(Into::into),
    _ => Err(TaskExecutionError::from(ApiError::bad_request("unsupported task type"))),
  };

  match result {
    Ok(value) => {
      store::complete_task(state, &task.id, value).await?;
      append_task_audit_log(state, &task, "task.succeeded", "Task completed").await?;
      Ok(())
    }
    Err(error) => {
      let retryable = error.retryable() && task.attempt_count < task.max_attempts;
      let error_payload = error.payload(retryable);

      if retryable {
        let next_retry_at = OffsetDateTime::now_utc()
          .checked_add(Duration::seconds(2))
          .ok_or_else(|| ApiError::internal("failed to compute retry timestamp"))?
          .format(&Rfc3339)
          .map_err(|_| ApiError::internal("failed to format retry timestamp"))?;
        store::retry_task(state, &task.id, error_payload.clone(), next_retry_at).await?;
        append_task_audit_log(state, &task, "task.retry_waiting", "Task scheduled for retry").await?;
      } else {
        store::fail_task(state, &task.id, error_payload.clone()).await?;
        append_task_audit_log(state, &task, "task.failed", "Task failed").await?;
      }

      Err(error.into_api_error())
    }
  }
}

pub async fn run_query_executor(state: &AppState, task: &TaskRecord) -> Result<(), ApiError> {
  match execute_query(state, task).await {
    Ok(result) => {
      store::complete_task(state, &task.id, result).await?;
      Ok(())
    }
    Err(error) => {
      let error_payload = error.payload(false);
      store::fail_task(state, &task.id, error_payload).await?;
      Err(error.into_api_error())
    }
  }
}

async fn execute_query(state: &AppState, task: &TaskRecord) -> Result<Value, TaskExecutionError> {
  let query = read_string(&task.payload, "query")?;
  let top_k = read_i64(&task.payload, "topK").unwrap_or(3).max(1) as usize;
  let language = task
    .payload
    .get("language")
    .and_then(Value::as_str)
    .filter(|value| !value.trim().is_empty())
    .unwrap_or("en")
    .to_string();
  execute_project_query(
    state,
    &task.project_id,
    ExecuteProjectQueryInput {
      query,
      top_k: Some(top_k),
      language: Some(language),
    },
  )
  .await
  .map_err(TaskExecutionError::from)
}

#[derive(Debug)]
struct TaskExecutionError {
  api_error: ApiError,
  code: String,
  retryable: bool,
  provider_status: Option<u16>,
}

impl TaskExecutionError {
  fn from_provider_error(error: ProviderError) -> Self {
    let api_error = if error.retryable() {
      ApiError::internal(error.message().to_string())
    } else {
      ApiError::bad_request(error.message().to_string())
    };

    Self {
      api_error,
      code: error.code().to_string(),
      retryable: error.retryable(),
      provider_status: error.provider_status(),
    }
  }

  fn retryable(&self) -> bool {
    self.retryable
  }

  fn payload(&self, retryable: bool) -> Value {
    let mut payload = json!({
      "code": self.code,
      "message": self.api_error.to_string(),
      "retryable": retryable
    });

    if let Some(provider_status) = self.provider_status {
      payload["providerStatus"] = json!(provider_status);
    }

    payload
  }

  fn into_api_error(self) -> ApiError {
    self.api_error
  }
}

impl From<ApiError> for TaskExecutionError {
  fn from(api_error: ApiError) -> Self {
    Self {
      api_error,
      code: "task_execution_failed".to_string(),
      retryable: false,
      provider_status: None,
    }
  }
}

impl From<QueryExecutionError> for TaskExecutionError {
  fn from(error: QueryExecutionError) -> Self {
    let code = error.code().to_string();
    let retryable = error.retryable();
    let provider_status = error.provider_status();
    Self {
      api_error: error.into_api_error(),
      code,
      retryable,
      provider_status,
    }
  }
}

async fn run_import_source_executor(state: &AppState, task: &TaskRecord) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let file_name = read_string(&task.payload, "fileName")?;
  let content_base64 = read_string(&task.payload, "contentBase64")?;
  let source = import_source(&root, &file_name, &content_base64)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;

  Ok(json!({
    "relativePath": source.relative_path,
    "size": source.size
  }))
}

async fn run_save_query_answer_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let source_task_id = read_string(&task.payload, "sourceTaskId")?;
  let title = read_string(&task.payload, "title")?;
  let slug = read_string(&task.payload, "slug")?;
  let answer = read_string(&task.payload, "answer")?;
  let context_summary = read_optional_string(&task.payload, "contextSummary").unwrap_or_default();
  let citations = read_saved_query_citations(&task.payload)?;
  let result = save_query_page(
    &root,
    SaveQueryPageInput {
      title: title.clone(),
      slug,
      answer,
      citations,
      context_summary,
    },
  )
  .map_err(|error| ApiError::bad_request(error.to_string()))?;

  if let Some(provider) = load_ingest_provider(state).await? {
    let page_path = root
      .safe_join(&result.relative_path)
      .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let page_content = std::fs::read_to_string(&page_path)
      .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let index_content = std::fs::read_to_string(root.as_path().join("wiki/index.md"))
      .unwrap_or_default();
    let prompt = build_enrich_prompt(&index_content, &page_content);
    let response = provider
      .complete_text(ProviderTextRequest {
        system_prompt: prompt.system_prompt,
        user_prompt: prompt.user_prompt,
      })
      .await
      .map_err(TaskExecutionError::from_provider_error)
      .map_err(TaskExecutionError::into_api_error)?;
    let enriched = apply_enrich_links(&page_content, &parse_enrich_response(&response.text));
    if enriched != page_content {
      std::fs::write(&page_path, enriched)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }
  }

  let _ = append_audit_log(
    state,
    CreateAuditLog {
      project_id: Some(task.project_id.clone()),
      actor_id: task.created_by.clone(),
      action: "query.page.saved".to_string(),
      target_type: "query".to_string(),
      target_id: result.relative_path.clone(),
      task_id: Some(task.id.clone()),
      summary: format!("Saved query page {title}"),
      metadata: json!({
        "relativePath": result.relative_path,
        "sourceTaskId": source_task_id
      }),
    },
  )
  .await?;

  Ok(json!({
    "relativePath": result.relative_path,
    "sourceTaskId": source_task_id,
    "title": title
  }))
}

async fn run_rescan_sources_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let discovered_count =
    rescan_sources(&root).map_err(|error| ApiError::bad_request(error.to_string()))?;

  Ok(json!({
    "discoveredCount": discovered_count
  }))
}

async fn run_delete_source_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let relative_path = read_string(&task.payload, "relativePath")?;
  delete_source(&root, &relative_path).map_err(|error| ApiError::bad_request(error.to_string()))?;
  Ok(json!({}))
}

async fn run_lint_executor(state: &AppState, task: &TaskRecord) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let mode = read_string(&task.payload, "mode")?;
  if mode == "structural" {
    let result = run_structural_lint(&root).map_err(|error| ApiError::bad_request(error.to_string()))?;
    return serde_json::to_value(result).map_err(|error| ApiError::internal(error.to_string()));
  }

  if mode == "semantic" {
    let provider = load_ingest_provider(state)
      .await?
      .ok_or_else(|| ApiError::bad_request("semantic lint requires an openai-compatible provider"))?;
    let prompt = build_semantic_lint_prompt(&root)
      .map_err(|error| ApiError::bad_request(error.to_string()))?
      .unwrap_or_default();
    let response = provider
      .complete_text(ProviderTextRequest {
        system_prompt: "You are a wiki quality analyst.".to_string(),
        user_prompt: prompt,
      })
      .await
      .map_err(TaskExecutionError::from_provider_error)
      .map_err(TaskExecutionError::into_api_error)?;
    let result = parse_semantic_lint_response(&response.text);
    return serde_json::to_value(result).map_err(|error| ApiError::internal(error.to_string()));
  }

  Err(ApiError::bad_request("unsupported lint mode"))
}

async fn run_ingest_source_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let relative_path = read_string(&task.payload, "relativePath")?;
  let source_path = root
    .safe_join(&relative_path)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  let source_identity = source_identity_from_relative_path(&relative_path);
  let content = read_source_text(&source_path).map_err(|error| ApiError::bad_request(error.to_string()))?;
  let source_name = source_path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| ApiError::bad_request("invalid source name"))?;
  let ingest_provider = load_ingest_provider(state).await?;

  let result = if let Some(cached) = check_ingest_cache(&root, &source_identity, &content)
    .map_err(|error| ApiError::bad_request(error.to_string()))?
  {
    cached
  } else if let Some(provider) = ingest_provider.clone() {
    let purpose = try_read_project_file(&root, "purpose.md");
    let schema = try_read_project_file(&root, "schema.md");
    let index = try_read_project_file(&root, "wiki/index.md");
    let overview = try_read_project_file(&root, "wiki/overview.md");
    let analysis_text = provider
      .complete_text(ProviderTextRequest {
        system_prompt: build_analysis_prompt(&purpose, &index, &content),
        user_prompt: build_analysis_user_prompt(source_name, &content),
      })
      .await
      .map_err(TaskExecutionError::from_provider_error)
      .map_err(TaskExecutionError::into_api_error)?;
    let analysis = AnalysisResult {
      title: analysis_title_from_text(source_name, &content, &analysis_text.text),
      summary_markdown: content.trim().to_string(),
      analysis: analysis_text.text,
    };
    let generation = provider
      .complete_text(ProviderTextRequest {
        system_prompt: build_generation_prompt(
          &schema,
          &purpose,
          &index,
          source_name,
          &overview,
          &content,
          &source_summary_path(&source_identity),
        ),
        user_prompt: build_generation_user_prompt(source_name, &analysis.analysis, &content),
      })
      .await
      .map_err(TaskExecutionError::from_provider_error)
      .map_err(TaskExecutionError::into_api_error)?;
    let review_suggestions = if should_run_dedicated_review_stage(&generation.text) {
      provider
        .complete_text(ProviderTextRequest {
          system_prompt: build_review_suggestion_prompt(
            &purpose,
            &index,
            &source_identity,
            &analysis.analysis,
            &content,
            &generation.text,
          ),
          user_prompt:
            "Emit only high-value REVIEW blocks for follow-up research or unresolved knowledge gaps. Output nothing if there are none."
              .to_string(),
        })
        .await
        .map(|response| response.text)
        .unwrap_or_default()
    } else {
      String::new()
    };
    let combined_generation = if review_suggestions.trim().is_empty() {
      generation.text
    } else {
      format!("{}\n\n{}", generation.text.trim_end(), review_suggestions.trim())
    };
    let merged_generation = merge_generated_pages_with_existing_content(
      &provider,
      &root,
      source_name,
      &combined_generation,
    )
    .await?;

    generate_wiki_from_analysis(
      &root,
      &source_identity,
      source_name,
      &content,
      &analysis,
      &merged_generation,
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?
  } else {
    let analysis = analyze_source(source_name, &content);
    generate_wiki_from_analysis(&root, &source_identity, source_name, &content, &analysis, "")
      .map_err(|error| ApiError::bad_request(error.to_string()))?
  };

  let review_sweep = if should_run_review_sweep(state, &task.project_id, &task.id).await? {
    match run_review_sweep(state, task, &root, ingest_provider.as_ref()).await {
      Ok(Some(result)) => Some(result),
      Ok(None) => None,
      Err(_) => None,
    }
  } else {
    None
  };

  let mut response = json!({
    "summaryPath": result.summary_path,
    "cacheHit": result.cache_hit,
    "writtenPaths": result.written_paths
  });

  if let Some(review_sweep) = review_sweep {
    response["reviewSweep"] = review_sweep;
  }

  Ok(response)
}

async fn run_review_sweep(
  state: &AppState,
  task: &TaskRecord,
  root: &knowledge_core::project::root::ProjectRoot,
  provider: Option<&OpenAiCompatibleProvider>,
) -> Result<Option<Value>, ApiError> {
  let mut result = match sweep_resolved_reviews(root) {
    Ok(result) => result,
    Err(_) => return Ok(None),
  };

  if !result.unresolved_ids.is_empty()
    && let Some(provider) = provider
    && let Ok(Some(prompt)) =
      build_review_sweep_prompt(root, &result.unresolved_ids, REVIEW_SWEEP_MAX_PAGES)
    && let Ok(response) = provider
      .complete_text(ProviderTextRequest {
        system_prompt: REVIEW_SWEEP_SYSTEM_PROMPT.to_string(),
        user_prompt: prompt,
      })
      .await
  {
    let resolved_ids = parse_review_resolution_ids(&response.text, &result.unresolved_ids);
    if !resolved_ids.is_empty() && let Ok(llm_result) = resolve_review_ids(root, &resolved_ids) {
      result
        .resolved_ids
        .extend(llm_result.resolved_ids.iter().cloned());
      result
        .unresolved_ids
        .retain(|review_id| !llm_result.resolved_ids.iter().any(|id| id == review_id));
    }
  }

  if !result.resolved_ids.is_empty() {
    let _ = append_audit_log(
      state,
      CreateAuditLog {
        project_id: Some(task.project_id.clone()),
        actor_id: task.created_by.clone(),
        action: "review.sweep.completed".to_string(),
        target_type: "review".to_string(),
        target_id: "batch".to_string(),
        task_id: Some(task.id.clone()),
        summary: format!(
          "Auto-resolved {} stale review item{}",
          result.resolved_ids.len(),
          if result.resolved_ids.len() == 1 { "" } else { "s" }
        ),
        metadata: json!({
          "resolvedIds": result.resolved_ids,
          "unresolvedIds": result.unresolved_ids
        }),
      },
    )
    .await?;
  }

  Ok(Some(json!({
    "resolvedIds": result.resolved_ids,
    "unresolvedIds": result.unresolved_ids
  })))
}

async fn run_update_review_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let review_id = read_string(&task.payload, "reviewId")?;
  let status = read_string(&task.payload, "status")?;
  let updated =
    update_review_status(&root, &review_id, &status).map_err(|error| ApiError::bad_request(error.to_string()))?;

  Ok(json!({
    "id": updated.id,
    "status": updated.status,
    "title": updated.title,
    "description": updated.description
  }))
}

async fn run_manual_review_sweep_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let provider = load_ingest_provider(state).await?;
  Ok(
    run_review_sweep(state, task, &root, provider.as_ref())
      .await?
      .unwrap_or_else(|| json!({ "resolvedIds": [], "unresolvedIds": [] })),
  )
}

async fn append_task_audit_log(
  state: &AppState,
  task: &TaskRecord,
  action: &str,
  summary: &str,
) -> Result<(), ApiError> {
  let _ = append_audit_log(
    state,
    CreateAuditLog {
      project_id: Some(task.project_id.clone()),
      actor_id: task.created_by.clone(),
      action: action.to_string(),
      target_type: "task".to_string(),
      target_id: task.id.clone(),
      task_id: Some(task.id.clone()),
      summary: format!("{summary}: {}", task.title),
      metadata: json!({
        "taskType": task.task_type,
        "status": task.status
      }),
    },
  )
  .await?;

  Ok(())
}

fn read_string(payload: &Value, key: &str) -> Result<String, ApiError> {
  payload
    .get(key)
    .and_then(Value::as_str)
    .map(str::to_string)
    .ok_or_else(|| ApiError::bad_request(format!("missing {key} payload")))
}

fn read_i64(payload: &Value, key: &str) -> Result<i64, ApiError> {
  payload
    .get(key)
    .and_then(Value::as_i64)
    .ok_or_else(|| ApiError::bad_request(format!("missing {key} payload")))
}

fn read_optional_string(payload: &Value, key: &str) -> Option<String> {
  payload
    .get(key)
    .and_then(Value::as_str)
    .map(str::to_string)
}

fn read_saved_query_citations(payload: &Value) -> Result<Vec<SavedQueryCitation>, ApiError> {
  let Some(items) = payload.get("citations").and_then(Value::as_array) else {
    return Ok(Vec::new());
  };

  items
    .iter()
    .map(|item| {
      let path = item
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("missing citation path"))?;
      let title = item
        .get("title")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("missing citation title"))?;

      Ok(SavedQueryCitation {
        path: path.to_string(),
        title: title.to_string(),
      })
    })
    .collect()
}

async fn should_run_review_sweep(
  state: &AppState,
  project_id: &str,
  current_task_id: &str,
) -> Result<bool, ApiError> {
  let active_ingests = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*)
     FROM project_tasks
     WHERE project_id = $1
       AND task_type = 'project.ingest_source'
       AND status IN ('queued', 'running', 'retry_waiting')
       AND id <> $2",
  )
  .bind(project_id)
  .bind(current_task_id)
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(active_ingests == 0)
}

async fn load_ingest_provider(state: &AppState) -> Result<Option<OpenAiCompatibleProvider>, ApiError> {
  let (
    provider_mode,
    provider_base_url,
    provider_api_key,
    provider_model,
    provider_timeout_seconds,
  ) = sqlx::query_as::<_, (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
  )>(
    "SELECT provider_mode, provider_base_url, provider_api_key, provider_model, provider_timeout_seconds
     FROM system_settings
     WHERE id = 1",
  )
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  if provider_mode != "openai-compatible"
    || provider_base_url.as_deref().unwrap_or("").trim().is_empty()
    || provider_model.as_deref().unwrap_or("").trim().is_empty()
  {
    return Ok(None);
  }

  Ok(Some(OpenAiCompatibleProvider::new(
    provider_base_url.unwrap_or_default(),
    provider_api_key.unwrap_or_default(),
    provider_model.unwrap_or_default(),
    provider_timeout_seconds.unwrap_or(30),
  )))
}

fn try_read_project_file(
  root: &knowledge_core::project::root::ProjectRoot,
  relative_path: &str,
) -> String {
  root
    .safe_join(relative_path)
    .ok()
    .and_then(|path| std::fs::read_to_string(path).ok())
    .unwrap_or_default()
}

fn source_identity_from_relative_path(relative_path: &str) -> String {
  relative_path
    .trim_start_matches("raw/sources/")
    .replace('\\', "/")
}

fn analysis_title_from_text(source_name: &str, source_content: &str, analysis_text: &str) -> String {
  analysis_text
    .lines()
    .find_map(|line| {
      let trimmed = line.trim();
      trimmed
        .strip_prefix("title:")
        .or_else(|| trimmed.strip_prefix("Title:"))
        .map(str::trim)
    })
    .filter(|title| !title.is_empty())
    .map(str::to_string)
    .unwrap_or_else(|| analyze_source(source_name, source_content).title)
}

async fn merge_generated_pages_with_existing_content(
  provider: &OpenAiCompatibleProvider,
  root: &knowledge_core::project::root::ProjectRoot,
  source_name: &str,
  generation_text: &str,
) -> Result<String, ApiError> {
  let review_suffix = review_block_suffix(generation_text);
  let mut blocks = parse_generation_file_blocks(generation_text);

  for block in &mut blocks {
    if block.path == "wiki/index.md" || block.path == "wiki/log.md" || block.path == "wiki/overview.md" {
      continue;
    }

    let absolute = root.safe_join(&block.path).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let existing_content = if absolute.exists() {
      Some(std::fs::read_to_string(&absolute).map_err(|error| ApiError::bad_request(error.to_string()))?)
    } else {
      None
    };

    match prepare_page_merge(&block.content, existing_content.as_deref()) {
      PageMergePlan::Final(final_content) => {
        block.content = final_content;
      }
      PageMergePlan::NeedsProvider {
        existing_content,
        array_merged,
      } => {
        let (system_prompt, user_prompt) =
          build_page_merge_prompts(&existing_content, &array_merged, source_name);
        let llm_output = provider
          .complete_text(ProviderTextRequest {
            system_prompt,
            user_prompt,
          })
          .await
          .map(|response| response.text)
          .ok();
        block.content = finalize_page_merge(
          &existing_content,
          &array_merged,
          llm_output.as_deref(),
          &today_utc(),
        );
      }
    }
  }

  let rendered = render_generation_file_blocks(&blocks);
  if review_suffix.is_empty() {
    Ok(rendered)
  } else {
    Ok(format!("{rendered}\n\n{review_suffix}"))
  }
}

fn today_utc() -> String {
  let now = OffsetDateTime::now_utc().date();
  format!(
    "{:04}-{:02}-{:02}",
    now.year(),
    u8::from(now.month()),
    now.day()
  )
}

fn review_block_suffix(generation_text: &str) -> String {
  generation_text
    .find("---REVIEW:")
    .map(|index| generation_text[index..].trim().to_string())
    .unwrap_or_default()
}
