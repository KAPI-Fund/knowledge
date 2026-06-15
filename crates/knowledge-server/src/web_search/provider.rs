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
    WebSearchProvider::SerpApi => serpapi_search(config, query, max_results).await,
    WebSearchProvider::SearXng => searxng_search(config, query, max_results).await,
    WebSearchProvider::Ollama => ollama_search(config, query, max_results).await,
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

#[derive(Debug, Deserialize)]
struct SerpApiResponse {
  #[serde(default)]
  organic_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  news_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  images_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  video_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  videos_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  shopping_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerpApiResult {
  #[serde(default)]
  title: Option<String>,
  #[serde(default)]
  link: Option<String>,
  #[serde(default)]
  url: Option<String>,
  #[serde(default)]
  original: Option<String>,
  #[serde(default)]
  thumbnail: Option<String>,
  #[serde(default)]
  snippet: Option<String>,
  #[serde(default)]
  summary: Option<String>,
  #[serde(default)]
  description: Option<String>,
  #[serde(default)]
  source: Option<String>,
  #[serde(default)]
  displayed_link: Option<String>,
}

async fn serpapi_search(
  config: &WebSearchConfig,
  query: &str,
  max_results: usize,
) -> Result<Vec<WebSearchResult>, ApiError> {
  let api_key = config.api_key.as_deref().unwrap_or("");
  let client = build_client()?;
  let url = format!("{}/search", config.serpapi_base_url.trim_end_matches('/'));
  let response = client
    .get(&url)
    .query(&[
      ("engine", config.serpapi_engine.as_str()),
      ("q", query),
      ("api_key", api_key),
      ("num", &max_results.to_string()),
    ])
    .header("Accept", "application/json")
    .send()
    .await
    .map_err(|error| ApiError::bad_request(format!("serpapi request failed: {error}")))?;
  if !response.status().is_success() {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    return Err(ApiError::bad_request(format!(
      "serpapi search failed ({status}): {text}"
    )));
  }
  let parsed: SerpApiResponse = response
    .json()
    .await
    .map_err(|error| ApiError::bad_request(format!("serpapi response parse failed: {error}")))?;

  if let Some(message) = parsed.error.as_deref().filter(|value| !value.trim().is_empty()) {
    return Err(ApiError::bad_request(format!("serpapi search failed: {message}")));
  }

  let raw = parsed
    .organic_results
    .or(parsed.news_results)
    .or(parsed.images_results)
    .or(parsed.video_results)
    .or(parsed.videos_results)
    .or(parsed.shopping_results)
    .unwrap_or_default();

  Ok(
    raw
      .into_iter()
      .take(max_results)
      .map(|item| {
        let url = item
          .link
          .or(item.url)
          .or(item.original)
          .or(item.thumbnail)
          .unwrap_or_default();
        let source = hostname_from_url(&url);
        let source = if source.is_empty() {
          item.source.or(item.displayed_link).unwrap_or_default()
        } else {
          source
        };
        WebSearchResult {
          title: item.title.unwrap_or_else(|| "Untitled".to_string()),
          url,
          snippet: item.snippet.or(item.summary).or(item.description).unwrap_or_default(),
          source,
        }
      })
      .collect(),
  )
}

#[derive(Debug, Deserialize)]
struct SearXngResponse {
  #[serde(default)]
  results: Vec<SearXngResult>,
}

#[derive(Debug, Deserialize)]
struct SearXngResult {
  #[serde(default)]
  title: Option<String>,
  #[serde(default)]
  url: Option<String>,
  #[serde(default)]
  content: Option<String>,
  #[serde(default)]
  engine: Option<String>,
  #[serde(default)]
  category: Option<String>,
}

fn searxng_endpoint(instance_url: &str) -> Result<url::Url, ApiError> {
  let trimmed = instance_url.trim();
  let with_protocol = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
    trimmed.to_string()
  } else {
    format!("https://{trimmed}")
  };
  let mut parsed = url::Url::parse(&with_protocol)
    .map_err(|_| ApiError::bad_request("Invalid SearXNG instance URL"))?;
  let path = parsed.path().trim_end_matches('/').to_string();
  let new_path = if path == "/search" || path.ends_with("/search") {
    if path.is_empty() { "/search".to_string() } else { path }
  } else {
    format!("{path}/search")
  };
  parsed.set_path(&new_path);
  parsed.set_query(None);
  parsed.set_fragment(None);
  Ok(parsed)
}

