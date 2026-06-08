use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::providers::{OpenAiCompatibleProvider, ProviderError, ProviderQueryRequest};
use crate::projects::audit::{append_audit_log, CreateAuditLog};
use crate::projects::service::project_root_for_id;
use crate::tasks::model::TaskRecord;
use crate::tasks::store;
use knowledge_core::ingest::{analyze_source, generate_wiki_from_analysis};
use knowledge_core::project::queries::{save_query_page, SaveQueryPageInput, SavedQueryCitation};
use knowledge_core::project::reviews::update_review_status;
use knowledge_core::project::sources::{delete_source, import_source, rescan_sources};
use knowledge_core::search::search_project;

pub async fn run_task_executor(state: &AppState, task: &TaskRecord) -> Result<(), ApiError> {
  let task = store::mark_task_running_attempt(state, &task.id).await?;

  let result: Result<Value, TaskExecutionError> = match task.task_type.as_str() {
    "query.answer" => execute_query(state, &task).await,
    "query.save_answer" => run_save_query_answer_executor(state, &task).await.map_err(Into::into),
    "project.import_source" => run_import_source_executor(state, &task).await.map_err(Into::into),
    "project.rescan_sources" => run_rescan_sources_executor(state, &task).await.map_err(Into::into),
    "project.delete_source" => run_delete_source_executor(state, &task).await.map_err(Into::into),
    "project.ingest_source" => run_ingest_source_executor(state, &task).await.map_err(Into::into),
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

  if provider_mode.trim().is_empty()
    || provider_base_url.as_deref().unwrap_or("").trim().is_empty()
    || provider_model.as_deref().unwrap_or("").trim().is_empty()
  {
    return Err(ApiError::bad_request("provider configuration is incomplete").into());
  }

  if provider_mode != "openai-compatible" {
    return Err(ApiError::bad_request("unsupported provider mode").into());
  }

  let query = read_string(&task.payload, "query")?;
  let top_k = read_i64(&task.payload, "topK").unwrap_or(3).max(1) as usize;
  let language = task
    .payload
    .get("language")
    .and_then(Value::as_str)
    .filter(|value| !value.trim().is_empty())
    .unwrap_or("en")
    .to_string();
  let root = project_root_for_id(state, &task.project_id).await?;
  let results = search_project(root.as_path(), &query)
    .map_err(|error| ApiError::internal(error.to_string()))?;
  let selected_results = results.into_iter().take(top_k).collect::<Vec<_>>();
  let context_blocks = build_context_blocks(&selected_results);
  let context_summary = build_context_summary(&selected_results);
  let provider = OpenAiCompatibleProvider::new(
    provider_base_url.unwrap_or_default(),
    provider_api_key.unwrap_or_default(),
    provider_model.clone().unwrap_or_default(),
    provider_timeout_seconds.unwrap_or(30),
  );
  let answer = provider
    .answer_query(ProviderQueryRequest {
      query,
      context_blocks,
      language,
    })
    .await
    .map_err(TaskExecutionError::from_provider_error)?;

  Ok(json!({
    "answer": answer.answer,
    "citations": selected_results
      .first()
      .map(|result| {
        json!({
          "path": result.path,
          "title": result.title,
          "snippet": result.snippet,
          "score": result.score
        })
      })
      .into_iter()
      .collect::<Vec<_>>(),
    "contextSummary": context_summary,
    "model": provider_model.unwrap_or_default(),
    "provider": provider_mode,
    "usage": {
      "promptTokens": answer.usage.prompt_tokens,
      "completionTokens": answer.usage.completion_tokens,
      "totalTokens": answer.usage.total_tokens
    },
    "completedAt": now_rfc3339()?
  }))
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

async fn run_ingest_source_executor(
  state: &AppState,
  task: &TaskRecord,
) -> Result<Value, ApiError> {
  let root = project_root_for_id(state, &task.project_id).await?;
  let relative_path = read_string(&task.payload, "relativePath")?;
  let source_path = root
    .safe_join(&relative_path)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
  let content =
    std::fs::read_to_string(&source_path).map_err(|error| ApiError::bad_request(error.to_string()))?;
  let source_name = source_path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or_else(|| ApiError::bad_request("invalid source name"))?;
  let analysis = analyze_source(source_name, &content);
  let result = generate_wiki_from_analysis(&root, source_name, &analysis)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;

  Ok(json!({
    "summaryPath": result.summary_path
  }))
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

fn build_context_blocks(results: &[knowledge_core::search::SearchResult]) -> Vec<String> {
  if results.is_empty() {
    return vec!["No relevant wiki context was retrieved.".to_string()];
  }

  results
    .iter()
    .enumerate()
    .map(|(index, result)| {
      format!(
        "[{}] {}\nTitle: {}\nSnippet: {}",
        index + 1,
        result.path,
        result.title,
        result.snippet
      )
    })
    .collect()
}

fn build_context_summary(results: &[knowledge_core::search::SearchResult]) -> String {
  results
    .iter()
    .map(|result| format!("{} ({})", result.path, result.title))
    .collect::<Vec<_>>()
    .join("; ")
}

fn now_rfc3339() -> Result<String, ApiError> {
  OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))
}
