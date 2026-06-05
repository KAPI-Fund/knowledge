mod support;

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{json, Value};
use support::TestEnvironment;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn admin_can_create_project_with_absolute_root() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("project-create").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  let login_app = build_app(state.clone());
  let login = login(login_app).await;
  let cookie = login
    .headers()
    .get(header::SET_COOKIE)
    .unwrap()
    .to_str()
    .unwrap()
    .to_string();
  let body = read_json(login.into_body()).await;
  let csrf = body.get("csrfToken").and_then(Value::as_str).unwrap();

  let project_root = temp.path().join("registered-project");
  std::fs::create_dir_all(&project_root).unwrap();

  let app = build_app(state);
  let response = app
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, cookie)
        .header("x-csrf-token", csrf)
        .body(Body::from(
          json!({
            "name": "knowledge-demo",
            "rootPath": project_root.to_string_lossy()
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::CREATED);
  assert!(project_root.join("purpose.md").is_file());
  assert!(project_root.join("schema.md").is_file());
  assert!(project_root.join("wiki/index.md").is_file());
  assert!(project_root.join("wiki/log.md").is_file());
  assert!(project_root.join("wiki/overview.md").is_file());
  assert!(project_root.join("raw/sources").is_dir());
  assert!(project_root.join("raw/assets").is_dir());
  assert!(project_root.join(".knowledge/ingest").is_dir());
}

#[tokio::test]
async fn non_member_cannot_list_project_members() {
  let _env = TestEnvironment::start("project-members").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let app = build_app(state);

  let response = app
    .oneshot(Request::builder().uri("/api/projects/project-1/members").body(Body::empty()).unwrap())
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_projects_and_users_return_registered_data() {
  let temp = tempdir().unwrap();
  let _env = TestEnvironment::start("project-listing").await.unwrap();
  let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  let login_app = build_app(state.clone());
  let login = login(login_app).await;
  let cookie = login
    .headers()
    .get(header::SET_COOKIE)
    .unwrap()
    .to_str()
    .unwrap()
    .to_string();
  let body = read_json(login.into_body()).await;
  let csrf = body.get("csrfToken").and_then(Value::as_str).unwrap();

  let project_root = temp.path().join("listed-project");
  std::fs::create_dir_all(&project_root).unwrap();
  let _ = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", csrf)
        .body(Body::from(
          json!({
            "name": "listed-project",
            "rootPath": project_root.to_string_lossy()
          })
          .to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  let projects_response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri("/api/projects")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(projects_response.status(), StatusCode::OK);
  let projects_payload = read_json(projects_response.into_body()).await;
  let projects = projects_payload.get("projects").and_then(Value::as_array).unwrap();
  assert_eq!(projects.len(), 1);
  assert_eq!(projects[0].get("name").and_then(Value::as_str), Some("listed-project"));

  let users_response = build_app(state)
    .oneshot(
      Request::builder()
        .uri("/api/users")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(users_response.status(), StatusCode::OK);
  let users_payload = read_json(users_response.into_body()).await;
  let users = users_payload.get("users").and_then(Value::as_array).unwrap();
  assert_eq!(users.len(), 1);
  assert_eq!(users[0].get("username").and_then(Value::as_str), Some("admin"));
}

async fn login(app: axum::Router) -> axum::response::Response {
  app
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
    .unwrap()
}

async fn read_json(body: Body) -> Value {
  let bytes = to_bytes(body, usize::MAX).await.unwrap();
  serde_json::from_slice(&bytes).unwrap()
}
