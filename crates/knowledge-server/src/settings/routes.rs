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
  #[serde(default)]
  pub image: Option<ImageSettingsBlock>,
  #[serde(default)]
  pub embedding: Option<EmbeddingSettingsBlock>,
  #[serde(default)]
  pub search: Option<SearchSettingsBlock>,
  #[serde(default)]
  pub fetch: Option<FetchSettingsBlock>,
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
pub struct FetchSettingsBlock {
    #[serde(default)]
    pub provider: Option<String>,
    /// Full per-provider config map (`{ firecrawl: { baseUrl, apiKey } }`), merged
    /// client-side and stored verbatim into fetch_provider_configs.
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

/// Return the per-provider fetch config with the firecrawl apiKey replaced by an
/// apiKeyConfigured boolean, so raw keys never leave the server. baseUrl passes
/// through unchanged.
fn redact_fetch_configs(configs: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    if let Some(fc) = configs.get("firecrawl") {
        let mut block = serde_json::Map::new();
        let configured = fc
            .get("apiKey")
            .and_then(serde_json::Value::as_str)
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false);
        block.insert("apiKeyConfigured".into(), serde_json::Value::Bool(configured));
        if let Some(base) = fc.get("baseUrl") {
            block.insert("baseUrl".into(), base.clone());
        }
        out.insert("firecrawl".into(), serde_json::Value::Object(block));
    }
    serde_json::Value::Object(out)
}

async fn build_settings_response(state: &AppState) -> Result<serde_json::Value, ApiError> {
    let (language, default_query_limit, search_provider, fetch_provider) =
        sqlx::query_as::<_, (String, i64, String, String)>(
            "SELECT language, default_query_limit, search_provider, fetch_provider
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
        fetch_provider_configs,
    ) = sqlx::query_as::<_, (
        bool, Option<String>, Option<String>, Option<String>, Option<i64>,
        Option<String>, Option<String>, Option<String>, String, Option<i64>,
        serde_json::Value,
        serde_json::Value,
    )>(
        "SELECT embedding_enabled, embedding_base_url, embedding_api_key,
                embedding_model, embedding_timeout_seconds,
                image_base_url, image_api_key, image_model, image_size, image_timeout_seconds,
                search_provider_configs, fetch_provider_configs
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
    // Same for the fetch JSONB (firecrawl apiKey -> apiKeyConfigured).
    let fetch_providers = redact_fetch_configs(&fetch_provider_configs);

    Ok(json!({
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
        "fetch": {
            "provider": fetch_provider,
            "providers": fetch_providers,
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

/// Deep-merge an incoming per-provider config map onto the stored one. Shared by
/// the search and fetch provider blocks (both store a `{ provider: { field: ... } }`
/// JSONB map with identical merge semantics). For each provider present in
/// `incoming`, overlay its fields onto the stored provider block, per-field:
/// - blank string (`""`) = KEEP the stored value (a blank apiKey box must not
///   wipe the stored key the client can't see — GET redacts stored keys),
/// - explicit `null` = CLEAR the field (drops it so the provider default / unset
///   applies; this is the only way to remove a stored key or custom base URL),
/// - any other value = SET it.
/// Providers absent from `incoming` are left untouched.
pub(crate) fn merge_provider_configs(
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
                    match value {
                        // Explicit null = clear this field.
                        serde_json::Value::Null => {
                            block.remove(key);
                        }
                        // Blank string = keep whatever is stored (do not overwrite).
                        serde_json::Value::String(s) if s.is_empty() => {}
                        other => {
                            block.insert(key.clone(), other.clone());
                        }
                    }
                }
            }
            out.insert(provider.clone(), serde_json::Value::Object(block));
        }
    }
    serde_json::Value::Object(out)
}

/// Accept only the search selectors the web-search layer can parse. Validated
/// before any write so an unknown value fails as a 400 up front instead of
/// leaving a partial update behind (and a later 500 when web-search parses it).
fn validate_search_provider(provider: &str) -> Result<(), ApiError> {
    if matches!(
        provider,
        "none" | "tavily" | "serpapi" | "searxng" | "ollama"
    ) {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "unknown search provider: {provider:?}"
        )))
    }
}

/// Accept only the fetch selectors the canvas URL extractor can parse. Validated
/// before any write, mirroring `validate_search_provider`.
fn validate_fetch_provider(provider: &str) -> Result<(), ApiError> {
    match provider {
        "none" | "firecrawl" => Ok(()),
        other => Err(ApiError::bad_request(format!(
            "unknown fetch provider: {other}"
        ))),
    }
}

/// The default number of retrieval results must be a positive integer no larger
/// than the search layer's hard cap (`MAX_RESULTS = 100` in knowledge-core). A
/// zero/negative limit would silently return no context; anything above the cap
/// is clamped downstream anyway, so reject it up front rather than storing a
/// misleading value.
const MAX_DEFAULT_QUERY_LIMIT: i64 = 100;

