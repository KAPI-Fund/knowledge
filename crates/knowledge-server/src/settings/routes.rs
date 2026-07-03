use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
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
  #[serde(default)]
  pub tavily_base_url: Option<String>,
  #[serde(default)]
  pub serpapi_base_url: Option<String>,
  #[serde(default)]
  pub clear_provider_api_key: Option<bool>,
  #[serde(default)]
  pub clear_search_api_key: Option<bool>,
}

fn configured(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty())
}

/// Serialize a connection for the settings API, redacting the raw api_key to a
/// boolean flag (matching the providerApiKeyConfigured convention).
pub(crate) fn connection_to_json(c: &crate::providers::ProviderConnection) -> serde_json::Value {
    json!({
        "id": c.id,
        "label": c.label,
        "baseUrl": c.base_url,
        "model": c.model,
        "timeoutSeconds": c.timeout_seconds,
        "isActive": c.is_active,
        "apiKeyConfigured": configured(c.api_key.as_deref()),
    })
}

/// Return the per-provider search config with any apiKey replaced by an
/// apiKeyConfigured boolean, so raw keys never leave the server.
fn redact_search_configs(configs: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for provider in ["tavily", "serpapi", "searxng", "ollama"] {
        let block = configs.get(provider);
        let mut fields = serde_json::Map::new();
        if let Some(obj) = block.and_then(|b| b.as_object()) {
            for (k, v) in obj {
                if k == "apiKey" {
                    let configured = v.as_str().is_some_and(|s| !s.is_empty());
                    fields.insert("apiKeyConfigured".to_string(), json!(configured));
                } else {
                    fields.insert(k.clone(), v.clone());
                }
            }
        }
        if !fields.contains_key("apiKeyConfigured") && provider != "searxng" {
            fields.insert("apiKeyConfigured".to_string(), json!(false));
        }
        out.insert(provider.to_string(), serde_json::Value::Object(fields));
    }
    serde_json::Value::Object(out)
}

async fn build_settings_response(state: &AppState) -> Result<serde_json::Value, ApiError> {
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
    ) = sqlx::query_as::<_, (
        String, String, i64,
        Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>,
        String,
    )>(
        "SELECT provider_mode, language, default_query_limit,
                provider_base_url, provider_api_key, provider_model,
                provider_embedding_model, provider_timeout_seconds, search_provider
         FROM system_settings WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let (
        embedding_enabled,
        embedding_base_url,
        embedding_api_key,
        embedding_model,
        embedding_timeout_seconds,
        image_base_url,
        image_api_key,
        image_model,
        image_size,
        image_timeout_seconds,
        search_provider_configs,
    ) = sqlx::query_as::<_, (
        bool, Option<String>, Option<String>, Option<String>, Option<i64>,
        Option<String>, Option<String>, Option<String>, String, Option<i64>,
        serde_json::Value,
    )>(
        "SELECT embedding_enabled, embedding_base_url, embedding_api_key,
                embedding_model, embedding_timeout_seconds,
                image_base_url, image_api_key, image_model, image_size, image_timeout_seconds,
                search_provider_configs
         FROM system_settings WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let connections: Vec<serde_json::Value> = crate::providers::list_connections(&state.pool)
        .await?
        .iter()
        .map(connection_to_json)
        .collect();

    // Redact api keys inside the search JSONB before returning it.
    let search_providers = redact_search_configs(&search_provider_configs);

    Ok(json!({
        // Legacy flat fields retained for backward compatibility during migration.
        "providerMode": provider_mode,
        "providerBaseUrl": provider_base_url,
        "providerApiKeyConfigured": configured(provider_api_key.as_deref()),
        "providerModel": provider_model,
        "providerEmbeddingModel": provider_embedding_model,
        "providerTimeoutSeconds": provider_timeout_seconds,
        // New structured blocks.
        "connections": connections,
        "embedding": {
            "enabled": embedding_enabled,
            "baseUrl": embedding_base_url,
            "model": embedding_model,
            "timeoutSeconds": embedding_timeout_seconds,
            "apiKeyConfigured": configured(embedding_api_key.as_deref()),
        },
        "image": {
            "baseUrl": image_base_url,
            "model": image_model,
            "size": image_size,
            "timeoutSeconds": image_timeout_seconds,
            "apiKeyConfigured": configured(image_api_key.as_deref()),
        },
        "search": {
            "provider": search_provider,
            "providers": search_providers,
        },
        "defaults": {
            "language": language,
            "defaultQueryLimit": default_query_limit,
        },
    }))
}

async fn get_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
  let _session = crate::auth::operator::require_operator(&state, &headers).await?;
  Ok(Json(build_settings_response(&state).await?))
}

async fn update_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
  let session = crate::auth::operator::require_operator(&state, &headers).await?;
  let supplied = headers
    .get("x-csrf-token")
    .and_then(|value| value.to_str().ok())
    .unwrap_or_default();
  if supplied.is_empty() || supplied != session.csrf_token {
    return Err(ApiError::unauthorized("invalid csrf token"));
  }
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
         provider_api_key = COALESCE(NULLIF($5, ''), CASE WHEN $17 THEN NULL ELSE provider_api_key END),
         provider_model = $6,
         provider_embedding_model = $7,
         provider_timeout_seconds = $8,
         search_provider = COALESCE($9, search_provider),
         search_api_key = COALESCE(NULLIF($10, ''), CASE WHEN $18 THEN NULL ELSE search_api_key END),
         serpapi_engine = COALESCE($11, serpapi_engine),
         searxng_url = $12,
         searxng_categories = COALESCE($13, searxng_categories),
         ollama_search_url = $14,
         tavily_base_url = COALESCE(NULLIF($15, ''), tavily_base_url),
         serpapi_base_url = COALESCE(NULLIF($16, ''), serpapi_base_url)
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
  .bind(payload.tavily_base_url.as_deref())
  .bind(payload.serpapi_base_url.as_deref())
  .bind(payload.clear_provider_api_key.unwrap_or(false))
  .bind(payload.clear_search_api_key.unwrap_or(false))
  .execute(&state.pool)
  .await
  .map_err(ApiError::from)?;

  Ok(Json(build_settings_response(&state).await?))
}

#[cfg(test)]
mod tests {
    use super::connection_to_json;
    use crate::providers::ProviderConnection;

    fn sample() -> ProviderConnection {
        ProviderConnection {
            id: "c1".into(),
            label: "OpenAI".into(),
            base_url: "https://api.openai.com".into(),
            api_key: Some("sk-secret".into()),
            model: "gpt-4o".into(),
            timeout_seconds: Some(30),
            is_active: true,
            sort_order: 0,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn connection_json_redacts_api_key() {
        let json = connection_to_json(&sample());
        assert_eq!(json["id"], "c1");
        assert_eq!(json["label"], "OpenAI");
        assert_eq!(json["model"], "gpt-4o");
        assert_eq!(json["isActive"], true);
        assert_eq!(json["apiKeyConfigured"], true);
        assert!(json.get("apiKey").is_none(), "raw api_key must never be serialized");
    }

    #[test]
    fn connection_json_reports_unconfigured_key() {
        let mut c = sample();
        c.api_key = None;
        assert_eq!(connection_to_json(&c)["apiKeyConfigured"], false);
    }
}
