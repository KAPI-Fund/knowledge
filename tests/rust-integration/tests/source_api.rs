mod support;

use std::fs;
use std::path::Path;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::{scheduler, store};
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{json, Value};
use support::TestEnvironment;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn import_source_writes_file_lists_source_and_records_task() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("source-import").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_id = create_project(state.clone(), &cookie, &csrf, temp.path().join("wiki-import")).await;

  let app = build_app(state.clone());
  let import_response = app
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/sources:import"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "fileName": "note.md",
            "contentBase64": "IyBOb3RlCgpIZWxsbyBmcm9tIHNvdXJjZS4K"
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(import_response.status(), StatusCode::ACCEPTED);
  let import_payload = read_json(import_response.into_body()).await;
  wait_for_task_terminal(
    &state,
    import_payload.get("taskId").and_then(Value::as_str).unwrap(),
  )
  .await;

  let list_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/sources"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(list_response.status(), StatusCode::OK);
  let payload = read_json(list_response.into_body()).await;
  let sources = payload.get("sources").and_then(Value::as_array).unwrap();
  assert_eq!(sources.len(), 1);
  assert_eq!(sources[0].get("relativePath").and_then(Value::as_str), Some("raw/sources/note.md"));

  let project_root = temp.path().join("wiki-import");
  assert_eq!(
    fs::read_to_string(project_root.join("raw/sources/note.md")).unwrap(),
    "# Note\n\nHello from source.\n"
  );

  let queue_json = fs::read_to_string(project_root.join(".knowledge/ingest/queue.json")).unwrap();
  assert!(queue_json.contains("\"taskType\":\"source_import\""));
  assert!(queue_json.contains("\"relativePath\":\"raw/sources/note.md\""));
}

#[tokio::test]
async fn rescan_and_delete_source_update_catalog_and_task_log() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("source-rescan").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("wiki-rescan");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  fs::write(
    project_root.join("raw/sources/manual.md"),
    "# Manual\n\nAdded outside the API.\n",
  )
  .unwrap();

  let rescan_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/sources:rescan"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(rescan_response.status(), StatusCode::ACCEPTED);
  let rescan_payload = read_json(rescan_response.into_body()).await;
  wait_for_task_terminal(
    &state,
    rescan_payload.get("taskId").and_then(Value::as_str).unwrap(),
  )
  .await;

  let delete_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri(format!("/api/projects/{project_id}/sources/manual.md"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(delete_response.status(), StatusCode::ACCEPTED);
  let delete_payload = read_json(delete_response.into_body()).await;
  wait_for_task_terminal(
    &state,
    delete_payload.get("taskId").and_then(Value::as_str).unwrap(),
  )
  .await;
  assert!(!Path::new(&project_root.join("raw/sources/manual.md")).exists());

  let list_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/sources"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  let payload = read_json(list_response.into_body()).await;
  assert_eq!(
    payload
      .get("sources")
      .and_then(Value::as_array)
      .map(Vec::len),
    Some(0)
  );

  let queue_json = fs::read_to_string(project_root.join(".knowledge/ingest/queue.json")).unwrap();
  assert!(queue_json.contains("\"taskType\":\"source_rescan\""));
  assert!(queue_json.contains("\"taskType\":\"source_delete\""));
}

#[tokio::test]
async fn delete_source_cleans_single_source_pages_and_rewrites_multi_source_pages() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("source-delete-cleanup").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("wiki-delete-cleanup");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  fs::write(
    project_root.join("raw/sources/manual.md"),
    "# Manual\n\nPrimary source.\n",
  )
  .unwrap();
  fs::write(
    project_root.join("wiki/sources/manual.md"),
    "---\ntype: source\ntitle: Manual\nsources: [\"manual.md\"]\n---\n\n# Manual\n",
  )
  .unwrap();
  fs::write(
    project_root.join("wiki/concepts/manual-only.md"),
    "---\ntype: concept\ntitle: Manual Only\nsources: [\"manual.md\"]\n---\n\n# Manual Only\n",
  )
  .unwrap();
  fs::write(
    project_root.join("wiki/concepts/shared.md"),
    "---\ntype: concept\ntitle: Shared\nsources:\n  - manual.md\n  - other.md\n---\n\n# Shared\n",
  )
  .unwrap();
  fs::write(
    project_root.join("wiki/concepts/false-positive.md"),
    "---\ntitle: Analysis of manual.md\nsources: [\"other.md\"]\n---\n\n# False Positive\n",
  )
  .unwrap();

  let delete_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri(format!("/api/projects/{project_id}/sources/manual.md"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(delete_response.status(), StatusCode::ACCEPTED);
  let delete_payload = read_json(delete_response.into_body()).await;
  wait_for_task_terminal(
    &state,
    delete_payload.get("taskId").and_then(Value::as_str).unwrap(),
  )
  .await;

  assert!(!project_root.join("raw/sources/manual.md").exists());
  assert!(!project_root.join("wiki/sources/manual.md").exists());
  assert!(!project_root.join("wiki/concepts/manual-only.md").exists());
  assert!(project_root.join("wiki/concepts/shared.md").exists());
  assert!(project_root.join("wiki/concepts/false-positive.md").exists());

  let shared = fs::read_to_string(project_root.join("wiki/concepts/shared.md")).unwrap();
  assert!(shared.contains("sources: [\"other.md\"]"));
  assert!(!shared.contains("manual.md"));

  let false_positive =
    fs::read_to_string(project_root.join("wiki/concepts/false-positive.md")).unwrap();
  assert!(false_positive.contains("sources: [\"other.md\"]"));
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

  assert_eq!(response.status(), StatusCode::CREATED);
  let payload = read_json(response.into_body()).await;
  payload.get("id").and_then(Value::as_str).unwrap().to_string()
}

async fn read_json(body: Body) -> Value {
  let bytes = to_bytes(body, usize::MAX).await.unwrap();
  serde_json::from_slice(&bytes).unwrap()
}

async fn wait_for_task_terminal(state: &knowledge_server::app::state::AppState, task_id: &str) {
  for _ in 0..20 {
    let progressed = scheduler::run_scheduler_tick(state).await.unwrap_or(false);
    let task = store::get_task_by_id(state, task_id).await.unwrap();
    if matches!(task.status.as_str(), "succeeded" | "failed" | "cancelled") {
      return;
    }
    if !progressed {
      tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
  }

  panic!("task did not reach a terminal state");
}
