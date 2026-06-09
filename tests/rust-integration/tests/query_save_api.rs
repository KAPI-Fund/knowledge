mod support;

use std::fs;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::store::{self, CreateTaskInput};
use knowledge_server::{build_app, tasks};
use serde_json::{json, Value};
use support::mock_openai::{MockOpenAiServer, MockScenario};
use support::{bootstrap_state_without_scheduler, TestEnvironment};
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn save_query_answer_task_writes_query_page_and_updates_index_and_log() {
  let env = TestEnvironment::start("query-save").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state_without_scheduler(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("query-save-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
  let query_task_id = seed_successful_query_task(&state, &project_id).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query-tasks/{query_task_id}/save"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
          json!({ "title": "Attention Notes" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::ACCEPTED);
  let payload = read_json(response.into_body()).await;
  let task_id = payload.get("taskId").and_then(Value::as_str).unwrap().to_string();
  wait_for_task_terminal(&state, &task_id).await;

  let detail = store::get_task_by_id(&state, &task_id).await.unwrap();
  assert_eq!(detail.status, "succeeded");
  assert_eq!(
    detail
      .result
      .as_ref()
      .and_then(|result| result.get("relativePath"))
      .and_then(Value::as_str),
    Some("wiki/queries/attention-notes.md"),
  );

  let page = fs::read_to_string(project_root.join("wiki/queries/attention-notes.md")).unwrap();
  assert!(page.contains("type: query"));
  assert!(page.contains("Attention focuses computation on relevant tokens."));

  let index = fs::read_to_string(project_root.join("wiki/index.md")).unwrap();
  assert!(index.contains("[[queries/attention-notes]]"));

  let log = fs::read_to_string(project_root.join("wiki/log.md")).unwrap();
  assert!(log.contains("query | Attention Notes"));
}

#[tokio::test]
async fn save_query_answer_task_enriches_saved_page_with_wikilinks_when_provider_is_configured() {
  let env = TestEnvironment::start("query-save-enrich").await.unwrap();
  let mock = MockOpenAiServer::start(MockScenario::query_save_enrich_success())
    .await
    .unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state_without_scheduler(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("query-save-enrich-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
  configure_provider(&state, &mock.base_url(), "mock-model").await;
  let query_task_id = seed_successful_query_task(&state, &project_id).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query-tasks/{query_task_id}/save"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
          json!({ "title": "Attention Notes" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::ACCEPTED);
  let payload = read_json(response.into_body()).await;
  let task_id = payload.get("taskId").and_then(Value::as_str).unwrap().to_string();
  wait_for_task_terminal(&state, &task_id).await;

  let page = fs::read_to_string(project_root.join("wiki/queries/attention-notes.md")).unwrap();
  assert!(page.contains("[[attention]]") || page.contains("[[Attention]]"));
  assert_eq!(mock.request_count(), 1);
}

async fn seed_successful_query_task(
  state: &knowledge_server::app::state::AppState,
  project_id: &str,
) -> String {
  let admin_user_id = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
    .bind("admin")
    .fetch_one(&state.pool)
    .await
    .unwrap();
  let task = store::create_task(
    state,
    CreateTaskInput {
      project_id: project_id.to_string(),
      task_type: "query.answer".to_string(),
      title: "Query: What is attention?".to_string(),
      relative_path: None,
      detail: json!({}),
      payload: json!({
        "query": "What is attention?",
        "topK": 3,
        "language": "en"
      }),
      created_by: admin_user_id,
      max_attempts: 3,
    },
  )
  .await
  .unwrap();

  store::complete_task(
    state,
    &task.id,
    json!({
      "answer": "Attention focuses computation on relevant tokens.",
      "citations": [
        {
          "path": "wiki/concepts/attention.md",
          "title": "Attention",
          "snippet": "Attention lets models focus on relevant tokens.",
          "score": 1
        }
      ],
      "contextSummary": "wiki/concepts/attention.md (Attention)",
      "model": "mock-model",
      "provider": "openai-compatible",
      "usage": {
        "promptTokens": 17,
        "completionTokens": 25,
        "totalTokens": 42
      }
    }),
  )
  .await
  .unwrap();

  task.id
}

async fn wait_for_task_terminal(state: &knowledge_server::app::state::AppState, task_id: &str) {
  for _ in 0..40 {
    let progressed = tasks::scheduler::run_scheduler_tick(state).await.unwrap_or(false);
    let task = store::get_task_by_id(state, task_id).await.unwrap();
    if matches!(task.status.as_str(), "succeeded" | "failed" | "cancelled") {
      return;
    }
    if !progressed {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  }

  panic!("task {task_id} did not reach a terminal state");
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

async fn configure_provider(
  state: &knowledge_server::app::state::AppState,
  base_url: &str,
  model: &str,
) {
  sqlx::query(
    "UPDATE system_settings
     SET provider_mode = $1,
         provider_base_url = $2,
         provider_api_key = $3,
         provider_model = $4,
         provider_timeout_seconds = $5",
  )
  .bind("openai-compatible")
  .bind(base_url)
  .bind("test-key")
  .bind(model)
  .bind(30_i64)
  .execute(&state.pool)
  .await
  .unwrap();
}
