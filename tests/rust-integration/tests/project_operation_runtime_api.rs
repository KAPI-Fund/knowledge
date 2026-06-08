mod support;

use std::fs;

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
async fn ingest_is_enqueued_and_completed_by_worker() {
  let env = TestEnvironment::start("project-op-exec").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("project-op-exec");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  let import = build_app(state.clone())
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
  assert_eq!(import.status(), StatusCode::ACCEPTED);

  let import_payload = read_json(import.into_body()).await;
  let import_task_id = import_payload.get("taskId").and_then(Value::as_str).unwrap().to_string();
  wait_for_task_terminal(&state, &import_task_id).await;

  let response = build_app(state.clone())
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
      .and_then(|result| result.get("summaryPath"))
      .and_then(Value::as_str),
    Some("wiki/sources/attention.md"),
  );

  let summary = fs::read_to_string(project_root.join("wiki/sources/attention.md")).unwrap();
  assert!(summary.contains("# Attention"));
}

#[tokio::test]
async fn review_update_is_enqueued_and_completed_by_worker() {
  let env = TestEnvironment::start("project-review-op-exec").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("project-review-op-exec");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  let import = build_app(state.clone())
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
  let import_payload = read_json(import.into_body()).await;
  wait_for_task_terminal(
    &state,
    import_payload.get("taskId").and_then(Value::as_str).unwrap(),
  )
  .await;

  let ingest = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/ingest"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({ "relativePath": "raw/sources/open-question.md" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  let ingest_payload = read_json(ingest.into_body()).await;
  wait_for_task_terminal(
    &state,
    ingest_payload.get("taskId").and_then(Value::as_str).unwrap(),
  )
  .await;

  let reviews = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/reviews"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  let reviews_payload = read_json(reviews.into_body()).await;
  let review_id = reviews_payload
    .get("reviews")
    .and_then(Value::as_array)
    .and_then(|items| items.first())
    .and_then(|item| item.get("id"))
    .and_then(Value::as_str)
    .unwrap()
    .to_string();

  let update = build_app(state.clone())
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

  assert_eq!(update.status(), StatusCode::ACCEPTED);
  let payload = read_json(update.into_body()).await;
  let task_id = payload.get("taskId").and_then(Value::as_str).unwrap().to_string();
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
async fn review_sweep_is_enqueued_and_completed_by_worker() {
  let env = TestEnvironment::start("project-review-sweep-op-exec").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("project-review-sweep-op-exec");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  fs::write(
    project_root.join("wiki/concepts/attention.md"),
    [
      "---",
      "type: concept",
      "title: Attention",
      "sources: []",
      "---",
      "",
      "# Attention",
    ]
    .join("\n"),
  )
  .unwrap();
  fs::write(
    project_root.join(".knowledge/reviews/items.json"),
    json!({
      "reviews": [
        {
          "id": "review-attention",
          "status": "open",
          "type": "missing-page",
          "title": "Missing page: attention",
          "description": "This page now exists.",
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

  let sweep = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/reviews:sweep"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(sweep.status(), StatusCode::ACCEPTED);
  let payload = read_json(sweep.into_body()).await;
  let task_id = payload.get("taskId").and_then(Value::as_str).unwrap().to_string();
  wait_for_task_terminal(&state, &task_id).await;

  let detail = store::get_task_by_id(&state, &task_id).await.unwrap();
  assert_eq!(detail.status, "succeeded");
  assert_eq!(
    detail.result.as_ref().and_then(|result| result.get("resolvedIds")),
    Some(&json!(["review-attention"])),
  );

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
