use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
use crate::http::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/system/settings", get(get_settings).patch(update_settings))
        .route(
            "/api/system/provider-connections",
            axum::routing::post(create_connection_handler),
        )
        .route(
            "/api/system/provider-connections/{id}",
            axum::routing::patch(update_connection_handler).delete(delete_connection_handler),
        )
        .route(
            "/api/system/provider-connections/{id}/activate",
            axum::routing::post(activate_connection_handler),
        )
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
  #[serde(default)]
  pub image: Option<ImageSettingsBlock>,
  #[serde(default)]
  pub embedding: Option<EmbeddingSettingsBlock>,
  #[serde(default)]
  pub search: Option<SearchSettingsBlock>,
  #[serde(default)]
  pub defaults: Option<DefaultsBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSettingsBlock {
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSettingsBlock {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSettingsBlock {
    #[serde(default)]
    pub provider: Option<String>,
    /// Full per-provider config map (already merged client-side so switching
    /// providers retains each key). Stored verbatim into search_provider_configs.
    #[serde(default)]
    pub providers: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultsBlock {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub default_query_limit: Option<i64>,
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

async fn require_operator_csrf(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::auth::session::SessionRecord, ApiError> {
    let session = crate::auth::operator::require_operator(state, headers).await?;
    let supplied = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if supplied.is_empty() || supplied != session.csrf_token {
        return Err(ApiError::unauthorized("invalid csrf token"));
    }
    Ok(session)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConnectionRequest {
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub model: String,
    #[serde(default)]
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConnectionRequest {
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    pub model: String,
    #[serde(default)]
    pub timeout_seconds: Option<i64>,
}

async fn create_connection_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateConnectionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_operator_csrf(&state, &headers).await?;
    crate::providers::create_connection(
        &state.pool,
        &crate::providers::NewConnection {
            label: payload.label,
            base_url: payload.base_url,
            api_key: payload.api_key,
            model: payload.model,
            timeout_seconds: payload.timeout_seconds,
        },
    )
    .await?;
    Ok(Json(build_settings_response(&state).await?))
}

async fn update_connection_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(payload): Json<UpdateConnectionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_operator_csrf(&state, &headers).await?;
    crate::providers::update_connection(
        &state.pool,
        &id,
        &crate::providers::UpdateConnection {
            label: payload.label,
            base_url: payload.base_url,
            api_key: payload.api_key,
            clear_api_key: payload.clear_api_key,
            model: payload.model,
            timeout_seconds: payload.timeout_seconds,
        },
    )
    .await?;
    Ok(Json(build_settings_response(&state).await?))
}

async fn delete_connection_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_operator_csrf(&state, &headers).await?;
    crate::providers::delete_connection(&state.pool, &id).await?;
    Ok(Json(build_settings_response(&state).await?))
}

async fn activate_connection_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_operator_csrf(&state, &headers).await?;
    crate::providers::activate_connection(&state.pool, &id).await?;
    Ok(Json(build_settings_response(&state).await?))
}

async fn get_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
  let _session = crate::auth::operator::require_operator(&state, &headers).await?;
  Ok(Json(build_settings_response(&state).await?))
}

/// Deep-merge an incoming per-provider search-config map onto the stored one.
/// For each provider present in `incoming`, overlay its fields onto the stored
/// provider block, but SKIP any incoming field whose value is an empty string so
/// a blank apiKey box keeps the stored key (the client never sees stored keys —
/// GET redacts them). Providers absent from `incoming` are left untouched.
pub(crate) fn merge_search_provider_configs(
    stored: &serde_json::Value,
    incoming: &serde_json::Value,
) -> serde_json::Value {
    let mut out = stored.as_object().cloned().unwrap_or_default();
    if let Some(incoming_obj) = incoming.as_object() {
        for (provider, fields) in incoming_obj {
            let mut block = out
                .get(provider)
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();
            if let Some(field_obj) = fields.as_object() {
                for (key, value) in field_obj {
                    // Blank string = "keep whatever is stored" (do not overwrite).
                    if value.as_str() == Some("") {
                        continue;
                    }
                    block.insert(key.clone(), value.clone());
                }
            }
            out.insert(provider.clone(), serde_json::Value::Object(block));
        }
    }
    serde_json::Value::Object(out)
}

async fn update_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
  require_operator_csrf(&state, &headers).await?;
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

  if let Some(image) = &payload.image {
      sqlx::query(
          "UPDATE system_settings
           SET image_base_url = COALESCE($1, image_base_url),
               image_api_key = COALESCE(NULLIF($2, ''), CASE WHEN $3 THEN NULL ELSE image_api_key END),
               image_model = COALESCE($4, image_model),
               image_size = COALESCE($5, image_size),
               image_timeout_seconds = COALESCE($6, image_timeout_seconds)
           WHERE id = 1",
      )
      .bind(image.base_url.as_deref())
      .bind(image.api_key.as_deref())
      .bind(image.clear_api_key)
      .bind(image.model.as_deref())
      .bind(image.size.as_deref())
      .bind(image.timeout_seconds)
      .execute(&state.pool)
      .await
      .map_err(ApiError::from)?;
  }

  if let Some(embedding) = &payload.embedding {
      sqlx::query(
          "UPDATE system_settings
           SET embedding_enabled = COALESCE($1, embedding_enabled),
               embedding_base_url = COALESCE($2, embedding_base_url),
               embedding_api_key = COALESCE(NULLIF($3, ''), CASE WHEN $4 THEN NULL ELSE embedding_api_key END),
               embedding_model = COALESCE($5, embedding_model),
               embedding_timeout_seconds = COALESCE($6, embedding_timeout_seconds)
           WHERE id = 1",
      )
      .bind(embedding.enabled)
      .bind(embedding.base_url.as_deref())
      .bind(embedding.api_key.as_deref())
      .bind(embedding.clear_api_key)
      .bind(embedding.model.as_deref())
      .bind(embedding.timeout_seconds)
      .execute(&state.pool)
      .await
      .map_err(ApiError::from)?;
  }

  if let Some(search) = &payload.search {
      if let Some(provider) = &search.provider {
          // Reject unknown selectors at write time so the failure surfaces here
          // as a 400 instead of a later 500 when web-search parses the stored value.
          if !matches!(
              provider.as_str(),
              "none" | "tavily" | "serpapi" | "searxng" | "ollama"
          ) {
              return Err(ApiError::bad_request(format!(
                  "unknown search provider: {provider:?}"
              )));
          }
          sqlx::query("UPDATE system_settings SET search_provider = $1 WHERE id = 1")
              .bind(provider)
              .execute(&state.pool)
              .await
              .map_err(ApiError::from)?;
      }
      // Deep-merge the incoming per-provider map into the stored one so fields
      // the client omits (notably a configured apiKey it never sees, because
      // GET redacts it) are preserved. Whole-object replacement would destroy
      // stored keys whenever any field is edited or the provider is switched.
      if let Some(incoming) = &search.providers {
          let existing: serde_json::Value = sqlx::query_scalar(
              "SELECT search_provider_configs FROM system_settings WHERE id = 1",
          )
          .fetch_one(&state.pool)
          .await
          .map_err(ApiError::from)?;
          let merged = merge_search_provider_configs(&existing, incoming);
          sqlx::query("UPDATE system_settings SET search_provider_configs = $1 WHERE id = 1")
              .bind(merged)
              .execute(&state.pool)
              .await
              .map_err(ApiError::from)?;
      }
  }

  if let Some(defaults) = &payload.defaults {
      sqlx::query(
          "UPDATE system_settings
           SET language = COALESCE($1, language),
               default_query_limit = COALESCE($2, default_query_limit)
           WHERE id = 1",
      )
      .bind(defaults.language.as_deref())
      .bind(defaults.default_query_limit)
      .execute(&state.pool)
      .await
      .map_err(ApiError::from)?;
  }

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

    use super::{CreateConnectionRequest, UpdateConnectionRequest};

    #[test]
    fn create_connection_request_parses_camel_case() {
        let req: CreateConnectionRequest = serde_json::from_str(
            r#"{"label":"OpenAI","baseUrl":"https://api.openai.com","apiKey":"sk","model":"gpt-4o","timeoutSeconds":30}"#,
        )
        .unwrap();
        assert_eq!(req.label, "OpenAI");
        assert_eq!(req.base_url, "https://api.openai.com");
        assert_eq!(req.model, "gpt-4o");
        assert_eq!(req.timeout_seconds, Some(30));
    }

    #[test]
    fn update_connection_request_defaults_clear_flag_false() {
        let req: UpdateConnectionRequest = serde_json::from_str(
            r#"{"label":"L","baseUrl":"u","model":"m"}"#,
        )
        .unwrap();
        assert_eq!(req.clear_api_key, false);
        assert!(req.api_key.is_none());
    }

    use super::UpdateSettingsRequest;

    #[test]
    fn update_settings_request_parses_capability_blocks() {
        let req: UpdateSettingsRequest = serde_json::from_str(
            r#"{
              "providerMode":"openai-compatible","language":"en","defaultQueryLimit":8,
              "image":{"baseUrl":"https://img.example.com","model":"gpt-image-1","size":"512x512","timeoutSeconds":60},
              "embedding":{"enabled":true,"baseUrl":"https://emb.example.com","model":"text-embedding-3-small"},
              "search":{"provider":"tavily","providers":{"tavily":{"apiKey":"tav","baseUrl":"https://api.tavily.com"}}}
            }"#,
        )
        .unwrap();
        let image = req.image.expect("image block");
        assert_eq!(image.model.as_deref(), Some("gpt-image-1"));
        assert_eq!(image.size.as_deref(), Some("512x512"));
        let embedding = req.embedding.expect("embedding block");
        assert_eq!(embedding.enabled, Some(true));
        let search = req.search.expect("search block");
        assert_eq!(search.provider.as_deref(), Some("tavily"));
    }

    use super::merge_search_provider_configs;

    #[test]
    fn merge_overlays_incoming_fields_and_preserves_stored_key() {
        use serde_json::json;
        let stored = json!({
            "tavily": { "apiKey": "stored-key", "baseUrl": "https://api.tavily.com" }
        });
        // The client resends the redacted-then-edited block: no apiKey (it never
        // saw it) but a changed baseUrl.
        let incoming = json!({
            "tavily": { "baseUrl": "https://tavily.local" }
        });
        let merged = merge_search_provider_configs(&stored, &incoming);
        assert_eq!(merged["tavily"]["apiKey"], "stored-key");
        assert_eq!(merged["tavily"]["baseUrl"], "https://tavily.local");
    }

    #[test]
    fn merge_blank_apikey_keeps_stored_and_nonblank_replaces() {
        use serde_json::json;
        let stored = json!({ "tavily": { "apiKey": "old" } });
        // Blank string means "keep": stored key survives.
        let kept = merge_search_provider_configs(&stored, &json!({ "tavily": { "apiKey": "" } }));
        assert_eq!(kept["tavily"]["apiKey"], "old");
        // Non-blank string replaces.
        let replaced = merge_search_provider_configs(&stored, &json!({ "tavily": { "apiKey": "new" } }));
        assert_eq!(replaced["tavily"]["apiKey"], "new");
    }

    #[test]
    fn merge_adds_provider_absent_from_stored() {
        use serde_json::json;
        let merged = merge_search_provider_configs(
            &json!({}),
            &json!({ "searxng": { "url": "https://searx.local" } }),
        );
        assert_eq!(merged["searxng"]["url"], "https://searx.local");
    }
}
