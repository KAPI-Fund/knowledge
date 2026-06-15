use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::audit::{append_audit_log, CreateAuditLog};
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::projects::tasks::{create_queued_task, CreateTaskRecord};

pub fn router() -> Router<AppState> {
  Router::new().route(
    "/api/projects/{project_id}/deep-research",
    post(deep_research_handler),
  )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepResearchRequest {
  pub topic: String,
  #[serde(default)]
  pub search_queries: Option<Vec<String>>,
}

async fn deep_research_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
  Json(payload): Json<DeepResearchRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
  let principal = authorized_principal(&state, &headers, Some(&project_id)).await?;
  validate_csrf(&headers, &principal)?;
  let topic = payload.topic.trim();
  if topic.is_empty() {
    return Err(ApiError::bad_request("topic is required"));
  }
  let queries = payload.search_queries.clone().unwrap_or_default();
  let task = create_queued_task(
    &state,
    CreateTaskRecord {
      project_id: project_id.clone(),
      task_type: "project.deep_research".to_string(),
      title: format!("Deep research: {topic}"),
      relative_path: None,
      detail: json!({}),
      created_by: principal.user_id.clone(),
    },
    json!({
      "topic": topic,
      "searchQueries": queries
    }),
  )
  .await?;
  append_audit_log(
    &state,
    CreateAuditLog {
      project_id: Some(project_id),
      actor_id: principal.user_id,
      action: "deep_research.enqueued".to_string(),
      target_type: "deep_research".to_string(),
      target_id: "task".to_string(),
      task_id: Some(task.id.clone()),
      summary: format!("Queued deep research: {topic}"),
      metadata: json!({ "topic": topic, "searchQueries": queries }),
    },
  )
  .await?;
  Ok((
    StatusCode::ACCEPTED,
    Json(json!({ "taskId": task.id, "status": task.status })),
  ))
}
