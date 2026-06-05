mod support;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{json, Value};
use support::TestEnvironment;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn system_settings_round_trip_provider_config() {
  let _env = TestEnvironment::start("system-settings").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;

  let update_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("PATCH")
        .uri("/api/system/settings")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "providerMode": "deterministic",
            "language": "en",
            "defaultQueryLimit": 3
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(update_response.status(), StatusCode::OK);

  let get_response = build_app(state)
    .oneshot(
      Request::builder()
        .uri("/api/system/settings")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(get_response.status(), StatusCode::OK);
  let payload = read_json(get_response.into_body()).await;
  assert_eq!(payload.get("providerMode").and_then(Value::as_str), Some("deterministic"));
  assert_eq!(payload.get("language").and_then(Value::as_str), Some("en"));
  assert_eq!(payload.get("defaultQueryLimit").and_then(Value::as_u64), Some(3));
}

#[tokio::test]
async fn ingest_creates_review_items_and_review_endpoint_can_update_status() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("review-items").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("review-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  let _ = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/sources:import"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "fileName": "open-question.md",
            "contentBase64": "IyBPcGVuIFF1ZXN0aW9uCgpXaGF0IGV2aWRlbmNlIGlzIHN0aWxsIG1pc3Npbmc/Cg=="
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  let _ = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/ingest"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "relativePath": "raw/sources/open-question.md" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  let reviews_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/reviews"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(reviews_response.status(), StatusCode::OK);
  let reviews_payload = read_json(reviews_response.into_body()).await;
  let reviews = reviews_payload.get("reviews").and_then(Value::as_array).unwrap();
  assert_eq!(reviews.len(), 1);
  let review_id = reviews[0].get("id").and_then(Value::as_str).unwrap().to_string();
  assert_eq!(reviews[0].get("status").and_then(Value::as_str), Some("open"));

  let update_response = build_app(state)
    .oneshot(
      Request::builder()
        .method("PATCH")
        .uri(format!("/api/projects/{project_id}/reviews/{review_id}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "status": "resolved" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(update_response.status(), StatusCode::OK);
  let update_payload = read_json(update_response.into_body()).await;
  assert_eq!(update_payload.get("status").and_then(Value::as_str), Some("resolved"));
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

async fn create_project(
  state: knowledge_server::app::state::AppState,
  cookie: &str,
  csrf: &str,
  project_root: std::path::PathBuf,
) -> String {
  let response = build_app(state)
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, cookie)
        .header("x-csrf-token", csrf)
        .body(Body::from(
          json!({
            "name": project_root.file_name().unwrap().to_string_lossy(),
            "rootPath": project_root.to_string_lossy()
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  let payload = read_json(response.into_body()).await;
  payload.get("id").and_then(Value::as_str).unwrap().to_string()
}

async fn read_json(body: Body) -> Value {
  let bytes = to_bytes(body, usize::MAX).await.unwrap();
  serde_json::from_slice(&bytes).unwrap()
}
