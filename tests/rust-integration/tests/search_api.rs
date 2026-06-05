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
async fn search_returns_matching_wiki_pages_and_snippets() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("search-query").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("search-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  fs::write(
    project_root.join("wiki/concepts/chain-of-thought.md"),
    "---\ntype: concept\ntitle: Chain of Thought\nsources: []\n---\n\n# Chain of Thought\n\nReasoning traces improve stepwise problem solving.\n",
  )
  .unwrap();

  let response = build_app(state)
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/search"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .body(Body::from(json!({ "query": "reasoning traces" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let payload = read_json(response.into_body()).await;
  let results = payload.get("results").and_then(Value::as_array).unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(
    results[0].get("path").and_then(Value::as_str),
    Some("wiki/concepts/chain-of-thought.md")
  );
  assert!(
    results[0]
      .get("snippet")
      .and_then(Value::as_str)
      .unwrap()
      .contains("Reasoning traces")
  );
}

#[tokio::test]
async fn graph_returns_nodes_and_edges_from_wikilinks() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("graph-query").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("graph-project");
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

  let response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/graph"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let payload = read_json(response.into_body()).await;
  assert_eq!(payload.get("nodes").and_then(Value::as_array).map(Vec::len), Some(2));
  assert_eq!(payload.get("edges").and_then(Value::as_array).map(Vec::len), Some(1));
}

#[tokio::test]
async fn task_endpoint_returns_source_task_queue() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("task-queue").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("tasks-project");
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
            "fileName": "task-note.md",
            "contentBase64": "IyBUYXNrIE5vdGUK"
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

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
  assert_eq!(tasks.len(), 1);
  assert_eq!(tasks[0].get("taskType").and_then(Value::as_str), Some("source_import"));
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
