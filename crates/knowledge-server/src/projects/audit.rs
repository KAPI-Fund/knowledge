use serde::Serialize;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;

#[derive(Debug, Clone)]
pub struct CreateAuditLog {
  pub project_id: Option<String>,
  pub actor_id: String,
  pub action: String,
  pub target_type: String,
  pub target_id: String,
  pub task_id: Option<String>,
  pub summary: String,
  pub metadata: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogRecord {
  pub id: String,
  pub action: String,
  pub target_type: String,
  pub target_id: String,
  pub task_id: Option<String>,
  pub summary: String,
  pub metadata: Value,
  pub created_at: String,
}

pub async fn append_audit_log(
  state: &AppState,
  input: CreateAuditLog,
) -> Result<AuditLogRecord, ApiError> {
  let id = Uuid::new_v4().to_string();
  let created_at = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format timestamp"))?;
  let metadata =
    serde_json::to_string(&input.metadata).map_err(|error| ApiError::internal(error.to_string()))?;

  sqlx::query(
    "INSERT INTO audit_logs (id, project_id, actor_id, action, target_type, target_id, task_id, summary, metadata, created_at)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::jsonb, $10)",
  )
  .bind(&id)
  .bind(&input.project_id)
  .bind(&input.actor_id)
  .bind(&input.action)
  .bind(&input.target_type)
  .bind(&input.target_id)
  .bind(&input.task_id)
  .bind(&input.summary)
  .bind(&metadata)
  .bind(&created_at)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(AuditLogRecord {
    id,
    action: input.action,
    target_type: input.target_type,
    target_id: input.target_id,
    task_id: input.task_id,
    summary: input.summary,
    metadata: input.metadata,
    created_at,
  })
}

pub async fn list_audit_logs(
  state: &AppState,
  project_id: &str,
) -> Result<Vec<AuditLogRecord>, ApiError> {
  let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, String, Value, String)>(
    "SELECT id, action, target_type, target_id, task_id, summary, metadata, created_at
     FROM audit_logs
     WHERE project_id = $1
     ORDER BY created_at DESC",
  )
  .bind(project_id)
  .fetch_all(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(rows
    .into_iter()
    .map(|(id, action, target_type, target_id, task_id, summary, metadata, created_at)| {
      AuditLogRecord {
        id,
        action,
        target_type,
        target_id,
        task_id,
        summary,
        metadata,
        created_at,
      }
    })
    .collect())
}
