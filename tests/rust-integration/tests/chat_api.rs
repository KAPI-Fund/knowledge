mod support;

use std::fs;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use support::mock_openai::{MockOpenAiServer, MockScenario};
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn conversation_crud_round_trip() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("chat-crud").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("chat-crud-project");
    let project_id =
        support::create_project_with_alias(state.clone(), &cookie, &csrf, project_root).await;

    // Missing CSRF is rejected.
    let missing_csrf = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/conversations"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "title": "x" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::UNAUTHORIZED);

    // Create.
    let created = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/conversations"))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "title": "Attention questions" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_payload = read_json(created.into_body()).await;
    let conversation_id = created_payload
        .get("id")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();

    // List.
    let listed = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/conversations"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_payload = read_json(listed.into_body()).await;
    assert_eq!(
        listed_payload
            .get("conversations")
            .and_then(Value::as_array)
            .unwrap()
            .len(),
        1
    );

    // Rename.
    let renamed = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/projects/{project_id}/conversations/{conversation_id}"
                ))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "title": "Renamed" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(renamed.status(), StatusCode::OK);
    let renamed_payload = read_json(renamed.into_body()).await;
    assert_eq!(
        renamed_payload.get("title").and_then(Value::as_str),
        Some("Renamed")
    );

    // Messages list starts empty.
    let messages = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/conversations/{conversation_id}/messages"
                ))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(messages.status(), StatusCode::OK);
    let messages_payload = read_json(messages.into_body()).await;
    assert!(
        messages_payload
            .get("messages")
            .and_then(Value::as_array)
            .unwrap()
            .is_empty()
    );

    // Delete.
    let deleted = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/projects/{project_id}/conversations/{conversation_id}"
                ))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::OK);

    let listed_after = build_app(state)
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/conversations"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let after_payload = read_json(listed_after.into_body()).await;
    assert!(
        after_payload
            .get("conversations")
            .and_then(Value::as_array)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn chat_message_streams_response_and_persists_assistant_message() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("chat-stream").await.unwrap();
    let mock = MockOpenAiServer::start(MockScenario::success())
        .await
        .unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("chat-stream-project");
    let project_id =
        support::create_project_with_alias(state.clone(), &cookie, &csrf, project_root.clone())
            .await;

    // Point provider at the mock; leave embedding model NULL so hybrid search
    // stays keyword-only (no embeddings round trip in this test).
    sqlx::query(
        "UPDATE system_settings
         SET provider_mode = 'openai-compatible',
             provider_base_url = $1,
             provider_api_key = 'test-key',
             provider_model = 'mock-model',
             provider_embedding_model = NULL
         WHERE id = 1",
    )
    .bind(mock.base_url())
    .execute(&state.pool)
    .await
    .unwrap();

    fs::write(
        project_root.join("wiki/concepts/attention.md"),
        "---\ntype: concept\ntitle: Attention\nsources: []\n---\n\n# Attention\n\nAttention focuses computation.\n",
    )
    .unwrap();

    // Create a conversation.
    let created = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/conversations"))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "title": "Chat" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let conversation_id = read_json(created.into_body())
        .await
        .get("id")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();

    // Send a message; the response is an SSE stream.
    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/projects/{project_id}/conversations/{conversation_id}/messages"
                ))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "content": "What is attention?" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap()
            .starts_with("text/event-stream")
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("event: delta"), "missing delta events: {text}");
    assert!(text.contains("event: done"), "missing done event: {text}");

    // Both turns are persisted.
    let messages = build_app(state)
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/conversations/{conversation_id}/messages"
                ))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let messages_payload = read_json(messages.into_body()).await;
    let items = messages_payload
        .get("messages")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].get("role").and_then(Value::as_str), Some("user"));
    assert_eq!(
        items[1].get("role").and_then(Value::as_str),
        Some("assistant")
    );
    assert_eq!(
        items[1].get("content").and_then(Value::as_str),
        Some("Attention focuses computation on relevant tokens.")
    );
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
