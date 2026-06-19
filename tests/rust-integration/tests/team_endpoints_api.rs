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

#[tokio::test]
async fn add_org_member_succeeds_for_org_admin() {
  let env = TestEnvironment::start("team_endpoints_add_member").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  insert_user(&state.pool, "bob").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/members"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({ "usernameOrEmail": "bob", "role": "org_member" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::CREATED);

  let role = sqlx::query_scalar::<_, String>(
    "SELECT om.role FROM organization_members om JOIN users u ON u.id = om.user_id \
     WHERE om.org_id = $1 AND u.username = 'bob'",
  )
  .bind(&org_id)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(role, "org_member");
}

#[tokio::test]
async fn add_org_member_unknown_user_is_404() {
  let env = TestEnvironment::start("team_endpoints_add_member_404").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/members"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({ "usernameOrEmail": "ghost", "role": "org_member" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn add_org_member_requires_org_admin() {
  let env = TestEnvironment::start("team_endpoints_add_member_403").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  // org owned by someone other than the seeded admin; admin is not a member.
  let founder = insert_user(&state.pool, "founder").await;
  let (org_id, _space) = insert_org(&state.pool, &founder, "acme").await;
  add_org_member(&state.pool, &org_id, &founder, "org_admin").await;
  insert_user(&state.pool, "bob").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/members"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({ "usernameOrEmail": "bob", "role": "org_member" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn remove_org_member_succeeds_for_org_admin() {
  let env = TestEnvironment::start("team_endpoints_remove_member").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri(format!("/api/orgs/{org_id}/members/{bob}"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::NO_CONTENT);

  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM organization_members WHERE org_id = $1 AND user_id = $2",
  )
  .bind(&org_id)
  .bind(&bob)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(count, 0);
}

#[tokio::test]
async fn create_team_makes_creator_leader_and_team_space() {
  let env = TestEnvironment::start("team_endpoints_create_team").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_member").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/teams"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "name": "Platform", "slug": "platform" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::CREATED);
  let body = read_json(response.into_body()).await;
  let team_id = body["id"].as_str().unwrap();
  assert_eq!(body["orgId"].as_str().unwrap(), org_id);

  let role = sqlx::query_scalar::<_, String>(
    "SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2",
  )
  .bind(team_id)
  .bind(&admin)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(role, "leader");

  let space_count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM spaces WHERE kind = 'team' AND team_id = $1",
  )
  .bind(team_id)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(space_count, 1);
}

