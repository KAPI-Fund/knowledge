mod support;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::tasks::{scheduler, store};
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{json, Value};
use support::mock_openai::{MockOpenAiServer, MockScenario};
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
            "defaultQueryLimit": 3,
            "providerBaseUrl": "http://127.0.0.1:18080/v1",
            "providerApiKey": "test-key",
            "providerModel": "mock-model",
            "providerEmbeddingModel": "mock-embedding",
            "providerTimeoutSeconds": 45
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
  assert_eq!(
    payload.get("providerBaseUrl").and_then(Value::as_str),
    Some("http://127.0.0.1:18080/v1"),
  );
  assert_eq!(payload.get("providerApiKeyConfigured").and_then(Value::as_bool), Some(true));
  assert_eq!(payload.get("providerModel").and_then(Value::as_str), Some("mock-model"));
  assert_eq!(
    payload.get("providerEmbeddingModel").and_then(Value::as_str),
    Some("mock-embedding"),
  );
  assert_eq!(payload.get("providerTimeoutSeconds").and_then(Value::as_i64), Some(45));
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

  let import_response = build_app(state.clone())
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
  assert_eq!(import_response.status(), StatusCode::ACCEPTED);
  let import_payload = read_json(import_response.into_body()).await;
  wait_for_task_terminal(
    &state,
    import_payload.get("taskId").and_then(Value::as_str).unwrap(),
  )
  .await;

  let ingest_response = build_app(state.clone())
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
  assert_eq!(ingest_response.status(), StatusCode::ACCEPTED);
  let ingest_payload = read_json(ingest_response.into_body()).await;
  wait_for_task_terminal(
    &state,
    ingest_payload.get("taskId").and_then(Value::as_str).unwrap(),
  )
  .await;

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

  let update_response = build_app(state.clone())
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

  assert_eq!(update_response.status(), StatusCode::ACCEPTED);
  let update_payload = read_json(update_response.into_body()).await;
  let task_id = update_payload.get("taskId").and_then(Value::as_str).unwrap().to_string();
  wait_for_task_terminal(&state, &task_id).await;

  let reviews_after = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/reviews?status=all"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  let after_payload = read_json(reviews_after.into_body()).await;
  assert_eq!(
    after_payload
      .get("reviews")
      .and_then(Value::as_array)
      .and_then(|items| items.first())
      .and_then(|item| item.get("status"))
      .and_then(Value::as_str),
    Some("resolved"),
  );
}

#[tokio::test]
async fn provider_generated_review_blocks_are_persisted_during_ingest() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("provider-review-items").await.unwrap();
  let mock = MockOpenAiServer::start(MockScenario::ingest_with_review_success()).await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("provider-review-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
  configure_provider(&state, &mock.base_url(), "mock-model").await;

  let import_response = build_app(state.clone())
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
  assert_eq!(import_response.status(), StatusCode::ACCEPTED);
  let import_payload = read_json(import_response.into_body()).await;
  wait_for_task_terminal(
    &state,
    import_payload.get("taskId").and_then(Value::as_str).unwrap(),
  )
  .await;

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
  assert_eq!(ingest_response.status(), StatusCode::ACCEPTED);
  let ingest_payload = read_json(ingest_response.into_body()).await;
  wait_for_task_terminal(
    &state,
    ingest_payload.get("taskId").and_then(Value::as_str).unwrap(),
  )
  .await;

  let reviews_response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/reviews?status=all"))
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
  assert_eq!(
    reviews[0].get("title").and_then(Value::as_str),
    Some("Missing evaluation page")
  );
  assert_eq!(
    reviews[0].get("type").and_then(Value::as_str),
    Some("missing-page")
  );
  assert_eq!(
    reviews[0].get("sourcePath").and_then(Value::as_str),
    Some("raw/sources/attention.md")
  );
  assert_eq!(
    reviews[0]
      .get("affectedPages")
      .and_then(Value::as_array)
      .map(Vec::len),
    Some(1)
  );
  assert_eq!(
    reviews[0]
      .get("searchQueries")
      .and_then(Value::as_array)
      .map(Vec::len),
    Some(2)
  );
  assert_eq!(
    reviews[0]
      .get("options")
      .and_then(Value::as_array)
      .map(Vec::len),
    Some(2)
  );
  assert!(
    reviews[0]
      .get("description")
      .and_then(Value::as_str)
      .unwrap()
      .contains("Create a dedicated page")
  );
}

