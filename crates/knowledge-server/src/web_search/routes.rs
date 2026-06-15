use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
use crate::auth::principal::resolve_principal;
use crate::http::error::ApiError;
use crate::web_search::config::load_web_search_config;
use crate::web_search::provider::web_search;

pub fn router() -> Router<AppState> {
  Router::new().route("/api/web-search", post(web_search_handler))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchRequest {
  pub query: String,
  #[serde(default)]
  pub max_results: Option<usize>,
}

async fn web_search_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<WebSearchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
  let principal = resolve_principal(&state, &headers).await?;
  if principal.requires_csrf() {
    let expected = principal
      .csrf_token
      .as_deref()
      .ok_or_else(|| ApiError::unauthorized("missing csrf token"))?;
    let supplied = headers
      .get("x-csrf-token")
      .and_then(|value| value.to_str().ok())
      .unwrap_or_default();
    if supplied.is_empty() || supplied != expected {
      return Err(ApiError::unauthorized("invalid csrf token"));
    }
  }
  let trimmed = payload.query.trim();
  if trimmed.is_empty() {
    return Err(ApiError::bad_request("query is required"));
  }
  let max_results = payload.max_results.unwrap_or(10).clamp(1, 50);

  let config = load_web_search_config(&state)
    .await?
    .ok_or_else(|| ApiError::bad_request("web search provider is not configured"))?;
  let results = web_search(&config, trimmed, max_results).await?;
  Ok(Json(json!({ "results": results })))
}
