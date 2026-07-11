use serde_json::Value;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::web_search::WebSearchProvider;

const DEFAULT_TAVILY_BASE_URL: &str = "https://api.tavily.com";
const DEFAULT_SERPAPI_BASE_URL: &str = "https://serpapi.com";
const DEFAULT_OLLAMA_SEARCH_URL: &str = "https://ollama.com";
const DEFAULT_BRAVE_BASE_URL: &str = "https://api.search.brave.com";
const DEFAULT_FIRECRAWL_SEARCH_BASE_URL: &str = "https://api.firecrawl.dev";

#[derive(Debug, Clone)]
pub struct WebSearchConfig {
  pub provider: WebSearchProvider,
  pub api_key: Option<String>,
  pub serpapi_engine: String,
  pub searxng_url: Option<String>,
  pub searxng_categories: Vec<String>,
  pub ollama_search_url: String,
  /// Overrideable base URL for tavily — defaults to the public endpoint.
  /// Tests inject a localhost mock via this field.
  pub tavily_base_url: String,
  /// Overrideable base URL for serpapi — defaults to the public endpoint.
  pub serpapi_base_url: String,
  /// Overrideable base URL for brave — defaults to the public endpoint.
  pub brave_base_url: String,
  /// Overrideable base URL for firecrawl search — defaults to the public endpoint.
  pub firecrawl_base_url: String,
}

impl WebSearchConfig {
  // Firecrawl stays keyless-capable: its api key is optional (Bearer only when set).
  pub fn requires_api_key(&self) -> bool {
    matches!(
      self.provider,
      WebSearchProvider::Tavily
        | WebSearchProvider::SerpApi
        | WebSearchProvider::Ollama
        | WebSearchProvider::Brave,
    )
  }
}

pub(crate) fn parse_provider(value: &str) -> Result<Option<WebSearchProvider>, ApiError> {
  match value {
    "none" => Ok(None),
    "tavily" => Ok(Some(WebSearchProvider::Tavily)),
    "serpapi" => Ok(Some(WebSearchProvider::SerpApi)),
    "searxng" => Ok(Some(WebSearchProvider::SearXng)),
    "ollama" => Ok(Some(WebSearchProvider::Ollama)),
    "brave" => Ok(Some(WebSearchProvider::Brave)),
    "firecrawl" => Ok(Some(WebSearchProvider::Firecrawl)),
    other => Err(ApiError::internal(format!(
      "invalid search_provider in system_settings: {other:?}"
    ))),
  }
}

pub(crate) fn parse_categories(value: &Value) -> Vec<String> {
  let parsed = value
    .as_array()
    .map(|items| {
      items
        .iter()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect::<Vec<_>>()
    })
    .unwrap_or_default();
  if parsed.is_empty() {
    vec!["general".to_string()]
  } else {
    parsed
  }
}

/// Pure: resolve the active provider's WebSearchConfig from the per-provider
/// JSONB map. `active` is the search_provider selector; `configs` is
/// search_provider_configs. Returns Ok(None) when the selector is "none".
pub(crate) fn resolve_web_search_config(
  active: &str,
  configs: &Value,
) -> Result<Option<WebSearchConfig>, ApiError> {
  let Some(provider) = parse_provider(active)? else {
    return Ok(None);
  };

  let block = |key: &str| configs.get(key);
  let str_field = |key: &str, field: &str| -> Option<String> {
    block(key)
      .and_then(|b| b.get(field))
      .and_then(Value::as_str)
      .map(str::to_string)
      .filter(|value| !value.trim().is_empty())
  };

  let api_key = match provider {
    WebSearchProvider::Tavily => str_field("tavily", "apiKey"),
    WebSearchProvider::SerpApi => str_field("serpapi", "apiKey"),
    WebSearchProvider::Ollama => str_field("ollama", "apiKey"),
    WebSearchProvider::Brave => str_field("brave", "apiKey"),
    WebSearchProvider::Firecrawl => str_field("firecrawl", "apiKey"),
    WebSearchProvider::SearXng => None,
  };

  let searxng_categories = block("searxng")
    .and_then(|b| b.get("categories"))
    .map(parse_categories)
    .unwrap_or_else(|| vec!["general".to_string()]);

  Ok(Some(WebSearchConfig {
    provider,
    api_key,
    serpapi_engine: str_field("serpapi", "engine").unwrap_or_else(|| "google".to_string()),
    searxng_url: str_field("searxng", "url"),
    searxng_categories,
    ollama_search_url: str_field("ollama", "url")
      .unwrap_or_else(|| DEFAULT_OLLAMA_SEARCH_URL.to_string()),
    tavily_base_url: str_field("tavily", "baseUrl")
      .unwrap_or_else(|| DEFAULT_TAVILY_BASE_URL.to_string()),
    serpapi_base_url: str_field("serpapi", "baseUrl")
      .unwrap_or_else(|| DEFAULT_SERPAPI_BASE_URL.to_string()),
    brave_base_url: str_field("brave", "baseUrl")
      .unwrap_or_else(|| DEFAULT_BRAVE_BASE_URL.to_string()),
    firecrawl_base_url: str_field("firecrawl", "baseUrl")
      .unwrap_or_else(|| DEFAULT_FIRECRAWL_SEARCH_BASE_URL.to_string()),
  }))
}