#[tokio::test]
async fn create_team_requires_org_membership() {
  let env = TestEnvironment::start("team_endpoints_create_team_403").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let founder = insert_user(&state.pool, "founder").await;
  let (org_id, _space) = insert_org(&state.pool, &founder, "acme").await;
  add_org_member(&state.pool, &org_id, &founder, "org_admin").await;
  let (cookie, csrf) = login_admin(&state).await; // admin is NOT a member

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/teams"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "name": "X", "slug": "x" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn list_teams_returns_all_for_org_admin() {
  let env = TestEnvironment::start("team_endpoints_list_teams_admin").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  insert_team(&state.pool, &org_id, "alpha").await;
  insert_team(&state.pool, &org_id, "beta").await;
  let (cookie, _csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri(format!("/api/orgs/{org_id}/teams"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  let body = read_json(response.into_body()).await;
  assert_eq!(body["teams"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn list_teams_scopes_to_membership_for_member() {
  let env = TestEnvironment::start("team_endpoints_list_teams_member").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let founder = insert_user(&state.pool, "founder").await;
  let (org_id, _space) = insert_org(&state.pool, &founder, "acme").await;
  add_org_member(&state.pool, &org_id, &founder, "org_admin").await;
  add_org_member(&state.pool, &org_id, &admin, "org_member").await;
  let (alpha_id, _a) = insert_team(&state.pool, &org_id, "alpha").await;
  insert_team(&state.pool, &org_id, "beta").await;
  add_team_member(&state.pool, &alpha_id, &admin, "member").await;
  let (cookie, _csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri(format!("/api/orgs/{org_id}/teams"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  let body = read_json(response.into_body()).await;
  let teams = body["teams"].as_array().unwrap();
  assert_eq!(teams.len(), 1);
  assert_eq!(teams[0]["slug"].as_str().unwrap(), "alpha");
}

#[tokio::test]
async fn add_team_member_succeeds_for_leader() {
  let env = TestEnvironment::start("team_endpoints_add_team_member").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let (team_id, _t) = insert_team(&state.pool, &org_id, "platform").await;
  add_team_member(&state.pool, &team_id, &admin, "leader").await;
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/teams/{team_id}/members"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "usernameOrEmail": "bob" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::CREATED);
  let role = sqlx::query_scalar::<_, String>(
    "SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2",
  )
  .bind(&team_id)
  .bind(&bob)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(role, "member");
}

#[tokio::test]
async fn add_team_member_rejects_non_org_member_target() {
  let env = TestEnvironment::start("team_endpoints_add_team_member_400").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let (team_id, _t) = insert_team(&state.pool, &org_id, "platform").await;
  add_team_member(&state.pool, &team_id, &admin, "leader").await;
  insert_user(&state.pool, "outsider").await; // not an org member
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/teams/{team_id}/members"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "usernameOrEmail": "outsider" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn add_team_member_requires_admin_or_leader() {
  let env = TestEnvironment::start("team_endpoints_add_team_member_403").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let founder = insert_user(&state.pool, "founder").await;
  let (org_id, _space) = insert_org(&state.pool, &founder, "acme").await;
  add_org_member(&state.pool, &org_id, &founder, "org_admin").await;
  add_org_member(&state.pool, &org_id, &admin, "org_member").await; // member, not leader
  let (team_id, _t) = insert_team(&state.pool, &org_id, "platform").await;
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/teams/{team_id}/members"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "usernameOrEmail": "bob" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn remove_team_member_succeeds_for_leader() {
  let env = TestEnvironment::start("team_endpoints_remove_team_member").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let (team_id, _t) = insert_team(&state.pool, &org_id, "platform").await;
  add_team_member(&state.pool, &team_id, &admin, "leader").await;
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;
  add_team_member(&state.pool, &team_id, &bob, "member").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri(format!("/api/orgs/{org_id}/teams/{team_id}/members/{bob}"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::NO_CONTENT);
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM team_members WHERE team_id = $1 AND user_id = $2",
  )
  .bind(&team_id)
  .bind(&bob)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(count, 0);
}

#[tokio::test]
async fn remove_team_member_cannot_remove_leader() {
  let env = TestEnvironment::start("team_endpoints_remove_leader_400").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let (team_id, _t) = insert_team(&state.pool, &org_id, "platform").await;
  add_team_member(&state.pool, &team_id, &admin, "leader").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri(format!("/api/orgs/{org_id}/teams/{team_id}/members/{admin}"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_spaces_returns_personal_orgs_and_teams() {
  let env = TestEnvironment::start("team_endpoints_list_spaces").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let (team_id, _t) = insert_team(&state.pool, &org_id, "platform").await;
  add_team_member(&state.pool, &team_id, &admin, "leader").await;
  let (cookie, _csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri("/api/spaces")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  let body = read_json(response.into_body()).await;

  assert!(body["personal"]["spaceId"].as_str().is_some());

  let orgs = body["orgs"].as_array().unwrap();
  assert_eq!(orgs.len(), 1);
  assert_eq!(orgs[0]["id"].as_str().unwrap(), org_id);
  assert_eq!(orgs[0]["role"].as_str().unwrap(), "org_admin");
  assert!(orgs[0]["spaceId"].as_str().is_some());

  let teams = body["teams"].as_array().unwrap();
  assert_eq!(teams.len(), 1);
  assert_eq!(teams[0]["id"].as_str().unwrap(), team_id);
  assert_eq!(teams[0]["orgId"].as_str().unwrap(), org_id);
  assert_eq!(teams[0]["role"].as_str().unwrap(), "leader");
  assert!(teams[0]["spaceId"].as_str().is_some());
}

#[tokio::test]
async fn create_project_in_org_space_requires_org_admin() {
  let env = TestEnvironment::start("team_endpoints_create_proj_org").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, org_space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({ "name": "Public KB", "spaceId": org_space }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::CREATED);
  let body = read_json(response.into_body()).await;
  let project_id = body["id"].as_str().unwrap();
  let space_id = sqlx::query_scalar::<_, String>("SELECT space_id FROM projects WHERE id = $1")
    .bind(project_id)
    .fetch_one(&state.pool)
    .await
    .unwrap();
  assert_eq!(space_id, org_space);
}

#[tokio::test]
async fn create_project_in_org_space_forbidden_for_member() {
  let env = TestEnvironment::start("team_endpoints_create_proj_org_403").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let founder = insert_user(&state.pool, "founder").await;
  let (org_id, org_space) = insert_org(&state.pool, &founder, "acme").await;
  add_org_member(&state.pool, &org_id, &founder, "org_admin").await;
  add_org_member(&state.pool, &org_id, &admin, "org_member").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "name": "X", "spaceId": org_space }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_project_in_team_space_allows_member() {
  let env = TestEnvironment::start("team_endpoints_create_proj_team").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_member").await;
  let (team_id, team_space) = insert_team(&state.pool, &org_id, "platform").await;
  add_team_member(&state.pool, &team_id, &admin, "member").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({ "name": "Team KB", "spaceId": team_space }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn list_projects_org_context_includes_public_and_team_kbs() {
  let env = TestEnvironment::start("team_endpoints_list_proj_org").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, org_space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  insert_project_in_space(&state.pool, &org_space, "public-kb").await;
  let (_team_id, team_space) = insert_team(&state.pool, &org_id, "platform").await;
  insert_project_in_space(&state.pool, &team_space, "team-kb").await;
  let (cookie, _csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri(format!("/api/projects?spaceId={org_space}"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  let body = read_json(response.into_body()).await;
  let projects = body["projects"].as_array().unwrap();
  // org_admin is Owner on both the public KB and the team KB.
  assert_eq!(projects.len(), 2);
  let kinds: Vec<&str> = projects
    .iter()
    .map(|p| p["spaceKind"].as_str().unwrap())
    .collect();
  assert!(kinds.contains(&"org"));
  assert!(kinds.contains(&"team"));
}

#[tokio::test]
async fn list_projects_member_sees_public_but_not_ungranted_team_kb() {
  let env = TestEnvironment::start("team_endpoints_list_proj_member").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let founder = insert_user(&state.pool, "founder").await;
  let (org_id, org_space) = insert_org(&state.pool, &founder, "acme").await;
  add_org_member(&state.pool, &org_id, &founder, "org_admin").await;
  add_org_member(&state.pool, &org_id, &admin, "org_member").await;
  insert_project_in_space(&state.pool, &org_space, "public-kb").await;
  let (_team_id, team_space) = insert_team(&state.pool, &org_id, "platform").await;
  insert_project_in_space(&state.pool, &team_space, "team-kb").await; // admin not in team
  let (cookie, _csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri(format!("/api/projects?spaceId={org_space}"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  let body = read_json(response.into_body()).await;
  let projects = body["projects"].as_array().unwrap();
  // org_member: Viewer on the public KB, no access to the team KB.
  assert_eq!(projects.len(), 1);
  assert_eq!(projects[0]["spaceKind"].as_str().unwrap(), "org");
  assert_eq!(projects[0]["role"].as_str().unwrap(), "viewer");
}

#[tokio::test]
async fn delete_project_succeeds_for_org_admin() {
  let env = TestEnvironment::start("team_endpoints_delete_proj").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, org_space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let project_id = insert_project_in_space(&state.pool, &org_space, "public-kb").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri(format!("/api/projects/{project_id}"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::NO_CONTENT);
  let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = $1")
    .bind(&project_id)
    .fetch_one(&state.pool)
    .await
    .unwrap();
  assert_eq!(count, 0);
}

#[tokio::test]
async fn delete_project_forbidden_for_viewer() {
  let env = TestEnvironment::start("team_endpoints_delete_proj_403").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let founder = insert_user(&state.pool, "founder").await;
  let (org_id, org_space) = insert_org(&state.pool, &founder, "acme").await;
  add_org_member(&state.pool, &org_id, &founder, "org_admin").await;
  add_org_member(&state.pool, &org_id, &admin, "org_member").await; // viewer on public KB
  let project_id = insert_project_in_space(&state.pool, &org_space, "public-kb").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri(format!("/api/projects/{project_id}"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn grant_kb_editor_to_org_member() {
  let env = TestEnvironment::start("team_endpoints_grant").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, org_space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;
  let project_id = insert_project_in_space(&state.pool, &org_space, "public-kb").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/grants"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "userId": bob, "role": "editor" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  let (role, can_import) = sqlx::query_as::<_, (String, bool)>(
    "SELECT role, can_import FROM project_members WHERE project_id = $1 AND user_id = $2",
  )
  .bind(&project_id)
  .bind(&bob)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(role, "editor");
  assert!(can_import);
}

#[tokio::test]
async fn grant_kb_rejects_non_org_member() {
  let env = TestEnvironment::start("team_endpoints_grant_400").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, org_space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let outsider = insert_user(&state.pool, "outsider").await; // not an org member
  let project_id = insert_project_in_space(&state.pool, &org_space, "public-kb").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/grants"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "userId": outsider, "role": "viewer" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn grant_kb_forbidden_for_non_manager() {
  let env = TestEnvironment::start("team_endpoints_grant_403").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let founder = insert_user(&state.pool, "founder").await;
  let (org_id, org_space) = insert_org(&state.pool, &founder, "acme").await;
  add_org_member(&state.pool, &org_id, &founder, "org_admin").await;
  add_org_member(&state.pool, &org_id, &admin, "org_member").await; // plain member
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;
  let project_id = insert_project_in_space(&state.pool, &org_space, "public-kb").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/grants"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "userId": bob, "role": "editor" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn revoke_kb_grant_succeeds_for_org_admin() {
  let env = TestEnvironment::start("team_endpoints_revoke").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, org_space) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let bob = insert_user(&state.pool, "bob").await;
  add_org_member(&state.pool, &org_id, &bob, "org_member").await;
  let project_id = insert_project_in_space(&state.pool, &org_space, "public-kb").await;
  grant_kb(&state.pool, &project_id, &bob, "editor").await;
  let (cookie, csrf) = login_admin(&state).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri(format!("/api/projects/{project_id}/grants/{bob}"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::NO_CONTENT);
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND user_id = $2",
  )
  .bind(&project_id)
  .bind(&bob)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(count, 0);
}
