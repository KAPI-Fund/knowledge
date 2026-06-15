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
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25,
                      "searchProvider": "searxng",
                      "searxngUrl": searxng_base,
                      "searxngCategories": ["general"]
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
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25,
                      "providerApiKey": "first-secret"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial.status(), StatusCode::OK);
    let body = read_json(initial.into_body()).await;
    assert_eq!(body["providerApiKeyConfigured"], json!(true));

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
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25
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
        body["providerApiKeyConfigured"],
        json!(true),
        "providerApiKeyConfigured must reflect DB state, not request payload"
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
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25,
                      "searchProvider": "searxng",
                      "searxngUrl": searxng_base,
                      "searxngCategories": ["general"]
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
                    json!({
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25,
                      "searchProvider": "tavily",
                      "searchApiKey": "stolen-key"
                    })
                    .to_string(),
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
