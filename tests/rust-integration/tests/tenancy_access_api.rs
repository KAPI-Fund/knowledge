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

use knowledge_server::tenancy::access::{AccessRole, can_manage_kb_access, project_access_role};

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
  // Org-space KBs are public: an org member with no grant reads as viewer.
  assert_eq!(
    project_access_role(pool, &org_project, &ungranted).await.unwrap(),
    Some(AccessRole::Viewer)
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

#[tokio::test]
async fn team_access_role_matrix() {
  let env = TestEnvironment::start("team-access-matrix").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let pool = &state.pool;

  let admin = insert_user(pool, "tam-admin").await;
  let leader = insert_user(pool, "tam-leader").await;
  let granted_editor = insert_user(pool, "tam-editor").await;
  let granted_viewer = insert_user(pool, "tam-viewer").await;
  let no_grant = insert_user(pool, "tam-nogrant").await;
  let other_member = insert_user(pool, "tam-other").await;
  let stranger = insert_user(pool, "tam-stranger").await;

  let (org_id, _org_space) = insert_org(pool, &admin, "tam-org").await;
  add_org_member(pool, &org_id, &admin, "org_admin").await;
  add_org_member(pool, &org_id, &leader, "org_member").await;
  add_org_member(pool, &org_id, &granted_editor, "org_member").await;
  add_org_member(pool, &org_id, &granted_viewer, "org_member").await;
  add_org_member(pool, &org_id, &no_grant, "org_member").await;
  add_org_member(pool, &org_id, &other_member, "org_member").await;

  let (team_id, team_space) = insert_team(pool, &org_id, "team-a").await;
  let team_project = insert_project_in_space(pool, &team_space, "team-kb").await;

  add_team_member(pool, &team_id, &leader, "leader").await;
  add_team_member(pool, &team_id, &granted_editor, "member").await;
  add_team_member(pool, &team_id, &granted_viewer, "member").await;
  add_team_member(pool, &team_id, &no_grant, "member").await;
  grant_kb(pool, &team_project, &granted_editor, "editor").await;
  grant_kb(pool, &team_project, &granted_viewer, "viewer").await;

  // org_admin owns every org KB, including private team KBs.
  assert_eq!(
    project_access_role(pool, &team_project, &admin).await.unwrap(),
    Some(AccessRole::Owner)
  );
  // The team leader is editor by default.
  assert_eq!(
    project_access_role(pool, &team_project, &leader).await.unwrap(),
    Some(AccessRole::Editor)
  );
  // Granted members get exactly their grant.
  assert_eq!(
    project_access_role(pool, &team_project, &granted_editor).await.unwrap(),
    Some(AccessRole::Editor)
  );
  assert_eq!(
    project_access_role(pool, &team_project, &granted_viewer).await.unwrap(),
    Some(AccessRole::Viewer)
  );
  // A team member with no grant has no access.
  assert_eq!(
    project_access_role(pool, &team_project, &no_grant).await.unwrap(),
    None
  );
  // An org member who is not on the team has no access to a private team KB.
  assert_eq!(
    project_access_role(pool, &team_project, &other_member).await.unwrap(),
    None
  );
  // A non-member of the org has no access.
  assert_eq!(
    project_access_role(pool, &team_project, &stranger).await.unwrap(),
    None
  );
}

#[tokio::test]
async fn team_access_revoked_when_member_removed_with_stale_grant() {
  let env = TestEnvironment::start("team-access-revoke").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let pool = &state.pool;

  let admin = insert_user(pool, "rev-admin").await;
  let member = insert_user(pool, "rev-member").await;

  let (org_id, _org_space) = insert_org(pool, &admin, "rev-org").await;
  add_org_member(pool, &org_id, &admin, "org_admin").await;
  add_org_member(pool, &org_id, &member, "org_member").await;

  let (team_id, team_space) = insert_team(pool, &org_id, "rev-team").await;
  let team_project = insert_project_in_space(pool, &team_space, "rev-team-kb").await;

  // The member is on the team and holds an editor grant on the team KB.
  add_team_member(pool, &team_id, &member, "member").await;
  grant_kb(pool, &team_project, &member, "editor").await;
  assert_eq!(
    project_access_role(pool, &team_project, &member).await.unwrap(),
    Some(AccessRole::Editor),
    "a granted team member should have editor access"
  );

  // Remove the member from the team exactly as remove_team_member_handler does:
  // it deletes team_members but leaves the project_members grant in place.
  sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2")
    .bind(&team_id)
    .bind(&member)
    .execute(pool)
    .await
    .unwrap();

  // The stale grant must not keep granting access to a private team KB.
  assert_eq!(
    project_access_role(pool, &team_project, &member).await.unwrap(),
    None,
    "a removed team member must lose access despite a stale grant"
  );
}

#[tokio::test]
async fn can_manage_kb_access_matrix() {
  let env = TestEnvironment::start("team-manage-cap").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let pool = &state.pool;

  let admin = insert_user(pool, "cap-admin").await;
  let leader = insert_user(pool, "cap-leader").await;
  let other_leader = insert_user(pool, "cap-other-leader").await;
  let member = insert_user(pool, "cap-member").await;

  let (org_id, org_space) = insert_org(pool, &admin, "cap-org").await;
  add_org_member(pool, &org_id, &admin, "org_admin").await;
  add_org_member(pool, &org_id, &leader, "org_member").await;
  add_org_member(pool, &org_id, &other_leader, "org_member").await;
  add_org_member(pool, &org_id, &member, "org_member").await;

  let public_project = insert_project_in_space(pool, &org_space, "cap-public-kb").await;

  let (team_id, team_space) = insert_team(pool, &org_id, "cap-team").await;
  let team_project = insert_project_in_space(pool, &team_space, "cap-team-kb").await;
  add_team_member(pool, &team_id, &leader, "leader").await;
  add_team_member(pool, &team_id, &member, "member").await;
  grant_kb(pool, &team_project, &member, "editor").await;

  let (_other_team, other_team_space) = insert_team(pool, &org_id, "cap-other-team").await;
  let other_team_project = insert_project_in_space(pool, &other_team_space, "cap-other-kb").await;

  // org_admin can manage grants on every org KB.
  assert!(can_manage_kb_access(pool, &public_project, &admin).await.unwrap());
  assert!(can_manage_kb_access(pool, &team_project, &admin).await.unwrap());
  // A plain org member cannot manage a public KB's grants.
  assert!(!can_manage_kb_access(pool, &public_project, &member).await.unwrap());
  // The team leader manages their own team's KB.
  assert!(can_manage_kb_access(pool, &team_project, &leader).await.unwrap());
  // A granted (non-leader) member cannot manage grants.
  assert!(!can_manage_kb_access(pool, &team_project, &member).await.unwrap());
  // A leader of a different team cannot manage this team's KB.
  add_team_member(pool, &_other_team, &other_leader, "leader").await;
  assert!(!can_manage_kb_access(pool, &team_project, &other_leader).await.unwrap());
  assert!(can_manage_kb_access(pool, &other_team_project, &other_leader).await.unwrap());
}

#[tokio::test]
async fn ensure_personal_space_is_concurrency_safe() {
  let env = TestEnvironment::start("personal-space-race").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();

  // A fresh user with no personal space yet.
  let user_id = insert_user(&state.pool, "race-user").await;

  // Fire many concurrent ensure_personal_space calls for the same user.
  let mut handles = Vec::new();
  for _ in 0..16 {
    let pool = state.pool.clone();
    let uid = user_id.clone();
    handles.push(tokio::spawn(async move {
      knowledge_server::tenancy::spaces::ensure_personal_space(&pool, &uid, "2026-01-01T00:00:00Z")
        .await
    }));
  }

  let mut ids = Vec::new();
  for handle in handles {
    let result = handle.await.unwrap();
    // No call may error with a unique-violation.
    let id = result.expect("ensure_personal_space must not error under concurrency");
    ids.push(id);
  }

  // All callers observe exactly one shared personal space.
  let first = &ids[0];
  assert!(ids.iter().all(|id| id == first), "all calls must return the same space id");
  let count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM spaces WHERE kind = 'personal' AND owner_user_id = $1",
  )
  .bind(&user_id)
  .fetch_one(&state.pool)
  .await
  .unwrap();
  assert_eq!(count, 1);
}
