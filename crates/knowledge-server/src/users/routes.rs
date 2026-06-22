use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

use crate::app::state::AppState;
use crate::http::error::ApiError;

pub fn router() -> Router<AppState> {
  Router::new().route("/api/users", get(list_users))
}

async fn list_users(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
  let _session = crate::auth::operator::require_operator(&state, &headers).await?;

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
