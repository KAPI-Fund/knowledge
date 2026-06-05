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
async fn import_and_ingest_create_persisted_task_summaries() {
  let env = TestEnvironment::start("task-summary").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("task-summary-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

  import_source_via_api(
    state.clone(),
    &cookie,
    &csrf,
    &project_id,
    "task.md",
    "IyBUYXNrCg==",
  )
  .await;
  ingest_source_via_api(state.clone(), &cookie, &csrf, &project_id, "raw/sources/task.md").await;

  let response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/tasks"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let payload = read_json(response.into_body()).await;
  let tasks = payload.get("tasks").and_then(Value::as_array).unwrap();
  assert_eq!(tasks.len(), 2);
  assert_eq!(tasks[0].get("status").and_then(Value::as_str), Some("completed"));
  assert!(tasks[0].get("id").and_then(Value::as_str).is_some());
}

#[tokio::test]
async fn task_detail_and_retry_endpoints_return_operational_state() {
  let env = TestEnvironment::start("task-detail").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("task-detail-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

  import_source_via_api(
    state.clone(),
    &cookie,
    &csrf,
    &project_id,
    "detail.md",
    "IyBEZXRhaWwK",
  )
  .await;

  let list_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/tasks"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  let list_payload = read_json(list_response.into_body()).await;
  let task_id = list_payload
    .get("tasks")
    .and_then(Value::as_array)
    .and_then(|tasks| tasks.first())
    .and_then(|task| task.get("id"))
    .and_then(Value::as_str)
    .unwrap()
    .to_string();

  let detail_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/tasks/{task_id}"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(detail_response.status(), StatusCode::OK);
  let detail_payload = read_json(detail_response.into_body()).await;
  assert_eq!(detail_payload.get("status").and_then(Value::as_str), Some("completed"));

  let retry_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/tasks/{task_id}/retry"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(retry_response.status(), StatusCode::OK);
  let retry_payload = read_json(retry_response.into_body()).await;
  assert_eq!(retry_payload.get("status").and_then(Value::as_str), Some("queued"));
}

#[tokio::test]
async fn project_operations_are_recorded_in_audit_log() {
  let env = TestEnvironment::start("audit-log").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("audit-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

  import_source_via_api(
    state.clone(),
    &cookie,
    &csrf,
    &project_id,
    "audit.md",
    "IyBBdWRpdAo=",
  )
  .await;

  let response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/audit-logs"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let payload = read_json(response.into_body()).await;
  let items = payload.get("items").and_then(Value::as_array).unwrap();
  assert!(items.iter().any(|item| item.get("action").and_then(Value::as_str) == Some("project.created")));
  assert!(items.iter().any(|item| item.get("action").and_then(Value::as_str) == Some("source.imported")));
}

#[tokio::test]
async fn graph_neighbors_and_project_detail_return_operational_view() {
  let env = TestEnvironment::start("graph-neighbors").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("graph-neighbors-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  fs::write(
    project_root.join("wiki/concepts/chain-of-thought.md"),
    "---\ntype: concept\ntitle: Chain of Thought\nsources: []\n---\n\nSee also [[reasoning-models]].\n",
  )
  .unwrap();
  fs::write(
    project_root.join("wiki/entities/reasoning-models.md"),
    "---\ntype: entity\ntitle: Reasoning Models\nsources: []\n---\n\n# Reasoning Models\n",
  )
  .unwrap();

  let neighbors_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/graph/chain-of-thought/neighbors"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(neighbors_response.status(), StatusCode::OK);
  let neighbors_payload = read_json(neighbors_response.into_body()).await;
  assert_eq!(
    neighbors_payload.get("node").and_then(|node| node.get("id")).and_then(Value::as_str),
    Some("chain-of-thought")
  );
  assert_eq!(
    neighbors_payload.get("neighbors").and_then(Value::as_array).map(Vec::len),
    Some(1)
  );

  let project_response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(project_response.status(), StatusCode::OK);
  let project_payload = read_json(project_response.into_body()).await;
  assert_eq!(
    project_payload
      .get("project")
      .and_then(|project| project.get("id"))
      .and_then(Value::as_str),
    Some(project_id.as_str())
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

async fn import_source_via_api(
  state: knowledge_server::app::state::AppState,
  cookie: &str,
  csrf: &str,
  project_id: &str,
  file_name: &str,
  content_base64: &str,
) {
  let response = build_app(state)
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/sources:import"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, cookie)
        .header("x-csrf-token", csrf)
        .body(Body::from(
          json!({
            "fileName": file_name,
            "contentBase64": content_base64
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::CREATED);
}

async fn ingest_source_via_api(
  state: knowledge_server::app::state::AppState,
  cookie: &str,
  csrf: &str,
  project_id: &str,
  relative_path: &str,
) {
  let response = build_app(state)
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/ingest"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, cookie)
        .header("x-csrf-token", csrf)
        .body(Body::from(json!({ "relativePath": relative_path }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
}

async fn read_json(body: Body) -> Value {
  let bytes = to_bytes(body, usize::MAX).await.unwrap();
  serde_json::from_slice(&bytes).unwrap()
}
