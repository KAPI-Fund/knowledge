use serde_json::Value;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::web_fetch::FetchProvider;

pub const DEFAULT_FIRECRAWL_BASE_URL: &str = "https://api.firecrawl.dev";

#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub provider: FetchProvider,
    pub base_url: String,
    pub api_key: Option<String>,
}

/// "none" -> Ok(None); "firecrawl" -> Ok(Some(Firecrawl)); anything else is an
/// internal error (the selector column is validated on write).
pub(crate) fn parse_fetch_provider(value: &str) -> Result<Option<FetchProvider>, ApiError> {
    match value {
        "none" => Ok(None),
        "firecrawl" => Ok(Some(FetchProvider::Firecrawl)),
        other => Err(ApiError::internal(format!(
            "unknown fetch provider stored in system_settings: {other}"
        ))),
    }
}

/// Pure resolver: reads configs["firecrawl"] { baseUrl, apiKey } and applies the
/// base-url default when the stored value is blank/absent.
pub(crate) fn resolve_fetch_config(
    active: &str,
    configs: &Value,
) -> Result<Option<FetchConfig>, ApiError> {
    let Some(provider) = parse_fetch_provider(active)? else {
        return Ok(None);
    };

    let str_field = |field: &str| -> Option<String> {
        configs
            .get("firecrawl")
            .and_then(|block| block.get(field))
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|v| !v.trim().is_empty())
    };

    let base_url = str_field("baseUrl").unwrap_or_else(|| DEFAULT_FIRECRAWL_BASE_URL.to_string());
    let api_key = str_field("apiKey");

    Ok(Some(FetchConfig {
        provider,
        base_url,
        api_key,
    }))
}

/// SELECT the selector + config map from the singleton row, then resolve.
pub async fn load_fetch_config(state: &AppState) -> Result<Option<FetchConfig>, ApiError> {
    let (provider, configs): (String, Value) = sqlx::query_as(
        "SELECT fetch_provider, fetch_provider_configs FROM system_settings WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|err| ApiError::internal(format!("failed to load fetch provider settings: {err}")))?;

    resolve_fetch_config(&provider, &configs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_fetch_provider_maps_known_values() {
        assert_eq!(parse_fetch_provider("none").unwrap(), None);
        assert_eq!(
            parse_fetch_provider("firecrawl").unwrap(),
            Some(FetchProvider::Firecrawl)
        );
    }

    #[test]
    fn parse_fetch_provider_rejects_unknown() {
        assert!(parse_fetch_provider("wget").is_err());
    }

    #[test]
    fn resolve_returns_none_for_none_selector() {
        let configs = json!({ "firecrawl": { "baseUrl": "http://x", "apiKey": "k" } });
        assert!(resolve_fetch_config("none", &configs).unwrap().is_none());
    }

    #[test]
    fn resolve_reads_firecrawl_block() {
        let configs = json!({ "firecrawl": { "baseUrl": "http://host:3002", "apiKey": "sk-1" } });
        let cfg = resolve_fetch_config("firecrawl", &configs).unwrap().unwrap();
        assert_eq!(cfg.provider, FetchProvider::Firecrawl);
        assert_eq!(cfg.base_url, "http://host:3002");
        assert_eq!(cfg.api_key.as_deref(), Some("sk-1"));
    }

    #[test]
    fn resolve_applies_base_url_default_when_absent() {
        let configs = json!({ "firecrawl": { "apiKey": "sk-1" } });
        let cfg = resolve_fetch_config("firecrawl", &configs).unwrap().unwrap();
        assert_eq!(cfg.base_url, DEFAULT_FIRECRAWL_BASE_URL);
        assert_eq!(cfg.api_key.as_deref(), Some("sk-1"));
    }

    #[test]
    fn resolve_applies_base_url_default_when_blank() {
        let configs = json!({ "firecrawl": { "baseUrl": "   ", "apiKey": "sk-1" } });
        let cfg = resolve_fetch_config("firecrawl", &configs).unwrap().unwrap();
        assert_eq!(cfg.base_url, DEFAULT_FIRECRAWL_BASE_URL);
    }

    #[test]
    fn resolve_api_key_absent_is_none() {
        let configs = json!({ "firecrawl": { "baseUrl": "http://host:3002" } });
        let cfg = resolve_fetch_config("firecrawl", &configs).unwrap().unwrap();
        assert_eq!(cfg.api_key, None);
    }
}
