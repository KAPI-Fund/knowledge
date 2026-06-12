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