fn validate_default_query_limit(limit: i64) -> Result<(), ApiError> {
    if (1..=MAX_DEFAULT_QUERY_LIMIT).contains(&limit) {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "defaultQueryLimit must be between 1 and {MAX_DEFAULT_QUERY_LIMIT}"
        )))
    }
}

async fn update_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
  require_operator_csrf(&state, &headers).await?;

  // Validate everything that can be rejected BEFORE opening the transaction, so
  // an invalid request never produces a partial write.
  if let Some(provider) = payload.search.as_ref().and_then(|s| s.provider.as_ref()) {
      validate_search_provider(provider)?;
  }
  if let Some(provider) = payload.fetch.as_ref().and_then(|f| f.provider.as_ref()) {
      validate_fetch_provider(provider)?;
  }
  if let Some(limit) = payload.defaults.as_ref().and_then(|d| d.default_query_limit) {
      validate_default_query_limit(limit)?;
  }

  // All writes share one transaction: any failure rolls the whole PATCH back
  // instead of leaving some capability blocks updated and others not.
  let mut tx = state.pool.begin().await.map_err(ApiError::from)?;

  if let Some(image) = &payload.image {
      // Validate the EFFECTIVE model+size: the UPDATE below keeps the stored value
      // for any field the request omits (COALESCE), so an incompatible pair could
      // otherwise slip through when only one of model/size is edited. Reject it
      // here — inside the tx, before the write — so a bad combo rolls back.
      let (stored_model, stored_size): (Option<String>, Option<String>) = sqlx::query_as(
          "SELECT image_model, image_size FROM system_settings WHERE id = 1",
      )
      .fetch_one(&mut *tx)
      .await
      .map_err(ApiError::from)?;
      let effective_model = image
          .model
          .as_deref()
          .map(str::trim)
          .filter(|m| !m.is_empty())
          .map(str::to_string)
          .or(stored_model)
          .unwrap_or_default();
      let effective_size = image
          .size
          .as_deref()
          .map(str::trim)
          .filter(|s| !s.is_empty())
          .map(str::to_string)
          .or(stored_size)
          .unwrap_or_default();
      crate::providers::validate_image_size(&effective_model, &effective_size)?;

      sqlx::query(
          "UPDATE system_settings
           SET image_base_url = COALESCE($1, image_base_url),
               image_api_key = COALESCE(NULLIF($2, ''), CASE WHEN $3 THEN NULL ELSE image_api_key END),
               image_model = COALESCE($4, image_model),
               image_size = COALESCE($5, image_size),
               image_timeout_seconds = COALESCE($6, image_timeout_seconds)
           WHERE id = 1",
      )
      .bind(image.base_url.as_deref().map(str::trim))
      .bind(image.api_key.as_deref())
      .bind(image.clear_api_key)
      .bind(image.model.as_deref().map(str::trim))
      .bind(image.size.as_deref().map(str::trim))
      .bind(image.timeout_seconds)
      .execute(&mut *tx)
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
      .bind(embedding.base_url.as_deref().map(str::trim))
      .bind(embedding.api_key.as_deref())
      .bind(embedding.clear_api_key)
      .bind(embedding.model.as_deref().map(str::trim))
      .bind(embedding.timeout_seconds)
      .execute(&mut *tx)
      .await
      .map_err(ApiError::from)?;
  }

  if let Some(search) = &payload.search {
      if let Some(provider) = &search.provider {
          sqlx::query("UPDATE system_settings SET search_provider = $1 WHERE id = 1")
              .bind(provider)
              .execute(&mut *tx)
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
          .fetch_one(&mut *tx)
          .await
          .map_err(ApiError::from)?;
          let merged = merge_provider_configs(&existing, incoming);
          sqlx::query("UPDATE system_settings SET search_provider_configs = $1 WHERE id = 1")
              .bind(merged)
              .execute(&mut *tx)
              .await
              .map_err(ApiError::from)?;
      }
  }

  if let Some(fetch) = &payload.fetch {
      if let Some(provider) = &fetch.provider {
          sqlx::query("UPDATE system_settings SET fetch_provider = $1 WHERE id = 1")
              .bind(provider)
              .execute(&mut *tx)
              .await
              .map_err(ApiError::from)?;
      }
      // Same deep-merge semantics as search: preserve the stored firecrawl apiKey
      // (redacted out of GET) when the client resends the block without it.
      if let Some(incoming) = &fetch.providers {
          let existing: serde_json::Value = sqlx::query_scalar(
              "SELECT fetch_provider_configs FROM system_settings WHERE id = 1",
          )
          .fetch_one(&mut *tx)
          .await
          .map_err(ApiError::from)?;
          let merged = merge_provider_configs(&existing, incoming);
          sqlx::query("UPDATE system_settings SET fetch_provider_configs = $1 WHERE id = 1")
              .bind(merged)
              .execute(&mut *tx)
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
      .execute(&mut *tx)
      .await
      .map_err(ApiError::from)?;
  }

  tx.commit().await.map_err(ApiError::from)?;

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

    use super::merge_provider_configs;

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
        let merged = merge_provider_configs(&stored, &incoming);
        assert_eq!(merged["tavily"]["apiKey"], "stored-key");
        assert_eq!(merged["tavily"]["baseUrl"], "https://tavily.local");
    }

    #[test]
    fn merge_blank_apikey_keeps_stored_and_nonblank_replaces() {
        use serde_json::json;
        let stored = json!({ "tavily": { "apiKey": "old" } });
        // Blank string means "keep": stored key survives.
        let kept = merge_provider_configs(&stored, &json!({ "tavily": { "apiKey": "" } }));
        assert_eq!(kept["tavily"]["apiKey"], "old");
        // Non-blank string replaces.
        let replaced = merge_provider_configs(&stored, &json!({ "tavily": { "apiKey": "new" } }));
        assert_eq!(replaced["tavily"]["apiKey"], "new");
    }

    #[test]
    fn merge_adds_provider_absent_from_stored() {
        use serde_json::json;
        let merged = merge_provider_configs(
            &json!({}),
            &json!({ "searxng": { "url": "https://searx.local" } }),
        );
        assert_eq!(merged["searxng"]["url"], "https://searx.local");
    }

    #[test]
    fn merge_null_clears_field() {
        use serde_json::json;
        let stored = json!({ "tavily": { "apiKey": "old", "baseUrl": "https://api.tavily.com" } });
        // Explicit null drops just that field; siblings untouched.
        let merged = merge_provider_configs(
            &stored,
            &json!({ "tavily": { "apiKey": null } }),
        );
        assert!(
            merged["tavily"].get("apiKey").is_none(),
            "null must remove the stored apiKey"
        );
        assert_eq!(merged["tavily"]["baseUrl"], "https://api.tavily.com");
    }

    use super::validate_search_provider;

    #[test]
    fn validate_search_provider_accepts_known_and_rejects_unknown() {
        for ok in ["none", "tavily", "serpapi", "searxng", "ollama"] {
            assert!(validate_search_provider(ok).is_ok(), "{ok} should be valid");
        }
        assert!(validate_search_provider("google").is_err());
        assert!(validate_search_provider("").is_err());
    }

    #[test]
    fn validate_default_query_limit_accepts_range_and_rejects_out_of_bounds() {
        use super::validate_default_query_limit;
        for ok in [1, 5, 100] {
            assert!(validate_default_query_limit(ok).is_ok(), "{ok} should be valid");
        }
        assert!(validate_default_query_limit(0).is_err());
        assert!(validate_default_query_limit(-3).is_err());
        assert!(validate_default_query_limit(101).is_err());
    }

    use super::validate_fetch_provider;

    #[test]
    fn validate_fetch_provider_accepts_known_and_rejects_unknown() {
        for ok in ["none", "firecrawl"] {
            assert!(validate_fetch_provider(ok).is_ok(), "{ok} should be valid");
        }
        assert!(validate_fetch_provider("tavily").is_err());
        assert!(validate_fetch_provider("").is_err());
    }

    use super::redact_fetch_configs;

    #[test]
    fn redact_fetch_configs_replaces_apikey_with_boolean_and_keeps_base_url() {
        use serde_json::json;
        let redacted = redact_fetch_configs(&json!({
            "firecrawl": { "apiKey": "fc-secret", "baseUrl": "https://api.firecrawl.dev" }
        }));
        assert_eq!(redacted["firecrawl"]["apiKeyConfigured"], true);
        assert_eq!(redacted["firecrawl"]["baseUrl"], "https://api.firecrawl.dev");
        assert!(
            redacted["firecrawl"].get("apiKey").is_none(),
            "raw firecrawl apiKey must never be serialized"
        );
    }

    #[test]
    fn redact_fetch_configs_reports_blank_key_as_unconfigured() {
        use serde_json::json;
        let redacted = redact_fetch_configs(&json!({ "firecrawl": { "apiKey": "   " } }));
        assert_eq!(redacted["firecrawl"]["apiKeyConfigured"], false);
    }

    #[test]
    fn merge_provider_configs_handles_firecrawl_block() {
        use serde_json::json;
        let stored = json!({ "firecrawl": { "apiKey": "stored", "baseUrl": "https://api.firecrawl.dev" } });
        // Client resends with a changed baseUrl and no apiKey (never saw it).
        let merged = merge_provider_configs(&stored, &json!({ "firecrawl": { "baseUrl": "https://fc.local" } }));
        assert_eq!(merged["firecrawl"]["apiKey"], "stored");
        assert_eq!(merged["firecrawl"]["baseUrl"], "https://fc.local");
    }
}
