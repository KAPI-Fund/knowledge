mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use tower::util::ServiceExt;
use uuid::Uuid;

/// Log in the seeded admin; returns (full set-cookie value, csrf_token).
/// The server's `extract_session_cookie` splits on ';' and finds
/// `knowledge_session=`, so passing the whole set-cookie string as the COOKIE
/// header works (this matches `tenancy_access_api.rs`).
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
  let csrf = read_json(response.into_body()).await["csrfToken"]
    .as_str()
    .unwrap()
    .to_string();
  (cookie, csrf)
}

async fn read_json(body: Body) -> Value {
  let bytes = to_bytes(body, usize::MAX).await.unwrap();
  serde_json::from_slice(&bytes).unwrap()
}

/// Insert a user directly; returns the new user id.
async fn insert_user(pool: &sqlx::PgPool, username: &str) -> String {
  let id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO users (id, username, password_hash, role, created_at) \
     VALUES ($1, $2, 'x', 'user', '2026-06-19T00:00:00Z')",
  )
  .bind(&id)
  .bind(username)
  .execute(pool)
  .await
  .unwrap();
  id
}

/// Resolve the seeded admin's user id.
async fn admin_id(pool: &sqlx::PgPool) -> String {
  sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = 'admin'")
    .fetch_one(pool)
    .await
    .unwrap()
}

// --- direct-insert helpers (copied verbatim from tenancy_access_api.rs) ---
// Some are unused until later tasks; unused-fn warnings are harmless here.

#[allow(dead_code)]
async fn insert_org(pool: &sqlx::PgPool, created_by: &str, slug: &str) -> (String, String) {
  let org_id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO organizations (id, name, slug, created_by, created_at) \
     VALUES ($1, $2, $3, $4, '2026-01-01T00:00:00Z')",
  )
  .bind(&org_id)
  .bind(slug)
  .bind(slug)
  .bind(created_by)
  .execute(pool)
  .await
  .unwrap();
  let space_id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, created_at) \
     VALUES ($1, 'org', NULL, $2, '2026-01-01T00:00:00Z')",
  )
  .bind(&space_id)
  .bind(&org_id)
  .execute(pool)
  .await
  .unwrap();
  (org_id, space_id)
}

#[allow(dead_code)]
async fn insert_project_in_space(pool: &sqlx::PgPool, space_id: &str, name: &str) -> String {
  let id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO projects (id, name, root_path, space_id, created_at) \
     VALUES ($1, $2, $3, $4, '2026-01-01T00:00:00Z')",
  )
  .bind(&id)
  .bind(name)
  .bind(format!("/tmp/{id}"))
  .bind(space_id)
  .execute(pool)
  .await
  .unwrap();
  id
}

#[allow(dead_code)]
async fn add_org_member(pool: &sqlx::PgPool, org_id: &str, user_id: &str, role: &str) {
  sqlx::query(
    "INSERT INTO organization_members (id, org_id, user_id, role, created_at) \
     VALUES ($1, $2, $3, $4, '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(org_id)
  .bind(user_id)
  .bind(role)
  .execute(pool)
  .await
  .unwrap();
}

#[allow(dead_code)]
async fn grant_kb(pool: &sqlx::PgPool, project_id: &str, user_id: &str, role: &str) {
  sqlx::query(
    "INSERT INTO project_members (id, project_id, user_id, role, can_import, created_at) \
     VALUES ($1, $2, $3, $4, $5, '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(project_id)
  .bind(user_id)
  .bind(role)
  .bind(role != "viewer")
  .execute(pool)
  .await
  .unwrap();
}

#[allow(dead_code)]
async fn insert_team(pool: &sqlx::PgPool, org_id: &str, slug: &str) -> (String, String) {
  let team_id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO teams (id, org_id, name, slug, created_by, created_at) \
     SELECT $1, $2, $3, $3, om.user_id, '2026-01-01T00:00:00Z' \
     FROM organization_members om WHERE om.org_id = $2 AND om.role = 'org_admin' LIMIT 1",
  )
  .bind(&team_id)
  .bind(org_id)
  .bind(slug)
  .execute(pool)
  .await
  .unwrap();
  let space_id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, team_id, created_at) \
     VALUES ($1, 'team', NULL, NULL, $2, '2026-01-01T00:00:00Z')",
  )
  .bind(&space_id)
  .bind(&team_id)
  .execute(pool)
  .await
  .unwrap();
  (team_id, space_id)
}

#[allow(dead_code)]
async fn add_team_member(pool: &sqlx::PgPool, team_id: &str, user_id: &str, role: &str) {
  sqlx::query(
    "INSERT INTO team_members (id, team_id, user_id, role, created_at) \
     VALUES ($1, $2, $3, $4, '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(team_id)
  .bind(user_id)
  .bind(role)
  .execute(pool)
  .await
  .unwrap();
}

#[tokio::test]
async fn health_endpoint_is_reachable() {
  let env = TestEnvironment::start("team_endpoints_health").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let app = build_app(state.clone());
  let response = app
    .oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap())
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn create_org_inserts_org_admin_and_space() {
  let env = TestEnvironment::start("team_endpoints_create_org").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/orgs")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({ "name": "Acme", "slug": "acme" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::CREATED);
  let body = read_json(response.into_body()).await;
  let org_id = body["id"].as_str().unwrap();
  assert_eq!(body["slug"].as_str().unwrap(), "acme");
  assert!(body["spaceId"].as_str().is_some());

  // creator is org_admin
  let admin = admin_id(&state.pool).await;
  let role = sqlx::query_scalar::<_, String>(
    "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
  )
  .bind(org_id)
  .bind(&admin)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(role, "org_admin");

  // org space exists
  let space_count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM spaces WHERE kind = 'org' AND org_id = $1",
  )
  .bind(org_id)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(space_count, 1);
}

#[tokio::test]
async fn create_org_rejects_duplicate_slug() {
  let env = TestEnvironment::start("team_endpoints_dup_slug").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  insert_org(&state.pool, &admin, "taken").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/orgs")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "name": "Dup", "slug": "taken" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
