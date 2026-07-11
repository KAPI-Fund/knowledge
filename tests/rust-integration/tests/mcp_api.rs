mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn mcp_initialize_and_tool_listing() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("mcp-initialize").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("mcp-project");
    let _project_id =
        support::create_project_with_alias(state.clone(), &cookie, &csrf, project_root).await;
    let token = mint_token(state.clone(), &cookie, &csrf).await;

    // initialize needs no auth and echoes a supported protocol version.
    let response = post_mcp(
        state.clone(),
        None,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "protocolVersion": "2025-06-18" },
        }),
    )
    .await;
    assert_eq!(response.0, StatusCode::OK);
    assert_eq!(response.1["result"]["protocolVersion"], "2025-06-18");
    assert!(response.1["result"]["capabilities"]["tools"].is_object());

    // tools/list requires a bearer token.
    let denied = post_mcp(
        state.clone(),
        None,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    )
    .await;
    assert_eq!(denied.0, StatusCode::UNAUTHORIZED);

    let listed = post_mcp(
        state.clone(),
        Some(&token),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/list" }),
    )
    .await;
    assert_eq!(listed.0, StatusCode::OK);
    let names = listed.1["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 9);
    assert!(names.contains(&"knowledge_status".to_string()));
    assert!(names.contains(&"knowledge_chat".to_string()));

    // GET is not part of our stateless transport.
    let get = build_app(state.clone())
        .oneshot(Request::builder().uri("/api/mcp").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::METHOD_NOT_ALLOWED);

    // Batch requests are rejected.
    let batch = post_mcp(state.clone(), Some(&token), json!([])).await;
    assert_eq!(batch.1["error"]["code"], -32600);
}

#[tokio::test]
async fn mcp_enabled_gate_and_tool_calls() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("mcp-gate").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("mcp-gate-project");
    let project_id =
        support::create_project_with_alias(state.clone(), &cookie, &csrf, project_root).await;
    let token = mint_token(state.clone(), &cookie, &csrf).await;

    // Disabled by default: tools fail with isError, status stays reachable.
    let search = call_tool(
        state.clone(),
        &token,
        "knowledge_search",
        json!({ "project_id": project_id, "query": "anything" }),
    )
    .await;
    assert_eq!(search["result"]["isError"], true);
    assert!(
        search["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("disabled")
    );

    let status = call_tool(state.clone(), &token, "knowledge_status", json!({})).await;
    assert!(status["result"]["isError"].is_null());
    let status_text = status["result"]["content"][0]["text"].as_str().unwrap();
    assert!(status_text.contains("\"mcpEnabled\": false"));

    // Enable via the settings PATCH.
    let patch = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(json!({ "mcp": { "enabled": true } }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);

    let health = build_app(state.clone())
        .oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let health_body = read_json(health.into_body()).await;
    assert_eq!(health_body["mcpEnabled"], true);

    let projects = call_tool(state.clone(), &token, "knowledge_projects", json!({})).await;
    assert!(projects["result"]["isError"].is_null());
    assert!(
        projects["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains(&project_id)
    );

    let files = call_tool(
        state.clone(),
        &token,
        "knowledge_files",
        json!({ "project_id": project_id }),
    )
    .await;
    assert!(files["result"]["isError"].is_null());
    assert!(files["result"]["content"][0]["text"].as_str().is_some());

    // Unknown tool -> invalid params error.
    let unknown = call_tool(state.clone(), &token, "knowledge_nope", json!({})).await;
    assert_eq!(unknown["error"]["code"], -32602);
}

async fn post_mcp(
    state: knowledge_server::app::state::AppState,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/mcp")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = build_app(state)
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let payload = if status == StatusCode::ACCEPTED {
        Value::Null
    } else {
        read_json(response.into_body()).await
    };
    (status, payload)
}

async fn call_tool(
    state: knowledge_server::app::state::AppState,
    token: &str,
    name: &str,
    arguments: Value,
) -> Value {
    let (status, payload) = post_mcp(
        state,
        Some(token),
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments },
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    payload
}

async fn mint_token(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
) -> String {
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .body(Body::from(json!({ "name": "mcp", "projectId": null }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    read_json(response.into_body()).await["token"]
        .as_str()
        .unwrap()
        .to_string()
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
    let csrf = body
        .get("csrfToken")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    (cookie, csrf)
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
