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

/// Mint an unscoped API token (Bearer) for a user. Returns the plaintext token.
#[allow(dead_code)]
async fn mint_token(state: &knowledge_server::app::state::AppState, user_id: &str) -> String {
  knowledge_server::auth::api_token::create_api_token(
    state,
    knowledge_server::auth::api_token::CreateApiTokenInput {
      user_id,
      project_id: None,
      name: "test-token",
    },
  )
  .await
  .unwrap()
  .1
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
     SELECT $1, $2, $3, $3, o.created_by, '2026-01-01T00:00:00Z' \
     FROM organizations o WHERE o.id = $2",
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
async fn list_org_members_returns_members_with_usernames() {
  let env = TestEnvironment::start("list_org_members").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, _csrf) = login_admin(&state).await;
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri(format!("/api/orgs/{org_id}/members"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let body = read_json(response.into_body()).await;
  let members = body["members"].as_array().unwrap();
  assert_eq!(members.len(), 2);
  let usernames: Vec<&str> = members
    .iter()
    .map(|m| m["username"].as_str().unwrap())
    .collect();
  assert!(usernames.contains(&"admin"));
  assert!(usernames.contains(&"bob"));
}

#[tokio::test]
async fn list_org_members_forbidden_for_non_member() {
  let env = TestEnvironment::start("list_org_members_403").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, _csrf) = login_admin(&state).await;
  let admin = admin_id(&state.pool).await;
  let owner = insert_user(&state.pool, "owner").await;
  let (org_id, _space) = insert_org(&state.pool, &owner, "globex").await;
  add_org_member(&state.pool, &org_id, &owner, "org_admin").await;
  // admin (the logged-in caller) is NOT a member of this org.
  let _ = admin;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri(format!("/api/orgs/{org_id}/members"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn project_members_payload_includes_username() {
  let env = TestEnvironment::start("project_members_username")
    .await
    .unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, _csrf) = login_admin(&state).await;
  let admin = admin_id(&state.pool).await;
  let (org_id, ospace) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let project_id = insert_project_in_space(&state.pool, &ospace, "handbook").await;
  grant_kb(&state.pool, &project_id, &admin, "owner").await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri(format!("/api/projects/{project_id}/members"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let body = read_json(response.into_body()).await;
  let members = body["members"].as_array().unwrap();
  assert!(!members.is_empty());
  assert_eq!(members[0]["username"].as_str().unwrap(), "admin");
  assert!(members[0]["canImport"].as_bool().is_some());
}

#[tokio::test]
async fn patch_org_member_role_updates_role() {
  let env = TestEnvironment::start("patch_org_role").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_admin(&state).await;
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("PATCH")
        .uri(format!("/api/orgs/{org_id}/members/{bob}"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "role": "org_admin" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let body = read_json(response.into_body()).await;
  assert_eq!(body["role"].as_str().unwrap(), "org_admin");
  let stored = sqlx::query_scalar::<_, String>(
    "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
  )
  .bind(&org_id)
  .bind(&bob)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(stored, "org_admin");
}

#[tokio::test]
async fn patch_org_member_role_rejects_bad_role() {
  let env = TestEnvironment::start("patch_org_role_400").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_admin(&state).await;
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("PATCH")
        .uri(format!("/api/orgs/{org_id}/members/{bob}"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "role": "wizard" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn patch_org_member_role_forbidden_for_member() {
  let env = TestEnvironment::start("patch_org_role_403").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_admin(&state).await;
  let admin = admin_id(&state.pool).await;
  let owner = insert_user(&state.pool, "owner").await;
  let (org_id, _space) = insert_org(&state.pool, &owner, "globex").await;
  add_org_member(&state.pool, &org_id, &owner, "org_admin").await;
  // Logged-in admin is only an ordinary member here.
  add_org_member(&state.pool, &org_id, &admin, "org_member").await;
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("PATCH")
        .uri(format!("/api/orgs/{org_id}/members/{bob}"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "role": "org_admin" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn patch_org_member_role_missing_membership_404() {
  let env = TestEnvironment::start("patch_org_role_404").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_admin(&state).await;
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let ghost = Uuid::new_v4().to_string();

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("PATCH")
        .uri(format!("/api/orgs/{org_id}/members/{ghost}"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "role": "org_admin" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_team_members_returns_members() {
  let env = TestEnvironment::start("list_team_members").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, _csrf) = login_admin(&state).await;
  let admin = admin_id(&state.pool).await;
  let (org_id, _ospace) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let (team_id, _tspace) = insert_team(&state.pool, &org_id, "platform").await;
  let lead = insert_user(&state.pool, "lead").await;
  add_team_member(&state.pool, &team_id, &lead, "leader").await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri(format!("/api/orgs/{org_id}/teams/{team_id}/members"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::OK);
  let body = read_json(response.into_body()).await;
  let members = body["members"].as_array().unwrap();
  assert_eq!(members.len(), 1);
  assert_eq!(members[0]["username"].as_str().unwrap(), "lead");
  assert_eq!(members[0]["role"].as_str().unwrap(), "leader");
}

#[tokio::test]
async fn list_team_members_forbidden_for_outsider() {
  let env = TestEnvironment::start("list_team_members_403").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, _csrf) = login_admin(&state).await;
  let owner = insert_user(&state.pool, "owner").await;
  let (org_id, _ospace) = insert_org(&state.pool, &owner, "globex").await;
  add_org_member(&state.pool, &org_id, &owner, "org_admin").await;
  let (team_id, _tspace) = insert_team(&state.pool, &org_id, "platform").await;
  // Logged-in admin is neither org admin nor a team member here.

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri(format!("/api/orgs/{org_id}/teams/{team_id}/members"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_org_rejects_bearer_token() {
  let env = TestEnvironment::start("create_org_bearer").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let token = mint_token(&state, &admin).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/orgs")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Acme", "slug": "acme" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_org_duplicate_slug_returns_400() {
  let env = TestEnvironment::start("create_org_dup_slug").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_admin(&state).await;

  let first = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/orgs")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Acme", "slug": "dup" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(first.status(), StatusCode::CREATED);

  let second = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/orgs")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Acme Two", "slug": "dup" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(second.status(), StatusCode::BAD_REQUEST);
}
