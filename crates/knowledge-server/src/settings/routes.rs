use axum::extract::State;
use axum::http::{header, HeaderMap};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
use crate::auth::session::find_session;
use crate::http::error::ApiError;

pub fn router() -> Router<AppState> {
  Router::new().route("/api/system/settings", get(get_settings).patch(update_settings))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsRequest {
  pub provider_mode: String,
  pub language: String,
  pub default_query_limit: i64,
}

async fn get_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
  require_session(&state, &headers).await?;
  let (provider_mode, language, default_query_limit) = sqlx::query_as::<_, (String, String, i64)>(
    "SELECT provider_mode, language, default_query_limit FROM system_settings WHERE id = 1",
  )
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(Json(json!({
    "providerMode": provider_mode,
    "language": language,
    "defaultQueryLimit": default_query_limit
  })))
}

async fn update_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
  require_session(&state, &headers).await?;
  sqlx::query(
    "UPDATE system_settings SET provider_mode = $1, language = $2, default_query_limit = $3 WHERE id = 1",
  )
  .bind(&payload.provider_mode)
  .bind(&payload.language)
  .bind(payload.default_query_limit)
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(Json(json!({
    "providerMode": payload.provider_mode,
    "language": payload.language,
    "defaultQueryLimit": payload.default_query_limit
  })))
}

async fn require_session(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
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

  let _session = find_session(state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;

  Ok(())
}