#[tokio::test]
async fn final_ingest_triggers_review_sweep_for_existing_missing_page_items() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("review-sweep").await.unwrap();
  let mock = MockOpenAiServer::start(MockScenario::ingest_success()).await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("review-sweep-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
  configure_provider(&state, &mock.base_url(), "mock-model").await;

  std::fs::write(
    project_root.join(".knowledge/reviews/items.json"),
    json!({
      "reviews": [
        {
          "id": "review-missing-attention-mechanism",
          "status": "open",
          "type": "missing-page",
          "title": "Missing page: attention mechanism",
          "description": "The concept page does not exist yet.",
          "options": [
            { "label": "Approve", "action": "Approve" },
            { "label": "Skip", "action": "Skip" }
          ]
        }
      ]
    })
    .to_string(),
  )
  .unwrap();

  std::fs::write(
    project_root.join("raw/sources/attention.md"),
    "# Attention\n\nTransformers use attention mechanisms.\n",
  )
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
  assert_eq!(ingest_response.status(), StatusCode::ACCEPTED);
  let ingest_payload = read_json(ingest_response.into_body()).await;
  let task_id = ingest_payload.get("taskId").and_then(Value::as_str).unwrap().to_string();
  wait_for_task_terminal(&state, &task_id).await;

  let task = store::get_task_by_id(&state, &task_id).await.unwrap();
  assert_eq!(task.status, "succeeded");
  assert_eq!(
    task.result.as_ref().and_then(|result| result.get("reviewSweep")),
    Some(&json!({
      "resolvedIds": ["review-missing-attention-mechanism"],
      "unresolvedIds": []
    })),
  );

  let reviews_response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/reviews?status=all"))
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
  assert_eq!(reviews[0].get("status").and_then(Value::as_str), Some("resolved"));
}

#[tokio::test]
async fn final_ingest_uses_provider_to_semantically_resolve_remaining_reviews() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("review-sweep-llm").await.unwrap();
  let mock = MockOpenAiServer::start(MockScenario::ingest_then_sweep_llm_success()).await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("review-sweep-llm-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
  configure_provider(&state, &mock.base_url(), "mock-model").await;

  std::fs::write(
    project_root.join(".knowledge/reviews/items.json"),
    json!({
      "reviews": [
        {
          "id": "review-context-window",
          "status": "open",
          "type": "missing-page",
          "title": "Missing page: Context Window",
          "description": "The wiki might already cover this concept indirectly.",
          "options": [
            { "label": "Approve", "action": "Approve" },
            { "label": "Skip", "action": "Skip" }
          ]
        }
      ]
    })
    .to_string(),
  )
  .unwrap();

  std::fs::write(
    project_root.join("raw/sources/attention.md"),
    "# Attention\n\nAttention defines an effective context window for transformer models.\n",
  )
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
  assert_eq!(ingest_response.status(), StatusCode::ACCEPTED);
  let ingest_payload = read_json(ingest_response.into_body()).await;
  let task_id = ingest_payload.get("taskId").and_then(Value::as_str).unwrap().to_string();
  wait_for_task_terminal(&state, &task_id).await;

  let task = store::get_task_by_id(&state, &task_id).await.unwrap();
  assert_eq!(task.status, "succeeded");
  assert_eq!(
    task.result.as_ref().and_then(|result| result.get("reviewSweep")),
    Some(&json!({
      "resolvedIds": ["review-context-window"],
      "unresolvedIds": []
    })),
  );

  let reviews_response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/reviews?status=all"))
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
  assert_eq!(reviews[0].get("status").and_then(Value::as_str), Some("resolved"));
}

#[tokio::test]
async fn review_endpoint_supports_status_type_and_limit_filters() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("review-filters").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = temp.path().join("review-filter-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  std::fs::write(
    project_root.join(".knowledge/reviews/items.json"),
    json!({
      "reviews": [
        {
          "id": "review-1",
          "status": "open",
          "type": "missing-page",
          "title": "Missing page: Attention",
          "description": "Open item"
        },
        {
          "id": "review-2",
          "status": "resolved",
          "type": "missing-page",
          "title": "Missing page: Context Window",
          "description": "Resolved item"
        },
        {
          "id": "review-3",
          "status": "open",
          "type": "duplicate",
          "title": "Duplicate page: Attention",
          "description": "Another open item"
        }
      ]
    })
    .to_string(),
  )
  .unwrap();

  let default_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/reviews"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(default_response.status(), StatusCode::OK);
  let default_payload = read_json(default_response.into_body()).await;
  let default_reviews = default_payload.get("reviews").and_then(Value::as_array).unwrap();
  assert_eq!(default_reviews.len(), 2);
  assert!(default_reviews.iter().all(|item| item.get("status").and_then(Value::as_str) != Some("resolved")));

  let filtered_response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!(
          "/api/projects/{project_id}/reviews?status=all&type=missing-page&limit=1"
        ))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(filtered_response.status(), StatusCode::OK);
  let filtered_payload = read_json(filtered_response.into_body()).await;
  let filtered_reviews = filtered_payload.get("reviews").and_then(Value::as_array).unwrap();
  assert_eq!(filtered_reviews.len(), 1);
  assert_eq!(filtered_reviews[0].get("id").and_then(Value::as_str), Some("review-1"));
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
