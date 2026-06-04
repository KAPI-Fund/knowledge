use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
use crate::auth::session::find_session;
use crate::http::error::ApiError;
use crate::projects::service::create_project;

pub fn router() -> Router<AppState> {
  Router::new()
    .route("/api/projects", post(create_project_handler))
    .route("/api/projects/{project_id}/members", get(list_project_members))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
  pub name: String,
  pub root_path: String,
}

async fn create_project_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, ApiError> {
  let session_id = extract_session_id(&headers).ok_or_else(|| ApiError::unauthorized("missing session"))?;
  let session = find_session(&state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;
  validate_csrf(&headers, &session.csrf_token)?;
  let project = create_project(&payload, &state, &session.user_id).await?;
  Ok((StatusCode::CREATED, Json(project)))
}

async fn list_project_members(
  State(state): State<AppState>,
  headers: HeaderMap,
  Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
  let session_id = extract_session_id(&headers).ok_or_else(|| ApiError::unauthorized("missing session"))?;
  let session = find_session(&state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;

  let membership = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM project_members WHERE project_id = ?1 AND user_id = ?2",
  )
  .bind(&project_id)
  .bind(&session.user_id)
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  if membership == 0 {
    return Err(ApiError::forbidden("not a project member"));
  }

  let rows = sqlx::query_as::<_, (String, String, i64)>(
    "SELECT user_id, role, can_import FROM project_members WHERE project_id = ?1 ORDER BY created_at ASC",
  )
  .bind(&project_id)
  .fetch_all(&state.pool)
  .await
  .map_err(ApiError::from)?;

  let members = rows
    .into_iter()
    .map(|(user_id, role, can_import)| {
      json!({
        "userId": user_id,
        "role": role,
        "canImport": can_import != 0
      })
    })
    .collect::<Vec<_>>();

  Ok(Json(json!({ "members": members })))
}

fn extract_session_id(headers: &HeaderMap) -> Option<String> {
  headers
    .get(header::COOKIE)
    .and_then(|value| value.to_str().ok())
    .and_then(|cookie| {
      cookie
        .split(';')
        .map(str::trim)
        .find(|item| item.starts_with("knowledge_session="))
        .map(|item| item.trim_start_matches("knowledge_session=").to_string())
    })
}

fn validate_csrf(headers: &HeaderMap, expected_token: &str) -> Result<(), ApiError> {
  let supplied = headers
    .get("x-csrf-token")
    .and_then(|value| value.to_str().ok())
    .unwrap_or_default();

  if supplied.is_empty() || supplied != expected_token {
    return Err(ApiError::unauthorized("invalid csrf token"));
  }

  Ok(())
}
