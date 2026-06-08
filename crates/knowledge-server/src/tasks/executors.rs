use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::audit::{append_audit_log, CreateAuditLog};
use crate::projects::service::project_root_for_id;
use crate::tasks::model::TaskRecord;
use crate::tasks::store;
use knowledge_core::ingest::{analyze_source, generate_wiki_from_analysis};
use knowledge_core::project::reviews::update_review_status;
use knowledge_core::project::sources::{delete_source, import_source, rescan_sources};
use knowledge_core::query::answer_from_results;
use knowledge_core::search::search_project;

pub async fn run_task_executor(state: &AppState, task: &TaskRecord) -> Result<(), ApiError> {
  let task = store::mark_task_running_attempt(state, &task.id).await?;

  let result = match task.task_type.as_str() {
    "query.answer" => execute_query(state, &task).await,
    "project.import_source" => run_import_source_executor(state, &task).await,
    "project.rescan_sources" => run_rescan_sources_executor(state, &task).await,
    "project.delete_source" => run_delete_source_executor(state, &task).await,
    "project.ingest_source" => run_ingest_source_executor(state, &task).await,
    "project.update_review" => run_update_review_executor(state, &task).await,
    _ => Err(ApiError::bad_request("unsupported task type")),
  };

  match result {
    Ok(value) => {
      store::complete_task(state, &task.id, value).await?;
      append_task_audit_log(state, &task, "task.succeeded", "Task completed").await?;
      Ok(())
    }
    Err(error) => {
      let retryable = task.task_type == "query.answer" && task.attempt_count < task.max_attempts;
      let error_payload = json!({
        "code": "task_execution_failed",
        "message": error.to_string(),
        "retryable": retryable
      });

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

      Err(error)
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
      let error_payload = json!({
        "code": "task_execution_failed",
        "message": error.to_string(),
        "retryable": false
      });
      store::fail_task(state, &task.id, error_payload).await?;
      Err(error)
    }
  }
}

async fn execute_query(state: &AppState, task: &TaskRecord) -> Result<Value, ApiError> {
  let (provider_mode, provider_base_url, provider_model) =
    sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
      "SELECT provider_mode, provider_base_url, provider_model FROM system_settings WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

  if provider_mode.trim().is_empty()
    || provider_base_url.as_deref().unwrap_or("").trim().is_empty()
    || provider_model.as_deref().unwrap_or("").trim().is_empty()
  {
    return Err(ApiError::bad_request("provider configuration is incomplete"));
  }

  let query = read_string(&task.payload, "query")?;
  let root = project_root_for_id(state, &task.project_id).await?;
  let results = search_project(root.as_path(), &query)
    .map_err(|error| ApiError::internal(error.to_string()))?;
  let answer = answer_from_results(&query, &results);

  Ok(json!({
    "answer": answer.answer,
    "citations": answer.citations.iter().map(|citation| json!({
      "path": citation.path,
      "title": citation.title,
      "snippet": results
        .iter()
        .find(|result| result.path == citation.path)
        .map(|result| result.snippet.clone())
        .unwrap_or_default(),
      "score": results
        .iter()
        .find(|result| result.path == citation.path)
        .map(|result| result.score)
        .unwrap_or_default()
    })).collect::<Vec<_>>(),
    "contextSummary": answer.context_summary,
    "model": provider_model.unwrap_or_default(),
    "provider": provider_mode,
    "usage": {
      "promptTokens": 0,
      "completionTokens": 0,
      "totalTokens": 0
    },
    "completedAt": now_rfc3339()?
  }))
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

fn now_rfc3339() -> Result<String, ApiError> {
  OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))
}