async fn searxng_search(
  config: &WebSearchConfig,
  query: &str,
  max_results: usize,
) -> Result<Vec<WebSearchResult>, ApiError> {
  let instance = config
    .searxng_url
    .as_deref()
    .ok_or_else(|| ApiError::bad_request("SearXNG instance URL not configured"))?;
  let endpoint = searxng_endpoint(instance)?;
  let client = build_client()?;
  let categories = if config.searxng_categories.is_empty() {
    "general".to_string()
  } else {
    config.searxng_categories.join(",")
  };
  let response = client
    .get(endpoint)
    .query(&[("q", query), ("format", "json"), ("categories", categories.as_str())])
    .header("Accept", "application/json")
    .send()
    .await
    .map_err(|error| ApiError::bad_request(format!("searxng request failed: {error}")))?;
  if !response.status().is_success() {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    return Err(ApiError::bad_request(format!(
      "SearXNG search failed ({status}): {text}"
    )));
  }
  let parsed: SearXngResponse = response
    .json()
    .await
    .map_err(|error| ApiError::bad_request(format!("searxng response parse failed: {error}")))?;

  Ok(
    parsed
      .results
      .into_iter()
      .take(max_results)
      .map(|item| {
        let url = item.url.unwrap_or_default();
        let source = hostname_from_url(&url);
        let source = if source.is_empty() {
          item.engine.or(item.category).unwrap_or_default()
        } else {
          source
        };
        WebSearchResult {
          title: item.title.unwrap_or_else(|| "Untitled".to_string()),
          url: url.clone(),
          snippet: item.content.unwrap_or_default(),
          source,
        }
      })
      .filter(|result| !result.url.is_empty())
      .collect(),
  )
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
  #[serde(default)]
  results: Vec<OllamaResult>,
  #[serde(default)]
  error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaResult {
  #[serde(default)]
  title: Option<String>,
  #[serde(default)]
  url: Option<String>,
  #[serde(default)]
  content: Option<String>,
}

async fn ollama_search(
  config: &WebSearchConfig,
  query: &str,
  max_results: usize,
) -> Result<Vec<WebSearchResult>, ApiError> {
  let api_key = config.api_key.as_deref().unwrap_or("");
  let client = build_client()?;
  let url = format!("{}/api/web_search", config.ollama_search_url.trim_end_matches('/'));
  let response = client
    .post(&url)
    .header("Content-Type", "application/json")
    .header("Authorization", format!("Bearer {api_key}"))
    .json(&serde_json::json!({"query": query, "max_results": max_results}))
    .send()
    .await
    .map_err(|error| ApiError::bad_request(format!("ollama request failed: {error}")))?;
  if !response.status().is_success() {
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
      return Err(ApiError::bad_request(
        "Ollama Web Search API authentication failed. Check your Ollama API key.",
      ));
    }
    let text = response.text().await.unwrap_or_default();
    return Err(ApiError::bad_request(format!(
      "Ollama web search failed ({status}): {text}"
    )));
  }
  let parsed: OllamaResponse = response
    .json()
    .await
    .map_err(|error| ApiError::bad_request(format!("ollama response parse failed: {error}")))?;

  if let Some(message) = parsed.error.as_deref().filter(|value| !value.trim().is_empty()) {
    return Err(ApiError::bad_request(format!("Ollama web search error: {message}")));
  }

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

  async fn spawn_mock_serpapi() -> (tokio::task::JoinHandle<()>, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
      if let Ok((mut socket, _)) = listener.accept().await {
        let mut buf = vec![0u8; 4096];
        let _ = socket.read(&mut buf).await;
        let body = serde_json::json!({
          "organic_results": [
            {"title": "Org", "link": "https://example.com/org", "snippet": "from organic"}
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
  async fn serpapi_picks_organic_results_and_normalizes() {
    let (handle, base) = spawn_mock_serpapi().await;
    let mut config = test_config(&base);
    config.provider = WebSearchProvider::SerpApi;
    config.serpapi_base_url = base.clone();
    let results = web_search(&config, "query", 5).await.unwrap();
    handle.await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Org");
    assert_eq!(results[0].url, "https://example.com/org");
    assert_eq!(results[0].snippet, "from organic");
    assert_eq!(results[0].source, "example.com");
  }

  async fn spawn_mock_searxng() -> (tokio::task::JoinHandle<()>, String) {
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
            {"title": "Sx", "url": "https://example.com/sx", "content": "from searxng", "engine": "duckduckgo"},
            {"title": "Sx2", "url": "", "content": "no url", "engine": "google"}
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
  async fn searxng_normalizes_results_and_drops_url_less_entries() {
    let (handle, base) = spawn_mock_searxng().await;
    let mut config = test_config(&base);
    config.provider = WebSearchProvider::SearXng;
    config.api_key = None;
    config.searxng_url = Some(base.clone());
    let results = web_search(&config, "query", 5).await.unwrap();
    handle.await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Sx");
    assert_eq!(results[0].url, "https://example.com/sx");
    assert_eq!(results[0].source, "example.com");
  }

  async fn spawn_mock_ollama() -> (tokio::task::JoinHandle<()>, String) {
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
            {"title": "O", "url": "https://example.com/o", "content": "from ollama"}
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
  async fn ollama_normalizes_results() {
    let (handle, base) = spawn_mock_ollama().await;
    let mut config = test_config(&base);
    config.provider = WebSearchProvider::Ollama;
    config.ollama_search_url = base.clone();
    let results = web_search(&config, "query", 5).await.unwrap();
    handle.await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "O");
    assert_eq!(results[0].url, "https://example.com/o");
    assert_eq!(results[0].source, "example.com");
  }
}
