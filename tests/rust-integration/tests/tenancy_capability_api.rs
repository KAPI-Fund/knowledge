mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use tower::util::ServiceExt;
use uuid::Uuid;

// ---------- shared helpers ----------

async fn read_json(body: Body) -> Value {
  let bytes = to_bytes(body, usize::MAX).await.unwrap();
  serde_json::from_slice(&bytes).unwrap()
}

/// Insert a user that can log in via password; returns its id.
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

/// Log in `username`; returns (full set-cookie value, csrf_token).
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

/// Build an org with one public KB, plus a logged-in viewer and editor.
/// Returns (project_id, viewer_(cookie,csrf), editor_(cookie,csrf)).
async fn org_kb_with_viewer_and_editor(
  state: &knowledge_server::app::state::AppState,
) -> (String, (String, String), (String, String)) {
  let pool = &state.pool;
  let admin = insert_login_user(pool, "cap-admin", "pw-admin").await;
  let viewer = insert_login_user(pool, "cap-viewer", "pw-viewer").await;
  let editor = insert_login_user(pool, "cap-editor", "pw-editor").await;
  let (org_id, org_space) = insert_org(pool, &admin, "cap-org").await;
  let project_id = insert_project_in_space(pool, &org_space, "cap-kb").await;
  add_org_member(pool, &org_id, &admin, "org_admin").await;
  add_org_member(pool, &org_id, &viewer, "org_member").await;
  add_org_member(pool, &org_id, &editor, "org_member").await;
  grant_kb(pool, &project_id, &viewer, "viewer").await;
  grant_kb(pool, &project_id, &editor, "editor").await;
  let viewer_auth = login(state, "cap-viewer", "pw-viewer").await;
  let editor_auth = login(state, "cap-editor", "pw-editor").await;
  (project_id, viewer_auth, editor_auth)
}

// ---------- tests ----------

#[tokio::test]
async fn viewer_cannot_sweep_reviews_editor_can() {
  let env = TestEnvironment::start("cap-sweep").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (project_id, (vcookie, vcsrf), (ecookie, ecsrf)) =
    org_kb_with_viewer_and_editor(&state).await;

  // Viewer is denied at the capability gate.
  let denied = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/reviews:sweep"))
        .header(header::COOKIE, &vcookie)
        .header("x-csrf-token", &vcsrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(denied.status(), StatusCode::FORBIDDEN);

  // Editor is allowed past the gate (task enqueued, no filesystem touched).
  let allowed = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/reviews:sweep"))
        .header(header::COOKIE, &ecookie)
        .header("x-csrf-token", &ecsrf)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(allowed.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn viewer_can_create_query_task() {
  let env = TestEnvironment::start("cap-query").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (project_id, (vcookie, vcsrf), _editor) =
    org_kb_with_viewer_and_editor(&state).await;

  // Querying is viewer-allowed per the capability matrix; it must NOT be gated.
  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query-tasks"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &vcookie)
        .header("x-csrf-token", &vcsrf)
        .body(Body::from(json!({ "query": "hello", "topK": 3 }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn viewer_cannot_edit_wiki() {
  let env = TestEnvironment::start("cap-edit-wiki").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (project_id, (vcookie, vcsrf), _editor) =
    org_kb_with_viewer_and_editor(&state).await;

  // PUT /files/content — editing wiki is Editor+; viewer must be blocked at the
  // gate, BEFORE any filesystem access.
  let save = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("PUT")
        .uri(format!("/api/projects/{project_id}/files/content"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &vcookie)
        .header("x-csrf-token", &vcsrf)
        .body(Body::from(
          json!({ "path": "wiki/x.md", "content": "hi" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(save.status(), StatusCode::FORBIDDEN);

  // POST /wiki-pages:delete — also Editor+.
  let delete = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/wiki-pages:delete"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &vcookie)
        .header("x-csrf-token", &vcsrf)
        .body(Body::from(json!({ "paths": ["wiki/x.md"] }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(delete.status(), StatusCode::FORBIDDEN);

  // POST /ingest — Editor+.
  let ingest = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/ingest"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &vcookie)
        .header("x-csrf-token", &vcsrf)
        .body(Body::from(json!({ "relativePath": "" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(ingest.status(), StatusCode::FORBIDDEN);
}

/// Helper: log in the seeded admin (operator, owns a personal space).
async fn login_admin(state: &knowledge_server::app::state::AppState) -> (String, String) {
  login(state, "admin", "secret-password").await
}

/// Helper: admin creates a personal-space project via HTTP, returns its id.
async fn create_personal_project(
  state: &knowledge_server::app::state::AppState,
  cookie: &str,
  csrf: &str,
  name: &str,
) -> String {
  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, cookie)
        .header("x-csrf-token", csrf)
        .body(Body::from(json!({ "name": name }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::CREATED);
  read_json(response.into_body()).await["id"]
    .as_str()
    .unwrap()
    .to_string()
}

/// Helper: mint a project-scoped API token for `project_id`, returns the secret.
async fn mint_scoped_token(
  state: &knowledge_server::app::state::AppState,
  cookie: &str,
  csrf: &str,
  project_id: &str,
) -> String {
  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/users/me/api-tokens")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, cookie)
        .header("x-csrf-token", csrf)
        .body(Body::from(
          json!({ "name": "scoped", "projectId": project_id }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::CREATED);
  read_json(response.into_body()).await["token"]
    .as_str()
    .unwrap()
    .to_string()
}

#[tokio::test]
async fn project_scoped_token_cannot_delete_other_project() {
  let env = TestEnvironment::start("scope-delete").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_admin(&state).await;
  let project_a = create_personal_project(&state, &cookie, &csrf, "scope-a").await;
  let project_b = create_personal_project(&state, &cookie, &csrf, "scope-b").await;
  let token_b = mint_scoped_token(&state, &cookie, &csrf, &project_b).await;

  // A token scoped to B must not be able to delete A, even though the token's
  // owner (admin) owns A.
  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("DELETE")
        .uri(format!("/api/projects/{project_a}"))
        .header("authorization", format!("Bearer {token_b}"))
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::FORBIDDEN);

  // And project A still exists.
  let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = $1")
    .bind(&project_a)
    .fetch_one(&state.pool)
    .await
    .unwrap();
  assert_eq!(exists, 1);
}

#[tokio::test]
async fn project_scoped_token_cannot_grant_on_other_project() {
  let env = TestEnvironment::start("scope-grant").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_admin(&state).await;
  let project_a = create_personal_project(&state, &cookie, &csrf, "grant-a").await;
  let project_b = create_personal_project(&state, &cookie, &csrf, "grant-b").await;
  let token_b = mint_scoped_token(&state, &cookie, &csrf, &project_b).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_a}/grants"))
        .header(header::CONTENT_TYPE, "application/json")
        .header("authorization", format!("Bearer {token_b}"))
        .body(Body::from(
          json!({ "userId": "whoever", "role": "viewer" }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
