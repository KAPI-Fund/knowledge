use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn admin_can_create_project_with_absolute_root() {
  let temp = tempdir().unwrap();
  let database_path = temp.path().join("project.sqlite");
  let database_url = format!(
    "sqlite://{}",
    database_path.to_string_lossy().replace('\\', "/")
  );
  let config = AppConfig::for_tests(database_url);
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
}

#[tokio::test]
async fn non_member_cannot_list_project_members() {
  let temp = tempdir().unwrap();
  let database_path = temp.path().join("members.sqlite");
  let database_url = format!(
    "sqlite://{}",
    database_path.to_string_lossy().replace('\\', "/")
  );
  let config = AppConfig::for_tests(database_url);
  let state = bootstrap_state(&config).await.unwrap();
  let app = build_app(state);

  let response = app
    .oneshot(Request::builder().uri("/api/projects/project-1/members").body(Body::empty()).unwrap())
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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
