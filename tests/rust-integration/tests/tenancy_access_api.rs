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

use knowledge_server::tenancy::access::{AccessRole, project_access_role};

/// Insert a bare user row and return its id.
async fn insert_user(pool: &sqlx::PgPool, username: &str) -> String {
  let id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO users (id, username, password_hash, role, created_at) \
     VALUES ($1, $2, 'x', 'user', '2026-01-01T00:00:00Z')",
  )
  .bind(&id)
  .bind(username)
  .execute(pool)
  .await
  .unwrap();
  id
}

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

#[tokio::test]
async fn access_role_matrix() {
  let env = TestEnvironment::start("tenancy-access-matrix").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let pool = &state.pool;

  let owner = insert_user(pool, "owner-user").await;
  let stranger = insert_user(pool, "stranger").await;
  let admin = insert_user(pool, "org-admin").await;
  let editor = insert_user(pool, "org-editor").await;
  let viewer = insert_user(pool, "org-viewer").await;
  let ungranted = insert_user(pool, "org-ungranted").await;

  // Personal space + project owned by `owner`.
  let owner_space =
    knowledge_server::tenancy::spaces::ensure_personal_space(pool, &owner, "2026-01-01T00:00:00Z")
      .await
      .unwrap();
  let personal_project = insert_project_in_space(pool, &owner_space, "personal-kb").await;

  // Org + org project, with members and grants.
  let (org_id, org_space) = insert_org(pool, &admin, "acme").await;
  let org_project = insert_project_in_space(pool, &org_space, "org-kb").await;
  add_org_member(pool, &org_id, &admin, "org_admin").await;
  add_org_member(pool, &org_id, &editor, "org_member").await;
  add_org_member(pool, &org_id, &viewer, "org_member").await;
  add_org_member(pool, &org_id, &ungranted, "org_member").await;
  grant_kb(pool, &org_project, &editor, "editor").await;
  grant_kb(pool, &org_project, &viewer, "viewer").await;

  // Personal space rules.
  assert_eq!(
    project_access_role(pool, &personal_project, &owner).await.unwrap(),
    Some(AccessRole::Owner)
  );
  assert_eq!(
    project_access_role(pool, &personal_project, &stranger).await.unwrap(),
    None
  );

  // Org space rules.
  assert_eq!(
    project_access_role(pool, &org_project, &admin).await.unwrap(),
    Some(AccessRole::Owner)
  );
  assert_eq!(
    project_access_role(pool, &org_project, &editor).await.unwrap(),
    Some(AccessRole::Editor)
  );
  assert_eq!(
    project_access_role(pool, &org_project, &viewer).await.unwrap(),
    Some(AccessRole::Viewer)
  );
  assert_eq!(
    project_access_role(pool, &org_project, &ungranted).await.unwrap(),
    None
  );
  assert_eq!(
    project_access_role(pool, &org_project, &stranger).await.unwrap(),
    None
  );
}

#[tokio::test]
async fn project_detail_requires_space_access() {
  let env = TestEnvironment::start("tenancy-http-access").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (admin_cookie, admin_csrf) = login_admin(&state).await;

  // Admin (owner via personal space) creates a project.
  let create = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &admin_cookie)
        .header("x-csrf-token", &admin_csrf)
        .body(Body::from(json!({ "name": "owned-kb" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(create.status(), StatusCode::CREATED);
  let bytes = to_bytes(create.into_body(), usize::MAX).await.unwrap();
  let project_id = serde_json::from_slice::<Value>(&bytes)
    .unwrap()
    .get("id")
    .and_then(Value::as_str)
    .unwrap()
    .to_string();

  // Owner can read it.
  let owner_view = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}"))
        .header(header::COOKIE, &admin_cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(owner_view.status(), StatusCode::OK);

  // A second registered user with no access is forbidden.
  let password_hash = knowledge_server::auth::password::hash_password("member-pw").unwrap();
  sqlx::query(
    "INSERT INTO users (id, username, password_hash, role, created_at) \
     VALUES ($1, 'outsider', $2, 'user', '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(&password_hash)
  .execute(&state.pool)
  .await
  .unwrap();

  let login = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
          json!({ "username": "outsider", "password": "member-pw" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(login.status(), StatusCode::OK);
  let outsider_cookie = login
    .headers()
    .get("set-cookie")
    .unwrap()
    .to_str()
    .unwrap()
    .to_string();

  let forbidden = build_app(state.clone())
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}"))
        .header(header::COOKIE, &outsider_cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn team_migration_creates_tables_and_constraints() {
  let env = TestEnvironment::start("team-migration").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let pool = &state.pool;

  assert!(table_exists(pool, "teams").await.unwrap());
  assert!(table_exists(pool, "team_members").await.unwrap());

  // A team space (kind='team', team_id set) is accepted.
  let admin_id = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = 'admin'")
    .fetch_one(pool)
    .await
    .unwrap();
  let (org_id, _org_space) = insert_org(pool, &admin_id, "team-mig-org").await;
  let team_id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO teams (id, org_id, name, slug, created_by, created_at) \
     VALUES ($1, $2, 'Team A', 'team-a', $3, '2026-01-01T00:00:00Z')",
  )
  .bind(&team_id)
  .bind(&org_id)
  .bind(&admin_id)
  .execute(pool)
  .await
  .unwrap();

  let team_space = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, team_id, created_at) \
     VALUES ($1, 'team', NULL, NULL, $2, '2026-01-01T00:00:00Z')",
  )
  .bind(&team_space)
  .bind(&team_id)
  .execute(pool)
  .await
  .unwrap();

  // The partial unique index forbids a second space for the same team.
  let duplicate = sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, team_id, created_at) \
     VALUES ($1, 'team', NULL, NULL, $2, '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(&team_id)
  .execute(pool)
  .await;
  assert!(duplicate.is_err(), "second space for a team must be rejected");

  // The owner-check rejects a malformed team space (team_id + org_id both set).
  let malformed = sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, team_id, created_at) \
     VALUES ($1, 'team', NULL, $2, $3, '2026-01-01T00:00:00Z')",
  )
  .bind(Uuid::new_v4().to_string())
  .bind(&org_id)
  .bind(&team_id)
  .execute(pool)
  .await;
  assert!(malformed.is_err(), "team space with org_id set must be rejected");
}
