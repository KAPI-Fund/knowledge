use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
  pub id: String,
  pub project_id: String,
  pub task_type: String,
  pub status: String,
  pub title: String,
  pub relative_path: Option<String>,
  pub detail: Value,
  pub payload: Value,
  pub result: Option<Value>,
  pub error: Option<Value>,
  pub attempt_count: i64,
  pub max_attempts: i64,
  pub created_by: String,
  pub created_at: String,
  pub updated_at: String,
  pub started_at: Option<String>,
  pub finished_at: Option<String>,
  pub lease_owner: Option<String>,
  pub lease_expires_at: Option<String>,
  pub next_retry_at: Option<String>,
}