pub async fn load_web_search_config(state: &AppState) -> Result<Option<WebSearchConfig>, ApiError> {
  let (search_provider, search_provider_configs) =
    sqlx::query_as::<_, (String, Value)>(
      "SELECT search_provider, search_provider_configs
       FROM system_settings
       WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

  resolve_web_search_config(&search_provider, &search_provider_configs)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_provider_maps_known_values() {
    assert_eq!(parse_provider("tavily").unwrap(), Some(WebSearchProvider::Tavily));
    assert_eq!(parse_provider("serpapi").unwrap(), Some(WebSearchProvider::SerpApi));
    assert_eq!(parse_provider("searxng").unwrap(), Some(WebSearchProvider::SearXng));
    assert_eq!(parse_provider("ollama").unwrap(), Some(WebSearchProvider::Ollama));
    assert_eq!(parse_provider("brave").unwrap(), Some(WebSearchProvider::Brave));
    assert_eq!(parse_provider("firecrawl").unwrap(), Some(WebSearchProvider::Firecrawl));
  }

  #[test]
  fn parse_provider_treats_none_sentinel_as_disabled() {
    assert!(parse_provider("none").unwrap().is_none());
  }

  #[test]
  fn parse_provider_errors_on_invalid_value() {
    assert!(parse_provider("google").is_err());
    assert!(parse_provider("").is_err());
  }

  #[test]
  fn parse_categories_handles_array_and_string_fallback() {
    use serde_json::json;
    let parsed = parse_categories(&json!(["general", "news"]));
    assert_eq!(parsed, vec!["general".to_string(), "news".to_string()]);

    let fallback = parse_categories(&json!("not an array"));
    assert_eq!(fallback, vec!["general".to_string()]);

    let empty = parse_categories(&json!([]));
    assert_eq!(empty, vec!["general".to_string()]);
  }

  #[test]
  fn resolve_reads_active_provider_block_from_jsonb() {
    use serde_json::json;
    let configs = json!({
      "tavily": { "apiKey": "tav-key", "baseUrl": "https://tavily.local" }
    });
    let cfg = resolve_web_search_config("tavily", &configs)
      .unwrap()
      .expect("configured");
    assert!(matches!(cfg.provider, WebSearchProvider::Tavily));
    assert_eq!(cfg.api_key.as_deref(), Some("tav-key"));
    assert_eq!(cfg.tavily_base_url, "https://tavily.local");
  }

  #[test]
  fn resolve_none_provider_is_disabled() {
    use serde_json::json;
    assert!(resolve_web_search_config("none", &json!({})).unwrap().is_none());
  }

  #[test]
  fn resolve_defaults_when_block_absent() {
    use serde_json::json;
    let cfg = resolve_web_search_config("tavily", &json!({}))
      .unwrap()
      .expect("configured");
    assert_eq!(cfg.tavily_base_url, "https://api.tavily.com");
    assert!(cfg.api_key.is_none());
  }

  #[test]
  fn resolve_brave_reads_key_and_base_url() {
    use serde_json::json;
    let configs = json!({
      "brave": { "apiKey": "brave-key", "baseUrl": "https://brave.local" }
    });
    let cfg = resolve_web_search_config("brave", &configs)
      .unwrap()
      .expect("configured");
    assert!(matches!(cfg.provider, WebSearchProvider::Brave));
    assert!(cfg.requires_api_key());
    assert_eq!(cfg.api_key.as_deref(), Some("brave-key"));
    assert_eq!(cfg.brave_base_url, "https://brave.local");

    let defaulted = resolve_web_search_config("brave", &json!({}))
      .unwrap()
      .expect("configured");
    assert_eq!(defaulted.brave_base_url, "https://api.search.brave.com");
  }

  #[test]
  fn resolve_firecrawl_key_is_optional() {
    use serde_json::json;
    let cfg = resolve_web_search_config("firecrawl", &json!({}))
      .unwrap()
      .expect("configured");
    assert!(matches!(cfg.provider, WebSearchProvider::Firecrawl));
    assert!(!cfg.requires_api_key());
    assert!(cfg.api_key.is_none());
    assert_eq!(cfg.firecrawl_base_url, "https://api.firecrawl.dev");

    let keyed = resolve_web_search_config(
      "firecrawl",
      &json!({ "firecrawl": { "apiKey": "fc-key", "baseUrl": "https://fc.local" } }),
    )
    .unwrap()
    .expect("configured");
    assert_eq!(keyed.api_key.as_deref(), Some("fc-key"));
    assert_eq!(keyed.firecrawl_base_url, "https://fc.local");
  }

  #[test]
  fn resolve_searxng_reads_url_and_categories() {
    use serde_json::json;
    let configs = json!({
      "searxng": { "url": "https://searx.local", "categories": ["news"] }
    });
    let cfg = resolve_web_search_config("searxng", &configs)
      .unwrap()
      .expect("configured");
    assert_eq!(cfg.searxng_url.as_deref(), Some("https://searx.local"));
    assert_eq!(cfg.searxng_categories, vec!["news".to_string()]);
  }
}
