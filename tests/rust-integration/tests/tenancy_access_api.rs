mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app, table_exists};
use serde_json::{Value, json};
use support::TestEnvironment;
use tower::util::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn migration_creates_tenancy_tables_and_personal_space_uniqueness() {
  let env = TestEnvironment::start("tenancy-migration").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  assert!(table_exists(&state.pool, "organizations").await.unwrap());
  assert!(table_exists(&state.pool, "organization_members").await.unwrap());
  assert!(table_exists(&state.pool, "spaces").await.unwrap());

  // The seeded admin has exactly one personal space.
  let admin_id = sqlx::query_scalar::<_, String>(
    "SELECT id FROM users WHERE username = 'admin'",
  )
  .fetch_one(&state.pool)
  .await
  .unwrap();

  let space_count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM spaces WHERE kind = 'personal' AND owner_user_id = $1",
  )
  .bind(&admin_id)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(space_count, 1, "admin must have exactly one personal space");

  // The partial unique index forbids a second personal space for the same user.
  let duplicate = sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, created_at) \
     VALUES ($1, 'personal', $2, NULL, '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(&admin_id)
  .execute(&state.pool)
  .await;
  assert!(duplicate.is_err(), "second personal space for a user must be rejected");
}

#[tokio::test]
async fn seeded_admin_is_operator_with_personal_space() {
  let env = TestEnvironment::start("tenancy-seed-admin").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  let role = sqlx::query_scalar::<_, String>(
    "SELECT role FROM users WHERE username = 'admin'",
  )
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(role, "operator");

  let admin_id = sqlx::query_scalar::<_, String>(
    "SELECT id FROM users WHERE username = 'admin'",
  )
  .fetch_one(&state.pool)
  .await
  .unwrap();
  let space = knowledge_server::tenancy::spaces::personal_space_id(&state.pool, &admin_id)
    .await
    .unwrap();
  assert!(space.is_some(), "seeded admin must have a personal space");
}

async fn login_admin(state: &knowledge_server::app::state::AppState) -> (String, String) {
  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
          json!({ "username": "admin", "password": "secret-password" }).to_string(),
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
  let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
  let payload: Value = serde_json::from_slice(&bytes).unwrap();
  let csrf = payload
    .get("csrfToken")
    .and_then(Value::as_str)
    .unwrap()
    .to_string();
  (cookie, csrf)
}

#[tokio::test]
async fn created_project_belongs_to_owner_personal_space() {
  let env = TestEnvironment::start("tenancy-create-project").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "name": "tenancy-demo" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::CREATED);
  let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
  let payload: Value = serde_json::from_slice(&bytes).unwrap();
  let project_id = payload.get("id").and_then(Value::as_str).unwrap();

  let admin_id = sqlx::query_scalar::<_, String>(
    "SELECT id FROM users WHERE username = 'admin'",
  )
  .fetch_one(&state.pool)
  .await
  .unwrap();
  let admin_space = knowledge_server::tenancy::spaces::personal_space_id(&state.pool, &admin_id)
    .await
    .unwrap()
    .unwrap();

  let project_space = sqlx::query_scalar::<_, String>(
    "SELECT space_id FROM projects WHERE id = $1",
  )
  .bind(project_id)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(project_space, admin_space);

  let member_role = sqlx::query_scalar::<_, String>(
    "SELECT role FROM project_members WHERE project_id = $1 AND user_id = $2",
  )
  .bind(project_id)
  .bind(&admin_id)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(member_role, "owner");
}
