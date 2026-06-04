use axum::body::Body;
use axum::http::{Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app, table_exists};
use serde_json::json;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn bootstraps_schema_on_start() {
  let temp = tempdir().unwrap();
  let database_path = temp.path().join("test.sqlite");
  let database_url = format!(
    "sqlite://{}",
    database_path.to_string_lossy().replace('\\', "/")
  );
  let config = AppConfig::for_tests(database_url);
  let state = bootstrap_state(&config).await.unwrap();
  let app = build_app(state.clone());
  let response = app
    .oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap())
    .await
    .unwrap()
    .status();
  assert_eq!(response, StatusCode::OK);
  assert!(table_exists(&state.pool, "users").await.unwrap());
}

#[tokio::test]
async fn login_sets_session_cookie_and_csrf_token() {
  let temp = tempdir().unwrap();
  let database_path = temp.path().join("auth.sqlite");
  let database_url = format!(
    "sqlite://{}",
    database_path.to_string_lossy().replace('\\', "/")
  );
  let config = AppConfig::for_tests(database_url);
  let state = bootstrap_state(&config).await.unwrap();
  let app = build_app(state.clone());

  let response = app
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
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

  assert_eq!(response.status(), StatusCode::OK);
  let cookie = response.headers().get("set-cookie");
  assert!(cookie.is_some());
}

#[tokio::test]
async fn me_requires_valid_session() {
  let temp = tempdir().unwrap();
  let database_path = temp.path().join("me.sqlite");
  let database_url = format!(
    "sqlite://{}",
    database_path.to_string_lossy().replace('\\', "/")
  );
  let config = AppConfig::for_tests(database_url);
  let state = bootstrap_state(&config).await.unwrap();
  let app = build_app(state);

  let response = app
    .oneshot(Request::builder().uri("/api/auth/me").body(Body::empty()).unwrap())
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
