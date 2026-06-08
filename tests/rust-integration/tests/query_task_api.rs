mod support;

use std::fs;

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
async fn query_task_creation_returns_task_id_and_terminal_result() {
  let env = TestEnvironment::start("query-task-api").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("query-task-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
  seed_search_page(&project_root).await;

  let create = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query-tasks"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "query": "What is attention?",
            "topK": 3
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(create.status(), StatusCode::ACCEPTED);
  let payload = read_json(create.into_body()).await;
  assert_eq!(payload.get("status").and_then(Value::as_str), Some("queued"));
  assert!(payload.get("taskId").and_then(Value::as_str).is_some());
}

#[tokio::test]
async fn query_task_fails_when_provider_configuration_is_missing() {
  let env = TestEnvironment::start("provider-missing").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("provider-missing-project");
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

  let result = knowledge_server::tasks::executors::run_query_executor(&state, &task).await;
  assert!(result.is_err());

  let detail = store::get_task_by_id(&state, &task.id).await.unwrap();
  assert_eq!(detail.status, "failed");
  assert_eq!(
    detail
      .error
      .as_ref()
      .and_then(|error| error.get("retryable"))
      .and_then(Value::as_bool),
    Some(false),
  );
}

#[tokio::test]
async fn query_task_runs_to_completion_and_persists_answer_and_citations() {
  let env = TestEnvironment::start("query-exec").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("query-exec-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
  seed_search_page(&project_root).await;
  sqlx::query(
    "UPDATE system_settings
     SET provider_mode = $1,
         provider_base_url = $2,
         provider_model = $3",
  )
  .bind("openai-compatible")
  .bind("mock://provider")
  .bind("mock-model")
  .execute(&state.pool)
  .await
  .unwrap();

  let create = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query-tasks"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({
            "query": "What is attention?",
            "topK": 3
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(create.status(), StatusCode::ACCEPTED);
  let payload = read_json(create.into_body()).await;
  let task_id = payload.get("taskId").and_then(Value::as_str).unwrap().to_string();
  let task = store::get_task_by_id(&state, &task_id).await.unwrap();

  knowledge_server::tasks::executors::run_query_executor(&state, &task)
    .await
    .unwrap();

  let detail = store::get_task_by_id(&state, &task_id).await.unwrap();
  assert_eq!(detail.status, "succeeded");
  assert!(
    detail
      .result
      .as_ref()
      .and_then(|result| result.get("answer"))
      .and_then(Value::as_str)
      .is_some()
  );
  assert_eq!(
    detail
      .result
      .as_ref()
      .and_then(|result| result.get("citations"))
      .and_then(Value::as_array)
      .map(Vec::len),
    Some(1),
  );
}

async fn seed_search_page(project_root: &std::path::Path) {
  fs::write(
    project_root.join("wiki/concepts/attention.md"),
    "---\ntype: concept\ntitle: Attention\nsources: [attention.md]\n---\n\n# Attention\n\nAttention lets models focus on relevant tokens.\n",
  )
  .unwrap();
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
