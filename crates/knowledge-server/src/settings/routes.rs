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
  pub provider_base_url: Option<String>,
  pub provider_api_key: Option<String>,
  pub provider_model: Option<String>,
  pub provider_embedding_model: Option<String>,
  pub provider_timeout_seconds: Option<i64>,
  #[serde(default)]
  pub search_provider: Option<String>,
  #[serde(default)]
  pub search_api_key: Option<String>,
  #[serde(default)]
  pub serpapi_engine: Option<String>,
  #[serde(default)]
  pub searxng_url: Option<String>,
  #[serde(default)]
  pub searxng_categories: Option<Vec<String>>,
  #[serde(default)]
  pub ollama_search_url: Option<String>,
}

async fn get_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
  require_session(&state, &headers).await?;
  let (
    provider_mode,
    language,
    default_query_limit,
    provider_base_url,
    provider_api_key,
    provider_model,
    provider_embedding_model,
    provider_timeout_seconds,
    search_provider,
    search_api_key,
    serpapi_engine,
    searxng_url,
    searxng_categories,
    ollama_search_url,
  ) = sqlx::query_as::<_, (
    String,
    String,
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    serde_json::Value,
    Option<String>,
  )>(
    "SELECT
      provider_mode,
      language,
      default_query_limit,
      provider_base_url,
      provider_api_key,
      provider_model,
      provider_embedding_model,
      provider_timeout_seconds,
      search_provider,
      search_api_key,
      serpapi_engine,
      searxng_url,
      searxng_categories,
      ollama_search_url
     FROM system_settings
     WHERE id = 1",
  )
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(Json(json!({
    "providerMode": provider_mode,
    "language": language,
    "defaultQueryLimit": default_query_limit,
    "providerBaseUrl": provider_base_url,
    "providerApiKeyConfigured": provider_api_key.as_deref().is_some_and(|value| !value.is_empty()),
    "providerModel": provider_model,
    "providerEmbeddingModel": provider_embedding_model,
    "providerTimeoutSeconds": provider_timeout_seconds,
    "searchProvider": search_provider,
    "searchApiKeyConfigured": search_api_key.as_deref().is_some_and(|value| !value.is_empty()),
    "serpapiEngine": serpapi_engine,
    "searxngUrl": searxng_url,
    "searxngCategories": searxng_categories,
    "ollamaSearchUrl": ollama_search_url
  })))
}

async fn update_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
  require_session(&state, &headers).await?;
  let searxng_categories_value = payload
    .searxng_categories
    .as_ref()
    .map(|values| serde_json::to_value(values).unwrap_or(serde_json::Value::Null));
  sqlx::query(
    "UPDATE system_settings
     SET provider_mode = $1,
         language = $2,
         default_query_limit = $3,
         provider_base_url = $4,
         provider_api_key = COALESCE(NULLIF($5, ''), provider_api_key),
         provider_model = $6,
         provider_embedding_model = $7,
         provider_timeout_seconds = $8,
         search_provider = COALESCE($9, search_provider),
         search_api_key = COALESCE(NULLIF($10, ''), search_api_key),
         serpapi_engine = COALESCE($11, serpapi_engine),
         searxng_url = $12,
         searxng_categories = COALESCE($13, searxng_categories),
         ollama_search_url = $14
     WHERE id = 1",
  )
  .bind(&payload.provider_mode)
  .bind(&payload.language)
  .bind(payload.default_query_limit)
  .bind(&payload.provider_base_url)
  .bind(payload.provider_api_key.as_deref())
  .bind(&payload.provider_model)
  .bind(&payload.provider_embedding_model)
  .bind(payload.provider_timeout_seconds)
  .bind(payload.search_provider.as_deref())
  .bind(payload.search_api_key.as_deref())
  .bind(payload.serpapi_engine.as_deref())
  .bind(payload.searxng_url.as_deref())
  .bind(searxng_categories_value)
  .bind(payload.ollama_search_url.as_deref())
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(Json(json!({
    "providerMode": payload.provider_mode,
    "language": payload.language,
    "defaultQueryLimit": payload.default_query_limit,
    "providerBaseUrl": payload.provider_base_url,
    "providerApiKeyConfigured": payload.provider_api_key.as_deref().is_some_and(|value| !value.is_empty()),
    "providerModel": payload.provider_model,
    "providerEmbeddingModel": payload.provider_embedding_model,
    "providerTimeoutSeconds": payload.provider_timeout_seconds,
    "searchProvider": payload.search_provider,
    "searchApiKeyConfigured": payload.search_api_key.as_deref().is_some_and(|value| !value.is_empty()),
    "serpapiEngine": payload.serpapi_engine,
    "searxngUrl": payload.searxng_url,
    "searxngCategories": payload.searxng_categories,
    "ollamaSearchUrl": payload.ollama_search_url
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
