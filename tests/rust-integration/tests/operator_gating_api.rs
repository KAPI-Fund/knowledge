mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use tower::util::ServiceExt;
use uuid::Uuid;

async fn read_json(body: Body) -> Value {
  let bytes = to_bytes(body, usize::MAX).await.unwrap();
  serde_json::from_slice(&bytes).unwrap()
}

async fn insert_login_user(pool: &sqlx::PgPool, username: &str, password: &str) -> String {
  let id = Uuid::new_v4().to_string();
  let hash = knowledge_server::auth::password::hash_password(password).unwrap();
  sqlx::query(
    "INSERT INTO users (id, username, password_hash, role, created_at) \
     VALUES ($1, $2, $3, 'user', '2026-01-01T00:00:00Z')",
  )
  .bind(&id)
  .bind(username)
  .bind(&hash)
  .execute(pool)
  .await
  .unwrap();
  id
}

async fn login(
  state: &knowledge_server::app::state::AppState,
  username: &str,
  password: &str,
) -> (String, String) {
  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
          json!({ "username": username, "password": password }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  let cookie = response
    .headers()
    .get("set-cookie")
    .unwrap()
    .to_str()
    .unwrap()
    .to_string();
  let csrf = read_json(response.into_body()).await["csrfToken"]
    .as_str()
    .unwrap()
    .to_string();
  (cookie, csrf)
}

#[tokio::test]
async fn settings_get_is_operator_only() {
  let env = TestEnvironment::start("op-settings").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  insert_login_user(&state.pool, "plain", "pw-plain").await;

  // Non-operator: forbidden.
  let (cookie, _csrf) = login(&state, "plain", "pw-plain").await;
  let denied = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri("/api/system/settings")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(denied.status(), StatusCode::FORBIDDEN);

  // Seeded operator admin: allowed.
  let (admin_cookie, _admin_csrf) = login(&state, "admin", "secret-password").await;
  let allowed = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri("/api/system/settings")
        .header(header::COOKIE, &admin_cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(allowed.status(), StatusCode::OK);
}

#[tokio::test]
async fn users_list_is_operator_only() {
  let env = TestEnvironment::start("op-users").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  insert_login_user(&state.pool, "plain", "pw-plain").await;

  let (cookie, _csrf) = login(&state, "plain", "pw-plain").await;
  let denied = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri("/api/users")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(denied.status(), StatusCode::FORBIDDEN);

  let (admin_cookie, _admin_csrf) = login(&state, "admin", "secret-password").await;
  let allowed = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri("/api/users")
        .header(header::COOKIE, &admin_cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(allowed.status(), StatusCode::OK);
}
