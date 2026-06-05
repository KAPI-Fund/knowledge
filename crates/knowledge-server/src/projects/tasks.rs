use serde::Serialize;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;

#[derive(Debug, Clone)]
pub struct CreateTaskRecord {
  pub project_id: String,
  pub task_type: String,
  pub title: String,
  pub relative_path: Option<String>,
  pub detail: Value,
  pub created_by: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
  pub id: String,
  pub task_type: String,
  pub status: String,
  pub title: String,
  pub relative_path: Option<String>,
  pub detail: Value,
  pub created_at: String,
  pub updated_at: String,
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
    "INSERT INTO project_tasks (id, project_id, task_type, status, title, relative_path, detail, created_by, created_at, updated_at)
     VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, $8, $9, $10)",
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
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(TaskRecord {
    id,
    task_type: input.task_type,
    status: "completed".to_string(),
    title: input.title,
    relative_path: input.relative_path,
    detail: input.detail,
    created_at: now.clone(),
    updated_at: now,
  })
}

pub async fn list_tasks(state: &AppState, project_id: &str) -> Result<Vec<TaskRecord>, ApiError> {
  let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, Value, String, String)>(
    "SELECT id, task_type, status, title, relative_path, detail, created_at, updated_at
     FROM project_tasks
     WHERE project_id = $1
     ORDER BY created_at ASC",
  )
  .bind(project_id)
  .fetch_all(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(rows
    .into_iter()
    .map(
      |(id, task_type, status, title, relative_path, detail, created_at, updated_at)| TaskRecord {
        id,
        task_type,
        status,
        title,
        relative_path,
        detail,
        created_at,
        updated_at,
      },
    )
    .collect())
}

pub async fn get_task(
  state: &AppState,
  project_id: &str,
  task_id: &str,
) -> Result<TaskRecord, ApiError> {
  let row = sqlx::query_as::<_, (String, String, String, String, Option<String>, Value, String, String)>(
    "SELECT id, task_type, status, title, relative_path, detail, created_at, updated_at
     FROM project_tasks
     WHERE project_id = $1 AND id = $2",
  )
  .bind(project_id)
  .bind(task_id)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?
  .ok_or_else(|| ApiError::bad_request("unknown task"))?;

  Ok(TaskRecord {
    id: row.0,
    task_type: row.1,
    status: row.2,
    title: row.3,
    relative_path: row.4,
    detail: row.5,
    created_at: row.6,
    updated_at: row.7,
  })
}

pub async fn update_task_status(
  state: &AppState,
  project_id: &str,
  task_id: &str,
  status: &str,
) -> Result<TaskRecord, ApiError> {
  let updated_at = now_rfc3339()?;
  let row = sqlx::query_as::<_, (String, String, String, String, Option<String>, Value, String, String)>(
    "UPDATE project_tasks
     SET status = $3, updated_at = $4
     WHERE project_id = $1 AND id = $2
     RETURNING id, task_type, status, title, relative_path, detail, created_at, updated_at",
  )
  .bind(project_id)
  .bind(task_id)
  .bind(status)
  .bind(&updated_at)
  .fetch_optional(&state.pool)
  .await
  .map_err(ApiError::from)?
  .ok_or_else(|| ApiError::bad_request("unknown task"))?;

  Ok(TaskRecord {
    id: row.0,
    task_type: row.1,
    status: row.2,
    title: row.3,
    relative_path: row.4,
    detail: row.5,
    created_at: row.6,
    updated_at: row.7,
  })
}

fn now_rfc3339() -> Result<String, ApiError> {
  OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))
}
