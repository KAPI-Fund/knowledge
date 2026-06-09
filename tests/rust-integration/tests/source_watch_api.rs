mod support;

use std::fs;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::projects::tasks::list_tasks;
use knowledge_server::{build_app, tasks};
use serde_json::{json, Value};
use support::{bootstrap_state_without_scheduler, TestEnvironment};
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn source_watch_scan_imports_changed_files_and_enqueues_ingest_tasks() {
  let env = TestEnvironment::start("source-watch").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state_without_scheduler(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;

  let project_root = tempdir().unwrap().path().join("source-watch-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  let watch_root = tempdir().unwrap().path().join("watched-sources");
  fs::create_dir_all(watch_root.join("team-a/docs")).unwrap();
  fs::write(watch_root.join("attention.md"), "# Attention\n\nRelevant tokens.\n").unwrap();
  fs::write(
    watch_root.join("team-a/child.md"),
    "# Child\n\nNested source identity.\n",
  )
  .unwrap();
  fs::write(
    watch_root.join("team-a/docs/peer.md"),
    "# Peer\n\nNested docs identity.\n",
  )
  .unwrap();
  fs::write(watch_root.join(".hidden.md"), "# Hidden\n\nShould be ignored.\n").unwrap();

  let update = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("PATCH")
        .uri(format!("/api/projects/{project_id}/source-watch"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "enabled": true,
            "path": watch_root.to_string_lossy(),
            "autoIngest": true,
            "intervalMinutes": 1,
            "includeExtensions": ["md"],
            "excludeExtensions": [],
            "excludeDirs": [],
            "excludeGlobs": [],
            "maxFileSizeMb": 100
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(update.status(), StatusCode::OK);

  let get = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri(format!("/api/projects/{project_id}/source-watch"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(get.status(), StatusCode::OK);
  let config_payload = read_json(get.into_body()).await;
  assert_eq!(config_payload.get("enabled").and_then(Value::as_bool), Some(true));
  assert_eq!(
    config_payload.get("path").and_then(Value::as_str),
    Some(watch_root.to_string_lossy().as_ref())
  );
  assert_eq!(
    config_payload.get("autoIngest").and_then(Value::as_bool),
    Some(true)
  );

  knowledge_server::projects::source_watch::scan_project_source_watch(&state, &project_id)
    .await
    .unwrap();

  for _ in 0..10 {
    if !tasks::scheduler::run_scheduler_tick(&state).await.unwrap_or(false) {
      break;
    }
    tokio::time::sleep(Duration::from_millis(25)).await;
  }

  assert!(project_root.join("raw/sources/attention.md").is_file());
  assert!(project_root.join("raw/sources/team-a/child.md").is_file());
  assert!(project_root.join("raw/sources/team-a/docs/peer.md").is_file());
  assert!(!project_root.join("raw/sources/.hidden.md").exists());

  let tasks = list_tasks(&state, &project_id).await.unwrap();
  assert!(tasks.iter().any(|task| {
    task.task_type == "project.ingest_source"
      && task.relative_path.as_deref() == Some("raw/sources/team-a/child.md")
  }));
  assert!(tasks.iter().any(|task| {
    task.task_type == "project.ingest_source"
      && task.relative_path.as_deref() == Some("raw/sources/team-a/docs/peer.md")
  }));
}

#[tokio::test]
async fn source_watch_scan_copies_files_without_ingest_when_auto_ingest_is_disabled() {
  let env = TestEnvironment::start("source-watch-no-auto-ingest").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state_without_scheduler(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;

  let project_root = tempdir().unwrap().path().join("source-watch-no-auto-ingest-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  let watch_root = tempdir().unwrap().path().join("watched-sources-no-auto-ingest");
  fs::create_dir_all(&watch_root).unwrap();
  fs::write(watch_root.join("attention.md"), "# Attention\n\nRelevant tokens.\n").unwrap();

  let update = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("PATCH")
        .uri(format!("/api/projects/{project_id}/source-watch"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "enabled": true,
            "path": watch_root.to_string_lossy(),
            "autoIngest": false,
            "intervalMinutes": 1,
            "includeExtensions": ["md"],
            "excludeExtensions": [],
            "excludeDirs": [],
            "excludeGlobs": [],
            "maxFileSizeMb": 100
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(update.status(), StatusCode::OK);

  knowledge_server::projects::source_watch::scan_project_source_watch(&state, &project_id)
    .await
    .unwrap();

  assert!(project_root.join("raw/sources/attention.md").is_file());
  let tasks = list_tasks(&state, &project_id).await.unwrap();
  assert!(!tasks.iter().any(|task| task.task_type == "project.ingest_source"));
}

#[tokio::test]
async fn source_watch_tick_scans_due_projects_based_on_interval() {
  let env = TestEnvironment::start("source-watch-tick").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state_without_scheduler(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;

  let project_root = tempdir().unwrap().path().join("source-watch-tick-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  let watch_root = tempdir().unwrap().path().join("watched-sources-tick");
  fs::create_dir_all(&watch_root).unwrap();
  fs::write(watch_root.join("attention.md"), "# Attention\n\nRelevant tokens.\n").unwrap();

  let update = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("PATCH")
        .uri(format!("/api/projects/{project_id}/source-watch"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "enabled": true,
            "path": watch_root.to_string_lossy(),
            "autoIngest": true,
            "intervalMinutes": 1,
            "includeExtensions": ["md"],
            "excludeExtensions": [],
            "excludeDirs": [],
            "excludeGlobs": [],
            "maxFileSizeMb": 100
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(update.status(), StatusCode::OK);

  sqlx::query(
    "UPDATE project_watch_settings
     SET last_scan_at = $2
     WHERE project_id = $1",
  )
  .bind(&project_id)
  .bind("1970-01-01T00:00:00Z")
  .execute(&state.pool)
  .await
  .unwrap();

  let scanned = knowledge_server::projects::source_watch::run_source_watch_tick(&state)
    .await
    .unwrap();
  assert_eq!(scanned, 1);
  assert!(project_root.join("raw/sources/attention.md").is_file());
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
