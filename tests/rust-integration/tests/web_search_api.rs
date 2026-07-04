mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tower::util::ServiceExt;

#[tokio::test]
async fn web_search_against_mock_searxng_returns_results() {
    let _env = TestEnvironment::start("web-search-flow").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let (searxng_handle, searxng_base) = spawn_mock_searxng().await;

    let patch = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "search": {
                        "provider": "searxng",
                        "providers": { "searxng": { "url": searxng_base, "categories": ["general"] } }
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web-search")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "query": "knowledge graphs", "maxResults": 3 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let payload = read_json(response.into_body()).await;
    let results = payload.get("results").and_then(Value::as_array).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["title"].as_str(), Some("Sx"));
    assert_eq!(results[0]["url"].as_str(), Some("https://example.com/sx"));

    searxng_handle.await.unwrap();
}

async fn spawn_mock_tavily_validating() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0u8; 8192];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let raw = String::from_utf8_lossy(&buf[..n]);
            let request_line = raw.lines().next().unwrap_or("");
            assert!(
                request_line.starts_with("POST /search "),
                "tavily: expected POST /search, got: {request_line}"
            );
            assert!(raw.contains("\"api_key\":\"test-key\""), "tavily: api_key value not locked: {raw}");
            assert!(raw.contains("\"query\":\"tavily query\""), "tavily: query value not locked: {raw}");
            assert!(raw.contains("\"search_depth\":\"advanced\""), "tavily: search_depth must be advanced: {raw}");
            assert!(raw.contains("\"max_results\":3"), "tavily: max_results must be 3: {raw}");
            let body = json!({
              "results": [
                { "title": "Tav", "url": "https://example.com/tav", "content": "from tavily" }
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

async fn spawn_mock_serpapi_validating() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0u8; 8192];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let raw = String::from_utf8_lossy(&buf[..n]);
            let request_line = raw.lines().next().unwrap_or("");
            assert!(
                request_line.starts_with("GET /search?"),
                "serpapi: expected GET /search?..., got: {request_line}"
            );
            assert!(request_line.contains("engine=google"), "serpapi: engine value not locked: {request_line}");
            assert!(request_line.contains("q=serpapi-query"), "serpapi: query value not locked: {request_line}");
            assert!(request_line.contains("api_key=test-key"), "serpapi: api_key value not locked: {request_line}");
            assert!(request_line.contains("num=3"), "serpapi: num must be 3: {request_line}");
            let body = json!({
              "organic_results": [
                { "title": "Serp", "link": "https://example.com/serp", "snippet": "from serpapi" }
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

async fn spawn_mock_ollama_validating() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0u8; 8192];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let raw = String::from_utf8_lossy(&buf[..n]);
            let request_line = raw.lines().next().unwrap_or("");
            assert!(
                request_line.starts_with("POST /api/web_search "),
                "ollama: expected POST /api/web_search, got: {request_line}"
            );
            assert!(
                raw.to_lowercase().contains("authorization: bearer test-key"),
                "ollama: Bearer token value not locked: {raw}"
            );
            assert!(raw.contains("\"query\":\"ollama query\""), "ollama: query value not locked: {raw}");
            assert!(raw.contains("\"max_results\":3"), "ollama: max_results must be 3: {raw}");
            let body = json!({
              "results": [
                { "title": "Olla", "url": "https://example.com/olla", "content": "from ollama" }
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

async fn spawn_mock_searxng() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let body = json!({
              "results": [
                {"title": "Sx", "url": "https://example.com/sx", "content": "from searxng", "engine": "duckduckgo"}
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

async fn login_and_csrf(state: knowledge_server::app::state::AppState) -> (String, String) {
    let login = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                      "username": "admin",
                      "password": "secret-password"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let body = read_json(login.into_body()).await;
    let csrf = body.get("csrfToken").and_then(Value::as_str).unwrap().to_string();
    (cookie, csrf)
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn patch_settings_response_reflects_db_state() {
    let _env = TestEnvironment::start("settings-response-truth").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let initial = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "embedding": {
                        "enabled": true,
                        "baseUrl": "https://emb.local/v1",
                        "model": "text-embedding-3-small",
                        "apiKey": "first-secret"
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial.status(), StatusCode::OK);
    let body = read_json(initial.into_body()).await;
    assert_eq!(body["embedding"]["apiKeyConfigured"], json!(true));

    let preserve = build_app(state)
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "embedding": { "model": "text-embedding-3-large" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preserve.status(), StatusCode::OK);
    let body = read_json(preserve.into_body()).await;
    assert_eq!(
        body["embedding"]["apiKeyConfigured"],
        json!(true),
        "apiKeyConfigured must reflect DB state, not request payload"
    );
}

#[tokio::test]
async fn web_search_via_bearer_token() {
    let _env = TestEnvironment::start("web-search-bearer").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let (searxng_handle, searxng_base) = spawn_mock_searxng().await;

    let patch = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "search": {
                        "provider": "searxng",
                        "providers": { "searxng": { "url": searxng_base, "categories": ["general"] } }
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);

    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(json!({ "name": "web-search-bearer" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint.status(), StatusCode::CREATED);
    let token = read_json(mint.into_body()).await["token"].as_str().unwrap().to_string();

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web-search")
                .header(header::CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({ "query": "knowledge graphs", "maxResults": 3 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response.into_body()).await;
    let results = payload.get("results").and_then(Value::as_array).unwrap();
    assert_eq!(results.len(), 1);

    searxng_handle.abort();
}

#[tokio::test]
async fn settings_get_rejects_bearer_token() {
    let _env = TestEnvironment::start("settings-bearer-get").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(json!({ "name": "settings-test" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let token = read_json(mint.into_body()).await["token"]
        .as_str()
        .unwrap()
        .to_string();

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .uri("/api/system/settings")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "system settings must reject Bearer authentication"
    );
}

#[tokio::test]
async fn settings_patch_rejects_bearer_token() {
    let _env = TestEnvironment::start("settings-bearer-patch").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(json!({ "name": "settings-test" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let token = read_json(mint.into_body()).await["token"]
        .as_str()
        .unwrap()
        .to_string();

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({ "search": { "provider": "tavily" } }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "system settings must reject Bearer authentication"
    );
}

#[tokio::test]
async fn web_search_tavily_uses_persisted_base_url() {
    let _env = TestEnvironment::start("web-search-tavily").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let (handle, base) = spawn_mock_tavily_validating().await;

    let patch = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "search": {
                        "provider": "tavily",
                        "providers": { "tavily": { "apiKey": "test-key", "baseUrl": base } }
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web-search")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "query": "tavily query", "maxResults": 3 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response.into_body()).await;
    let results = payload.get("results").and_then(Value::as_array).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["title"].as_str(), Some("Tav"));
    assert_eq!(results[0]["url"].as_str(), Some("https://example.com/tav"));

    handle.abort();
}

#[tokio::test]
async fn web_search_serpapi_uses_persisted_base_url() {
    let _env = TestEnvironment::start("web-search-serpapi").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let (handle, base) = spawn_mock_serpapi_validating().await;

    let patch = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "search": {
                        "provider": "serpapi",
                        "providers": { "serpapi": { "apiKey": "test-key", "engine": "google", "baseUrl": base } }
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web-search")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "query": "serpapi-query", "maxResults": 3 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response.into_body()).await;
    let results = payload.get("results").and_then(Value::as_array).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["title"].as_str(), Some("Serp"));
    assert_eq!(results[0]["url"].as_str(), Some("https://example.com/serp"));

    handle.abort();
}

#[tokio::test]
async fn web_search_ollama_uses_persisted_url() {
    let _env = TestEnvironment::start("web-search-ollama").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let (handle, base) = spawn_mock_ollama_validating().await;

    let patch = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "search": {
                        "provider": "ollama",
                        "providers": { "ollama": { "apiKey": "test-key", "url": base } }
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web-search")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "query": "ollama query", "maxResults": 3 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response.into_body()).await;
    let results = payload.get("results").and_then(Value::as_array).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["title"].as_str(), Some("Olla"));
    assert_eq!(results[0]["url"].as_str(), Some("https://example.com/olla"));

    handle.abort();
}

#[tokio::test]
async fn patch_settings_search_api_key_configured_reflects_db_state() {
    let _env = TestEnvironment::start("settings-search-api-key-truth").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let initial = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "search": {
                        "provider": "tavily",
                        "providers": { "tavily": { "apiKey": "first-search-secret" } }
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial.status(), StatusCode::OK);
    let body = read_json(initial.into_body()).await;
    assert_eq!(body["search"]["providers"]["tavily"]["apiKeyConfigured"], json!(true));

    let preserve = build_app(state)
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "search": { "provider": "tavily" } }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preserve.status(), StatusCode::OK);
    let body = read_json(preserve.into_body()).await;
    assert_eq!(
        body["search"]["providers"]["tavily"]["apiKeyConfigured"],
        json!(true),
        "apiKeyConfigured must reflect DB state, not request payload"
    );
}

#[tokio::test]
async fn patch_settings_can_clear_provider_api_key() {
    let _env = TestEnvironment::start("settings-key-clear").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let set_key = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "embedding": {
                        "enabled": true,
                        "baseUrl": "https://emb.local/v1",
                        "model": "text-embedding-3-small",
                        "apiKey": "my-secret"
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(set_key.status(), StatusCode::OK);
    assert_eq!(
        read_json(set_key.into_body()).await["embedding"]["apiKeyConfigured"],
        json!(true)
    );

    let clear_key = build_app(state)
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "embedding": {
                        "clearApiKey": true
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clear_key.status(), StatusCode::OK);
    assert_eq!(
        read_json(clear_key.into_body()).await["embedding"]["apiKeyConfigured"],
        json!(false),
        "embedding.clearApiKey:true must set embedding.apiKeyConfigured to false"
    );
}

#[tokio::test]
async fn patch_settings_replacement_key_wins_over_clear() {
    let _env = TestEnvironment::start("settings-replace-beats-clear").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let set_key = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "embedding": {
                        "enabled": true,
                        "baseUrl": "https://emb.local/v1",
                        "model": "text-embedding-3-small",
                        "apiKey": "old-secret"
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(set_key.status(), StatusCode::OK);

    let replace = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "embedding": {
                        "apiKey": "new-secret",
                        "clearApiKey": true
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replace.status(), StatusCode::OK);
    assert_eq!(
        read_json(replace.into_body()).await["embedding"]["apiKeyConfigured"],
        json!(true),
        "a supplied replacement key must take precedence over clearApiKey"
    );

    let stored: Option<String> =
        sqlx::query_scalar("SELECT embedding_api_key FROM system_settings WHERE id = 1")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(stored.as_deref(), Some("new-secret"));
}

#[tokio::test]
async fn patch_settings_can_clear_search_api_key() {
    let _env = TestEnvironment::start("settings-search-key-clear").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let set_key = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "search": {
                        "provider": "tavily",
                        "providers": { "tavily": { "apiKey": "my-search-secret" } }
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(set_key.status(), StatusCode::OK);
    assert_eq!(
        read_json(set_key.into_body()).await["search"]["providers"]["tavily"]["apiKeyConfigured"],
        json!(true)
    );

    let clear_key = build_app(state)
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "search": {
                        "provider": "tavily",
                        "providers": { "tavily": { "apiKey": null } }
                      }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clear_key.status(), StatusCode::OK);
    assert_eq!(
        read_json(clear_key.into_body()).await["search"]["providers"]["tavily"]["apiKeyConfigured"],
        json!(false),
        "explicit null apiKey must set search.providers.tavily.apiKeyConfigured to false"
    );
}
