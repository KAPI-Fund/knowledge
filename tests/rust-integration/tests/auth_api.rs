mod support;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app, table_exists};
use serde_json::{json, Value};
use support::TestEnvironment;
use tower::util::ServiceExt;
use axum::body::to_bytes;

#[tokio::test]
async fn bootstraps_schema_on_start() {
  let env = TestEnvironment::start("bootstraps-schema").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
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
  let env = TestEnvironment::start("login-session").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
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
async fn login_caches_session_in_redis() {
  let env = TestEnvironment::start("login-cache").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let app = build_app(state);

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
  let session_id = response
    .headers()
    .get("set-cookie")
    .and_then(|value| value.to_str().ok())
    .and_then(|value| {
      value
        .split(';')
        .map(str::trim)
        .find(|part| part.starts_with("knowledge_session="))
        .map(|part| part.trim_start_matches("knowledge_session=").to_string())
    })
    .unwrap();

  let cached = env
    .redis_get(&format!("session:{session_id}"))
    .await
    .unwrap();
  assert!(cached.is_some());
}

#[tokio::test]
async fn me_requires_valid_session() {
  let env = TestEnvironment::start("me-session").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let app = build_app(state);

  let response = app
    .oneshot(Request::builder().uri("/api/auth/me").body(Body::empty()).unwrap())
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_returns_current_user_payload() {
  let env = TestEnvironment::start("me-payload").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let app = build_app(state.clone());

  let login = app
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
  let cookie = login
    .headers()
    .get("set-cookie")
    .unwrap()
    .to_str()
    .unwrap()
    .to_string();

  let response = build_app(state)
    .oneshot(
      Request::builder()
        .uri("/api/auth/me")
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
  let payload: Value = serde_json::from_slice(&bytes).unwrap();
  assert_eq!(
    payload.get("user").and_then(|user| user.get("username")).and_then(Value::as_str),
    Some("admin")
  );
}

#[tokio::test]
async fn concurrent_login_requests_do_not_fail_when_bootstrapping_admin() {
  let env = TestEnvironment::start("concurrent-login").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  let request = || {
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
      .unwrap()
  };

  let first = build_app(state.clone()).oneshot(request());
  let second = build_app(state).oneshot(request());
  let (first, second) = tokio::join!(first, second);

  assert_eq!(first.unwrap().status(), StatusCode::OK);
  assert_eq!(second.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn login_rejects_unknown_username() {
  let env = TestEnvironment::start("login-unknown-user").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
          json!({ "username": "hacker", "password": "any-password" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

  let count =
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = 'hacker'")
      .fetch_one(&state.pool)
      .await
      .unwrap();
  assert_eq!(count, 0, "unknown username must not be persisted to the database");
}

#[tokio::test]
async fn register_creates_user_with_role_and_session_cookie() {
  let env = TestEnvironment::start("register-create").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let app = build_app(state.clone());

  let response = app
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
          json!({ "username": "newcomer", "password": "hunter2-pass" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::CREATED);
  assert!(response.headers().get("set-cookie").is_some());

  let role = sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE username = 'newcomer'")
    .fetch_one(&state.pool)
    .await
    .unwrap();
  assert_eq!(role, "user", "self-registered users must get the non-operator role");
}

#[tokio::test]
async fn register_rejects_duplicate_username() {
  let env = TestEnvironment::start("register-duplicate").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  // 'admin' is seeded by bootstrap; a second account with the same name must 400.
  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
          json!({ "username": "admin", "password": "another-pass" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_rejects_short_password() {
  let env = TestEnvironment::start("register-short-pwd").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
          json!({ "username": "shorty", "password": "short" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::BAD_REQUEST);

  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE username = 'shorty'")
    .fetch_one(&state.pool)
    .await
    .unwrap();
  assert_eq!(count, 0, "rejected registration must not persist a user");
}

#[tokio::test]
async fn register_then_me_returns_new_user() {
  let env = TestEnvironment::start("register-then-me").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  let register = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
          json!({ "username": "fresh-user", "password": "hunter2-pass" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(register.status(), StatusCode::CREATED);
  let cookie = register
    .headers()
    .get("set-cookie")
    .unwrap()
    .to_str()
    .unwrap()
    .to_string();

  let response = build_app(state)
    .oneshot(
      Request::builder()
        .uri("/api/auth/me")
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
  let payload: Value = serde_json::from_slice(&bytes).unwrap();
  assert_eq!(
    payload.get("user").and_then(|user| user.get("role")).and_then(Value::as_str),
    Some("user")
  );
}

#[tokio::test]
async fn bootstrap_without_admin_password_skips_admin_creation() {
  let env = TestEnvironment::start("bootstrap-no-pwd").await.unwrap();
  let mut config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  config.admin_password = None;
  let state = bootstrap_state(&config).await.unwrap();

  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
    .fetch_one(&state.pool)
    .await
    .unwrap();
  assert_eq!(count, 0, "bootstrap must not create any user when admin_password is None");
}
