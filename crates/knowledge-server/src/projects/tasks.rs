use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::tasks::model::TaskRecord;
use crate::tasks::store::{create_task, get_task_by_id, CreateTaskInput};

#[derive(Debug, Clone)]
pub struct CreateTaskRecord {
  pub project_id: String,
  pub task_type: String,
  pub title: String,
  pub relative_path: Option<String>,
  pub detail: Value,
  pub created_by: String,
}

pub async fn create_completed_task(
  state: &AppState,
  input: CreateTaskRecord,
) -> Result<TaskRecord, ApiError> {
  let id = Uuid::new_v4().to_string();
  let now = now_rfc3339()?;
  let detail = serde_json::to_string(&input.detail)
    .map_err(|error| ApiError::internal(error.to_string()))?;

  sqlx::query(
    "INSERT INTO project_tasks (
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at
    ) VALUES (
      $1, $2, $3, $4, $5, $6, $7::jsonb, '{}'::jsonb, NULL, NULL,
      0, 3, $8, $9, $10, NULL, $11,
      NULL, NULL, NULL
    )",
  )
  .bind(&id)
  .bind(&input.project_id)
  .bind(&input.task_type)
  .bind("completed")
  .bind(&input.title)
  .bind(&input.relative_path)
  .bind(&detail)
  .bind(&input.created_by)
  .bind(&now)
  .bind(&now)
  .bind(&now)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  get_task_by_id(state, &id).await
}

pub async fn create_queued_task(
  state: &AppState,
  input: CreateTaskRecord,
  payload: Value,
) -> Result<TaskRecord, ApiError> {
  create_task(
    state,
    CreateTaskInput {
      project_id: input.project_id,
      task_type: input.task_type,
      title: input.title,
      relative_path: input.relative_path,
      detail: input.detail,
      payload,
      created_by: input.created_by,
      max_attempts: 3,
    },
  )
  .await
}

pub async fn list_tasks(state: &AppState, project_id: &str) -> Result<Vec<TaskRecord>, ApiError> {
  sqlx::query_as::<_, TaskRecord>(
    "SELECT
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at
     FROM project_tasks
     WHERE project_id = $1
     ORDER BY created_at ASC",
  )
  .bind(project_id)
  .fetch_all(&state.pool)
  .await
  .map_err(ApiError::from)
}

pub async fn get_task(
  state: &AppState,
  project_id: &str,
  task_id: &str,
) -> Result<TaskRecord, ApiError> {
  let task = get_task_by_id(state, task_id).await?;
  if task.project_id != project_id {
    return Err(ApiError::bad_request("unknown task"));
  }
  Ok(task)
}

pub async fn update_task_status(
  state: &AppState,
  project_id: &str,
  task_id: &str,
  status: &str,
) -> Result<TaskRecord, ApiError> {
  let updated_at = now_rfc3339()?;
  let finished_at = if matches!(status, "succeeded" | "failed" | "cancelled" | "completed") {
    Some(updated_at.clone())
  } else {
    None
  };

  sqlx::query_as::<_, TaskRecord>(
    "UPDATE project_tasks
     SET status = $3, updated_at = $4, finished_at = COALESCE($5, finished_at)
     WHERE project_id = $1 AND id = $2
     RETURNING
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at",
  )
  .bind(project_id)
  .bind(task_id)
  .bind(status)
  .bind(&updated_at)
  .bind(&finished_at)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?
  .ok_or_else(|| ApiError::bad_request("unknown task"))
}

fn now_rfc3339() -> Result<String, ApiError> {
  OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))
}
