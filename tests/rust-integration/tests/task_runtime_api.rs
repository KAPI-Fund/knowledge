mod support;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::store::{self, CreateTaskInput};
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{json, Value};
use support::TestEnvironment;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn created_tasks_start_with_runtime_fields() {
  let env = TestEnvironment::start("runtime-schema").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("runtime-schema-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query-tasks"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "query": "attention",
            "topK": 3
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::ACCEPTED);
  let payload = read_json(response.into_body()).await;
  let task_id = payload.get("taskId").and_then(Value::as_str).unwrap();
  let detail = get_task_detail(state, &cookie, &project_id, task_id).await;
  assert_eq!(detail.get("status").and_then(Value::as_str), Some("queued"));
  assert_eq!(detail.get("attemptCount").and_then(Value::as_i64), Some(0));
  assert_eq!(detail.get("maxAttempts").and_then(Value::as_i64), Some(3));
}

#[tokio::test]
async fn scheduler_can_acquire_and_complete_a_queued_task() {
  let env = TestEnvironment::start("lease-store").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("lease-store-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;
  let admin_user_id = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
    .bind("admin")
    .fetch_one(&state.pool)
    .await
    .unwrap();
  let task = store::create_task(
    &state,
    CreateTaskInput {
      project_id,
      task_type: "query.answer".to_string(),
      title: "Query: attention".to_string(),
      relative_path: None,
      detail: json!({}),
      payload: json!({
        "query": "attention",
        "topK": 3
      }),
      created_by: admin_user_id,
      max_attempts: 3,
    },
  )
  .await
  .unwrap();

  let leased = store::acquire_next_task(&state, "worker-1", 30).await.unwrap().unwrap();
  assert_eq!(leased.id, task.id);
  assert_eq!(leased.status, "running");

  let completed = store::complete_task(&state, &task.id, json!({ "answer": "done" }))
    .await
    .unwrap();
  assert_eq!(completed.status, "succeeded");
}

#[tokio::test]
async fn startup_requeues_running_and_retry_waiting_tasks() {
  let env = TestEnvironment::start("task-recovery").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("task-recovery-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;
  let admin_user_id = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
    .bind("admin")
    .fetch_one(&state.pool)
    .await
    .unwrap();

  let running = store::create_task(
    &state,
    CreateTaskInput {
      project_id: project_id.clone(),
      task_type: "query.answer".to_string(),
      title: "Query: running".to_string(),
      relative_path: None,
      detail: json!({}),
      payload: json!({ "query": "running", "topK": 3 }),
      created_by: admin_user_id.clone(),
      max_attempts: 3,
    },
  )
  .await
  .unwrap();
  let waiting = store::create_task(
    &state,
    CreateTaskInput {
      project_id,
      task_type: "query.answer".to_string(),
      title: "Query: waiting".to_string(),
      relative_path: None,
      detail: json!({}),
      payload: json!({ "query": "waiting", "topK": 3 }),
      created_by: admin_user_id,
      max_attempts: 3,
    },
  )
  .await
  .unwrap();

  store::update_task_status(&state, &running.id, "running").await.unwrap();
  store::update_task_status(&state, &waiting.id, "retry_waiting").await.unwrap();

  knowledge_server::tasks::recovery::recover_tasks(&state).await.unwrap();

  let running_after = store::get_task_by_id(&state, &running.id).await.unwrap();
  let waiting_after = store::get_task_by_id(&state, &waiting.id).await.unwrap();
  assert_eq!(running_after.status, "queued");
  assert_eq!(waiting_after.status, "queued");
}

#[tokio::test]
async fn queued_task_creation_emits_worker_wakeup_signal() {
  let env = TestEnvironment::start("task-wakeup").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("task-wakeup-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query-tasks"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "query": "attention",
            "topK": 3
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::ACCEPTED);
  let wakeup: Option<String> = state
    .cache
    .get_json("tasks:wakeup")
    .await
    .unwrap();
  assert_eq!(wakeup.as_deref(), Some("queued"));
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

async fn get_task_detail(
  state: knowledge_server::app::state::AppState,
  cookie: &str,
  project_id: &str,
  task_id: &str,
) -> Value {
  let response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/query-tasks/{task_id}"))
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  read_json(response.into_body()).await
}

async fn read_json(body: Body) -> Value {
  let bytes = to_bytes(body, usize::MAX).await.unwrap();
  serde_json::from_slice(&bytes).unwrap()
}
