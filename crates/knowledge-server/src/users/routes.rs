use axum::extract::State;
use axum::http::{header, HeaderMap};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::app::state::AppState;
use crate::auth::session::find_session;
use crate::http::error::ApiError;

pub fn router() -> Router<AppState> {
  Router::new().route("/api/users", get(list_users))
}

async fn list_users(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
  let session_id = headers
    .get(header::COOKIE)
    .and_then(|value| value.to_str().ok())
    .and_then(|cookie| {
      cookie
        .split(';')
        .map(str::trim)
        .find(|item| item.starts_with("knowledge_session="))
        .map(|item| item.trim_start_matches("knowledge_session=").to_string())
    })
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;

  let _session = find_session(&state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;

  let rows = sqlx::query_as::<_, (String, String, String)>(
    "SELECT id, username, role FROM users ORDER BY created_at ASC",
  )
  .fetch_all(&state.pool)
  .await
  .map_err(ApiError::from)?;

  let users = rows
    .into_iter()
    .map(|(id, username, role)| json!({ "id": id, "username": username, "role": role }))
    .collect::<Vec<_>>();

  Ok(Json(json!({ "users": users })))
}
