use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::tasks::model::TaskRecord;

#[derive(Debug, Clone)]
pub struct CreateTaskInput {
  pub project_id: String,
  pub task_type: String,
  pub title: String,
  pub relative_path: Option<String>,
  pub detail: Value,
  pub payload: Value,
  pub created_by: String,
  pub max_attempts: i64,
}

pub async fn create_task(state: &AppState, input: CreateTaskInput) -> Result<TaskRecord, ApiError> {
  let id = Uuid::new_v4().to_string();
  let now = now_rfc3339()?;
  let detail =
    serde_json::to_string(&input.detail).map_err(|error| ApiError::internal(error.to_string()))?;
  let payload =
    serde_json::to_string(&input.payload).map_err(|error| ApiError::internal(error.to_string()))?;

  sqlx::query(
    "INSERT INTO project_tasks (
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at
    ) VALUES (
      $1, $2, $3, $4, $5, $6, $7::jsonb, $8::jsonb, NULL, NULL,
      $9, $10, $11, $12, $13, NULL, NULL,
      NULL, NULL, NULL
    )",
  )
  .bind(&id)
  .bind(&input.project_id)
  .bind(&input.task_type)
  .bind("queued")
  .bind(&input.title)
  .bind(&input.relative_path)
  .bind(&detail)
  .bind(&payload)
  .bind(0_i64)
  .bind(input.max_attempts)
  .bind(&input.created_by)
  .bind(&now)
  .bind(&now)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;
  state
    .cache
    .signal_task_wakeup()
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?;

  get_task_by_id(state, &id).await
}

pub async fn get_task_by_id(state: &AppState, task_id: &str) -> Result<TaskRecord, ApiError> {
  let task = sqlx::query_as::<_, TaskRecord>(
    "SELECT
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at
     FROM project_tasks
     WHERE id = $1",
  )
  .bind(task_id)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?
  .ok_or_else(|| ApiError::bad_request("unknown task"))?;

  Ok(task)
}

pub async fn acquire_next_task(
  state: &AppState,
  lease_owner: &str,
  lease_seconds: i64,
) -> Result<Option<TaskRecord>, ApiError> {
  let lease_expires_at = OffsetDateTime::now_utc()
    .checked_add(time::Duration::seconds(lease_seconds))
    .ok_or_else(|| ApiError::internal("failed to compute lease expiry"))?
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format lease expiry"))?;
  let started_at = now_rfc3339()?;

  // Ingest-queue pause (upstream ingest-queue.ts pause semantics, drain
  // variant): while system_settings.ingest_paused, ingest-class tasks are not
  // claimed; already-running tasks finish and other task types keep flowing.
  let task = sqlx::query_as::<_, TaskRecord>(
    "WITH next_task AS (
      SELECT id
      FROM project_tasks
      WHERE status IN ('queued', 'retry_waiting')
        AND (next_retry_at IS NULL OR next_retry_at <= $3)
        AND (
          task_type NOT IN ('project.import_source', 'project.ingest_source', 'project.delete_source')
          OR NOT (SELECT ingest_paused FROM system_settings WHERE id = 1)
        )
      ORDER BY created_at ASC
      LIMIT 1
      FOR UPDATE SKIP LOCKED
    )
    UPDATE project_tasks
    SET status = 'running',
        started_at = COALESCE(started_at, $3),
        updated_at = $3,
        lease_owner = $1,
        lease_expires_at = $2,
        next_retry_at = NULL
    WHERE id IN (SELECT id FROM next_task)
    RETURNING
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at",
  )
  .bind(lease_owner)
  .bind(&lease_expires_at)
  .bind(&started_at)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(task)
}

