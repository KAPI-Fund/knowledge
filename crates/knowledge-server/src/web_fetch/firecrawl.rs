use std::time::Duration;

use serde::Deserialize;

use crate::http::error::ApiError;
use crate::web_fetch::config::FetchConfig;
use crate::web_fetch::ExtractedPage;

#[derive(Debug, Deserialize)]
struct FirecrawlResponse {
    success: Option<bool>,
    data: Option<FirecrawlData>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FirecrawlData {
    markdown: Option<String>,
    metadata: Option<FirecrawlMetadata>,
}

#[derive(Debug, Deserialize)]
struct FirecrawlMetadata {
    title: Option<String>,
}

/// Scrape a URL via Firecrawl's /v1/scrape endpoint and return readable markdown.
/// POST {base_url}/v1/scrape with { url, formats:["markdown"], onlyMainContent:true },
/// Bearer auth when an api_key is present. Renders JS, so a 60s timeout.
pub async fn scrape(config: &FetchConfig, url: &str) -> Result<ExtractedPage, ApiError> {
    let endpoint = format!("{}/v1/scrape", config.base_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| ApiError::internal(format!("failed to build fetch client: {err}")))?;

    let mut request = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "url": url,
            "formats": ["markdown"],
            "onlyMainContent": true,
        }));

    if let Some(key) = config.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
        request = request.header("Authorization", format!("Bearer {key}"));
    }

    let response = request
        .send()
        .await
        .map_err(|err| ApiError::bad_request(format!("firecrawl request failed: {err}")))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| ApiError::bad_request(format!("firecrawl response read failed: {err}")))?;

    if !status.is_success() {
        return Err(ApiError::bad_request(format!(
            "firecrawl returned {status}: {body}"
        )));
    }

    let parsed: FirecrawlResponse = serde_json::from_str(&body).map_err(|err| {
        ApiError::bad_request(format!("firecrawl response was not valid JSON: {err}; body: {body}"))
    })?;

    if parsed.success == Some(false) {
        let message = parsed.error.unwrap_or_else(|| "firecrawl reported failure".to_string());
        return Err(ApiError::bad_request(format!("firecrawl error: {message}")));
    }

    let data = parsed
        .data
        .ok_or_else(|| ApiError::bad_request("firecrawl response had no data".to_string()))?;

    let title = data
        .metadata
        .and_then(|m| m.title)
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Untitled".to_string());

    Ok(ExtractedPage {
        title,
        markdown: data.markdown.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_fetch::FetchProvider;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Spawn a one-shot TCP server that returns `body` as an HTTP response with the
    /// given status line, and return the base_url pointing at it. Mirrors the
    /// spawn_mock_tavily pattern in web_search/provider.rs.
    async fn spawn_mock(status_line: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            // Drain the request (best-effort; we don't parse it).
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let response = format!(
                "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        });
        format!("http://{addr}")
    }

    fn config_for(base_url: String) -> FetchConfig {
        FetchConfig {
            provider: FetchProvider::Firecrawl,
            base_url,
            api_key: Some("sk-test".to_string()),
        }
    }

    #[tokio::test]
    async fn scrape_maps_success_to_extracted_page() {
        let base = spawn_mock(
            "200 OK",
            r##"{"success":true,"data":{"markdown":"# Hello","metadata":{"title":"Example"}}}"##,
        )
        .await;
        let page = scrape(&config_for(base), "https://example.com").await.unwrap();
        assert_eq!(page.title, "Example");
        assert_eq!(page.markdown, "# Hello");
    }

    #[tokio::test]
    async fn scrape_defaults_title_when_absent() {
        let base = spawn_mock(
            "200 OK",
            r#"{"success":true,"data":{"markdown":"body only"}}"#,
        )
        .await;
        let page = scrape(&config_for(base), "https://example.com").await.unwrap();
        assert_eq!(page.title, "Untitled");
        assert_eq!(page.markdown, "body only");
    }

    #[tokio::test]
    async fn scrape_maps_success_false_to_bad_request() {
        let base = spawn_mock(
            "200 OK",
            r#"{"success":false,"error":"could not reach target"}"#,
        )
        .await;
        let err = scrape(&config_for(base), "https://example.com").await.unwrap_err();
        assert!(err.to_string().contains("could not reach target"));
    }

    #[tokio::test]
    async fn scrape_maps_non_2xx_to_bad_request() {
        let base = spawn_mock("500 Internal Server Error", r#"{"error":"boom"}"#).await;
        let err = scrape(&config_for(base), "https://example.com").await.unwrap_err();
        assert!(err.to_string().contains("500"));
    }
}
