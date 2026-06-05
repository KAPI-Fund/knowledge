mod support;

use std::fs;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{json, Value};
use support::TestEnvironment;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn ingest_source_generates_summary_and_updates_indexes() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("ingest-source").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("ingest-project");
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
            "fileName": "attention.md",
            "contentBase64": "IyBBdHRlbnRpb24KClRyYW5zZm9ybWVycyB1c2UgYXR0ZW50aW9uIG1lY2hhbmlzbXMuCg=="
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  let ingest_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/ingest"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "relativePath": "raw/sources/attention.md" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(ingest_response.status(), StatusCode::OK);
  let payload = read_json(ingest_response.into_body()).await;
  assert_eq!(payload.get("summaryPath").and_then(Value::as_str), Some("wiki/sources/attention.md"));

  let summary = fs::read_to_string(project_root.join("wiki/sources/attention.md")).unwrap();
  assert!(summary.contains("type: source"));
  assert!(summary.contains("sources: [attention.md]"));
  assert!(summary.contains("# Attention"));

  let index = fs::read_to_string(project_root.join("wiki/index.md")).unwrap();
  assert!(index.contains("[[attention]]"));

  let log = fs::read_to_string(project_root.join("wiki/log.md")).unwrap();
  assert!(log.contains("ingest | Attention"));

  let overview = fs::read_to_string(project_root.join("wiki/overview.md")).unwrap();
  assert!(overview.contains("Attention"));

  let analysis_checkpoint =
    fs::read_to_string(project_root.join(".knowledge/ingest/checkpoints/attention.analysis.json"))
      .unwrap();
  assert!(analysis_checkpoint.contains("\"title\":\"Attention\""));

  let generation_checkpoint =
    fs::read_to_string(project_root.join(".knowledge/ingest/checkpoints/attention.generation.json"))
      .unwrap();
  assert!(generation_checkpoint.contains("\"summaryPath\":\"wiki/sources/attention.md\""));
}

#[tokio::test]
async fn query_api_returns_answer_and_citations_from_search_context() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("query-answer").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("query-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  fs::write(
    project_root.join("wiki/concepts/attention.md"),
    "---\ntype: concept\ntitle: Attention\nsources: [attention.md]\n---\n\n# Attention\n\nAttention lets models focus on relevant tokens.\n",
  )
  .unwrap();

  let response = build_app(state)
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .body(Body::from(json!({ "query": "What is attention?" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let payload = read_json(response.into_body()).await;
  assert!(payload.get("answer").and_then(Value::as_str).unwrap().contains("Attention"));
  assert!(
    payload
      .get("contextSummary")
      .and_then(Value::as_str)
      .unwrap()
      .contains("wiki/concepts/attention.md")
  );
  let citations = payload.get("citations").and_then(Value::as_array).unwrap();
  assert_eq!(citations.len(), 1);
  assert_eq!(
    citations[0].get("path").and_then(Value::as_str),
    Some("wiki/concepts/attention.md")
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