pub async fn complete_task(
  state: &AppState,
  task_id: &str,
  result: Value,
) -> Result<TaskRecord, ApiError> {
  let updated_at = now_rfc3339()?;
  let result_json =
    serde_json::to_string(&result).map_err(|error| ApiError::internal(error.to_string()))?;

  let task = sqlx::query_as::<_, TaskRecord>(
    "UPDATE project_tasks
     SET status = 'succeeded',
         result = $2::jsonb,
         error = NULL,
         updated_at = $3,
         finished_at = $3,
         lease_owner = NULL,
         lease_expires_at = NULL,
         next_retry_at = NULL
     WHERE id = $1
     RETURNING
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at",
  )
  .bind(task_id)
  .bind(&result_json)
  .bind(&updated_at)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?
  .ok_or_else(|| ApiError::bad_request("unknown task"))?;

  Ok(task)
}

pub async fn fail_task(
  state: &AppState,
  task_id: &str,
  error: Value,
) -> Result<TaskRecord, ApiError> {
  let updated_at = now_rfc3339()?;
  let error_json =
    serde_json::to_string(&error).map_err(|value| ApiError::internal(value.to_string()))?;

  let task = sqlx::query_as::<_, TaskRecord>(
    "UPDATE project_tasks
     SET status = 'failed',
         error = $2::jsonb,
         updated_at = $3,
         finished_at = $3,
         lease_owner = NULL,
         lease_expires_at = NULL,
         next_retry_at = NULL
     WHERE id = $1
     RETURNING
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at",
  )
  .bind(task_id)
  .bind(&error_json)
  .bind(&updated_at)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?
  .ok_or_else(|| ApiError::bad_request("unknown task"))?;

  Ok(task)
}

pub async fn retry_task(
  state: &AppState,
  task_id: &str,
  error: Value,
  next_retry_at: String,
) -> Result<TaskRecord, ApiError> {
  let updated_at = now_rfc3339()?;
  let error_json =
    serde_json::to_string(&error).map_err(|value| ApiError::internal(value.to_string()))?;

  let task = sqlx::query_as::<_, TaskRecord>(
    "UPDATE project_tasks
     SET status = 'retry_waiting',
         error = $2::jsonb,
         updated_at = $3,
         finished_at = NULL,
         lease_owner = NULL,
         lease_expires_at = NULL,
         next_retry_at = $4
     WHERE id = $1
     RETURNING
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at",
  )
  .bind(task_id)
  .bind(&error_json)
  .bind(&updated_at)
  .bind(&next_retry_at)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?
  .ok_or_else(|| ApiError::bad_request("unknown task"))?;

  Ok(task)
}

pub async fn mark_task_running_attempt(
  state: &AppState,
  task_id: &str,
) -> Result<TaskRecord, ApiError> {
  let updated_at = now_rfc3339()?;
  let task = sqlx::query_as::<_, TaskRecord>(
    "UPDATE project_tasks
     SET attempt_count = attempt_count + 1,
         updated_at = $2
     WHERE id = $1
     RETURNING
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at",
  )
  .bind(task_id)
  .bind(&updated_at)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?
  .ok_or_else(|| ApiError::bad_request("unknown task"))?;

  Ok(task)
}

pub async fn update_task_status(
  state: &AppState,
  task_id: &str,
  status: &str,
) -> Result<TaskRecord, ApiError> {
  let updated_at = now_rfc3339()?;
  let task = sqlx::query_as::<_, TaskRecord>(
    "UPDATE project_tasks
     SET status = $2,
         updated_at = $3,
         lease_owner = CASE WHEN $2 = 'queued' THEN NULL ELSE lease_owner END,
         lease_expires_at = CASE WHEN $2 = 'queued' THEN NULL ELSE lease_expires_at END,
         next_retry_at = CASE WHEN $2 = 'queued' THEN NULL ELSE next_retry_at END
     WHERE id = $1
     RETURNING
      id, project_id, task_type, status, title, relative_path, detail, payload, result, error,
      attempt_count, max_attempts, created_by, created_at, updated_at, started_at, finished_at,
      lease_owner, lease_expires_at, next_retry_at",
  )
  .bind(task_id)
  .bind(status)
  .bind(&updated_at)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?
  .ok_or_else(|| ApiError::bad_request("unknown task"))?;

  Ok(task)
}

fn now_rfc3339() -> Result<String, ApiError> {
  OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))
}
