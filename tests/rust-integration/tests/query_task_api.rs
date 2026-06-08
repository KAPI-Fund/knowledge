mod support;

use std::fs;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::providers::{OpenAiCompatibleProvider, ProviderQueryRequest};
use knowledge_server::tasks::store::{self, CreateTaskInput};
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{json, Value};
use support::mock_openai::{MockOpenAiServer, MockScenario};
use support::{bootstrap_state_without_scheduler, TestEnvironment};
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
  let mock = MockOpenAiServer::start(MockScenario::success()).await.unwrap();
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
         provider_api_key = $3,
         provider_model = $4,
         provider_timeout_seconds = $5",
  )
  .bind("openai-compatible")
  .bind(mock.base_url())
  .bind("test-key")
  .bind("mock-model")
  .bind(30_i64)
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

#[tokio::test]
async fn query_task_uses_openai_compatible_provider_response() {
  let env = TestEnvironment::start("query-provider").await.unwrap();
  let mock = MockOpenAiServer::start(MockScenario::success()).await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  configure_provider(&state, &mock.base_url(), "mock-model").await;
  let project_root = tempdir().unwrap().path().join("query-provider-project");
  let project_id = seed_project_with_attention_page(&state, &project_root).await;
  let task = create_query_task_via_api(&state, &project_id, "What is attention?").await;

  wait_for_task_terminal(&state, &task.id).await;

  let detail = store::get_task_by_id(&state, &task.id).await.unwrap();
  assert_eq!(detail.status, "succeeded");
  assert_eq!(
    detail.result.as_ref().unwrap()["answer"],
    "Attention focuses computation on relevant tokens."
  );
  assert_eq!(detail.result.as_ref().unwrap()["usage"]["totalTokens"], 42);
}

#[tokio::test]
async fn provider_maps_openai_compatible_response_into_internal_answer() {
  let server = MockOpenAiServer::start(MockScenario::success()).await.unwrap();
  let client = OpenAiCompatibleProvider::new(server.base_url(), "test-key", "mock-model", 30);

  let answer = client
    .answer_query(ProviderQueryRequest {
      query: "What is attention?".to_string(),
      context_blocks: vec![
        "[1] wiki/concepts/attention.md\nAttention lets models focus on relevant tokens."
          .to_string(),
      ],
      language: "en".to_string(),
    })
    .await
    .unwrap();

  assert_eq!(answer.answer, "Attention focuses computation on relevant tokens.");
  assert_eq!(answer.usage.total_tokens, 42);
}

#[tokio::test]
async fn retryable_provider_failure_enters_retry_waiting_with_next_retry_at() {
  let env = TestEnvironment::start("query-retry").await.unwrap();
  let mock = MockOpenAiServer::start(MockScenario::retryable_error()).await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state_without_scheduler(&config).await.unwrap();
  configure_provider(&state, &mock.base_url(), "mock-model").await;
  let project_root = tempdir().unwrap().path().join("query-retry-project");
  let project_id = seed_project_with_attention_page(&state, &project_root).await;
  let task = seed_query_task(&state, &project_id, "attention").await;

  knowledge_server::tasks::scheduler::run_scheduler_tick(&state)
    .await
    .unwrap_err();

  let detail = store::get_task_by_id(&state, &task.id).await.unwrap();
  assert_eq!(detail.status, "retry_waiting");
  assert!(detail.next_retry_at.is_some());
  assert_eq!(detail.error.as_ref().unwrap()["retryable"], true);
  assert_eq!(detail.error.as_ref().unwrap()["code"], "provider_rate_limited");
  assert_eq!(detail.error.as_ref().unwrap()["providerStatus"], 429);
}

#[tokio::test]
async fn non_retryable_provider_failure_fails_immediately() {
  let env = TestEnvironment::start("query-invalid-request").await.unwrap();
  let mock = MockOpenAiServer::start(MockScenario::invalid_request()).await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state_without_scheduler(&config).await.unwrap();
  configure_provider(&state, &mock.base_url(), "mock-model").await;
  let project_root = tempdir().unwrap().path().join("query-invalid-request-project");
  let project_id = seed_project_with_attention_page(&state, &project_root).await;
  let task = seed_query_task(&state, &project_id, "attention").await;

  knowledge_server::tasks::scheduler::run_scheduler_tick(&state)
    .await
    .unwrap_err();

  let detail = store::get_task_by_id(&state, &task.id).await.unwrap();
  assert_eq!(detail.status, "failed");
  assert_eq!(detail.error.as_ref().unwrap()["retryable"], false);
  assert_eq!(detail.error.as_ref().unwrap()["code"], "provider_invalid_request");
  assert_eq!(detail.error.as_ref().unwrap()["providerStatus"], 400);
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

async fn seed_project_with_attention_page(
  state: &knowledge_server::app::state::AppState,
  project_root: &std::path::Path,
) -> String {
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.to_path_buf()).await;
  seed_search_page(project_root).await;
  project_id
}

async fn create_query_task_via_api(
  state: &knowledge_server::app::state::AppState,
  project_id: &str,
  query: &str,
) -> knowledge_server::tasks::model::TaskRecord {
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
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
            "query": query,
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
  let task_id = payload.get("taskId").and_then(Value::as_str).unwrap();
  store::get_task_by_id(state, task_id).await.unwrap()
}

async fn seed_query_task(
  state: &knowledge_server::app::state::AppState,
  project_id: &str,
  query: &str,
) -> knowledge_server::tasks::model::TaskRecord {
  let _ = login_and_csrf(state.clone()).await;
  let admin_user_id = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
    .bind("admin")
    .fetch_one(&state.pool)
    .await
    .unwrap();

  store::create_task(
    state,
    CreateTaskInput {
      project_id: project_id.to_string(),
      task_type: "query.answer".to_string(),
      title: format!("Query: {query}"),
      relative_path: None,
      detail: json!({}),
      payload: json!({
        "query": query,
        "topK": 3,
        "language": "en"
      }),
      created_by: admin_user_id,
      max_attempts: 3,
    },
  )
  .await
  .unwrap()
}

async fn wait_for_task_terminal(state: &knowledge_server::app::state::AppState, task_id: &str) {
  for _ in 0..80 {
    let detail = store::get_task_by_id(state, task_id).await.unwrap();
    if matches!(detail.status.as_str(), "succeeded" | "failed" | "cancelled") {
      return;
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  panic!("task {task_id} did not reach a terminal state in time");
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
