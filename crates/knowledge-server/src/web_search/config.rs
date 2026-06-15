use serde_json::Value;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::web_search::WebSearchProvider;

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
}

impl WebSearchConfig {
  pub fn requires_api_key(&self) -> bool {
    matches!(
      self.provider,
      WebSearchProvider::Tavily | WebSearchProvider::SerpApi | WebSearchProvider::Ollama,
    )
  }
}

pub(crate) fn parse_provider(value: &str) -> Option<WebSearchProvider> {
  match value {
    "tavily" => Some(WebSearchProvider::Tavily),
    "serpapi" => Some(WebSearchProvider::SerpApi),
    "searxng" => Some(WebSearchProvider::SearXng),
    "ollama" => Some(WebSearchProvider::Ollama),
    _ => None,
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

pub async fn load_web_search_config(state: &AppState) -> Result<Option<WebSearchConfig>, ApiError> {
  let (
    search_provider,
    search_api_key,
    serpapi_engine,
    searxng_url,
    searxng_categories,
    ollama_search_url,
  ) = sqlx::query_as::<
    _,
    (String, Option<String>, Option<String>, Option<String>, Value, Option<String>),
  >(
    "SELECT
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

  let Some(provider) = parse_provider(&search_provider) else {
    return Ok(None);
  };

  Ok(Some(WebSearchConfig {
    provider,
    api_key: search_api_key.filter(|value| !value.is_empty()),
    serpapi_engine: serpapi_engine.unwrap_or_else(|| "google".to_string()),
    searxng_url: searxng_url.filter(|value| !value.trim().is_empty()),
    searxng_categories: parse_categories(&searxng_categories),
    ollama_search_url: ollama_search_url
      .filter(|value| !value.trim().is_empty())
      .unwrap_or_else(|| "https://ollama.com".to_string()),
    tavily_base_url: "https://api.tavily.com".to_string(),
    serpapi_base_url: "https://serpapi.com".to_string(),
  }))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_provider_maps_known_values() {
    assert_eq!(parse_provider("tavily"), Some(WebSearchProvider::Tavily));
    assert_eq!(parse_provider("serpapi"), Some(WebSearchProvider::SerpApi));
    assert_eq!(parse_provider("searxng"), Some(WebSearchProvider::SearXng));
    assert_eq!(parse_provider("ollama"), Some(WebSearchProvider::Ollama));
  }

  #[test]
  fn parse_provider_returns_none_for_sentinel_and_unknown() {
    assert!(parse_provider("none").is_none());
    assert!(parse_provider("").is_none());
    assert!(parse_provider("google").is_none());
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
}
