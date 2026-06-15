use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::http::error::ApiError;
use crate::web_search::config::WebSearchConfig;
use crate::web_search::{WebSearchProvider, WebSearchResult};

const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

pub async fn web_search(
  config: &WebSearchConfig,
  query: &str,
  max_results: usize,
) -> Result<Vec<WebSearchResult>, ApiError> {
  if config.requires_api_key() && config.api_key.as_deref().unwrap_or("").is_empty() {
    return Err(ApiError::bad_request(format!(
      "web search provider \"{}\" requires an api key; configure one in Settings",
      config.provider.as_str()
    )));
  }
  match config.provider {
    WebSearchProvider::Tavily => tavily_search(config, query, max_results).await,
    WebSearchProvider::SerpApi => Err(ApiError::internal("serpapi not yet implemented")),
    WebSearchProvider::SearXng => Err(ApiError::internal("searxng not yet implemented")),
    WebSearchProvider::Ollama => Err(ApiError::internal("ollama not yet implemented")),
  }
}

fn build_client() -> Result<Client, ApiError> {
  Client::builder()
    .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
    .build()
    .map_err(|error| ApiError::internal(format!("web search client init failed: {error}")))
}

fn hostname_from_url(url: &str) -> String {
  match url::Url::parse(url) {
    Ok(parsed) => parsed
      .host_str()
      .map(|host| host.trim_start_matches("www.").to_string())
      .unwrap_or_default(),
    Err(_) => String::new(),
  }
}

#[derive(Debug, Deserialize)]
struct TavilyResponse {
  #[serde(default)]
  results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
  #[serde(default)]
  title: Option<String>,
  #[serde(default)]
  url: Option<String>,
  #[serde(default)]
  content: Option<String>,
}

async fn tavily_search(
  config: &WebSearchConfig,
  query: &str,
  max_results: usize,
) -> Result<Vec<WebSearchResult>, ApiError> {
  let api_key = config.api_key.as_deref().unwrap_or("");
  let client = build_client()?;
  let body = serde_json::json!({
    "api_key": api_key,
    "query": query,
    "max_results": max_results,
    "search_depth": "advanced",
    "include_answer": false,
  });
  let url = format!("{}/search", config.tavily_base_url.trim_end_matches('/'));
  let response = client
    .post(&url)
    .header("Content-Type", "application/json")
    .json(&body)
    .send()
    .await
    .map_err(|error| ApiError::bad_request(format!("tavily request failed: {error}")))?;
  if !response.status().is_success() {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    return Err(ApiError::bad_request(format!(
      "tavily search failed ({status}): {text}"
    )));
  }
  let parsed: TavilyResponse = response
    .json()
    .await
    .map_err(|error| ApiError::bad_request(format!("tavily response parse failed: {error}")))?;

  Ok(
    parsed
      .results
      .into_iter()
      .take(max_results)
      .map(|item| {
        let url = item.url.unwrap_or_default();
        WebSearchResult {
          title: item.title.unwrap_or_else(|| "Untitled".to_string()),
          url: url.clone(),
          snippet: item.content.unwrap_or_default(),
          source: hostname_from_url(&url),
        }
      })
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_config(base_url: &str) -> WebSearchConfig {
    WebSearchConfig {
      provider: WebSearchProvider::Tavily,
      api_key: Some("test-key".to_string()),
      serpapi_engine: "google".to_string(),
      searxng_url: None,
      searxng_categories: vec!["general".to_string()],
      ollama_search_url: "https://ollama.com".to_string(),
      tavily_base_url: base_url.to_string(),
      serpapi_base_url: base_url.to_string(),
    }
  }

  async fn spawn_mock_tavily() -> (tokio::task::JoinHandle<()>, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
      if let Ok((mut socket, _)) = listener.accept().await {
        let mut buf = vec![0u8; 4096];
        let _ = socket.read(&mut buf).await;
        let body = serde_json::json!({
          "results": [
            {"title": "Demo", "url": "https://example.com/demo", "content": "snippet text"},
            {"title": "Demo 2", "url": "https://other.example.com/page", "content": "second"}
          ]
        })
        .to_string();
        let payload = format!(
          "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
          body.len(),
          body
        );
        let _ = socket.write_all(payload.as_bytes()).await;
        let _ = socket.shutdown().await;
      }
    });
    (handle, format!("http://127.0.0.1:{port}"))
  }

  #[tokio::test]
  async fn tavily_normalizes_results_and_extracts_hostname() {
    let (handle, base) = spawn_mock_tavily().await;
    let config = test_config(&base);
    let results = web_search(&config, "query", 10).await.unwrap();
    handle.await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Demo");
    assert_eq!(results[0].url, "https://example.com/demo");
    assert_eq!(results[0].snippet, "snippet text");
    assert_eq!(results[0].source, "example.com");
    assert_eq!(results[1].source, "other.example.com");
  }

  #[tokio::test]
  async fn web_search_rejects_when_api_key_missing() {
    let mut config = test_config("http://127.0.0.1:1");
    config.api_key = None;
    let err = web_search(&config, "query", 10).await.unwrap_err();
    assert!(err.to_string().contains("requires an api key"));
  }
}
