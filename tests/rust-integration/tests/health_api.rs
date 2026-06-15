mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::Value;
use support::TestEnvironment;
use tower::util::ServiceExt;

#[tokio::test]
async fn health_returns_json_status_body() {
    let _env = TestEnvironment::start("health-status").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        body.get("service").and_then(Value::as_str),
        Some("knowledge-server")
    );
}
