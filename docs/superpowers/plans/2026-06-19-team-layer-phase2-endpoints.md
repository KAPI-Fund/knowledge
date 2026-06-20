# Team Layer Phase 2 — Backend Endpoints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the HTTP endpoints for organizations, teams, members, per-KB grants, and space-scoped project CRUD that sit on top of the Phase 1 resolver (`tenancy::access`), plus Zod schemas and endpoint integration tests.

**Architecture:** New Axum sub-routers under `crates/knowledge-server/src/tenancy/` (`orgs.rs`, `teams.rs`, `grants.rs`, `spaces_api.rs`), each merged into the app via `http::router::build_router`. Endpoints reuse the existing `resolve_principal` / CSRF machinery and the Phase 1 `project_access_role` / `can_manage_kb_access` resolver. Project creation/listing is extended in `projects/` to be space-aware. All request/response contracts get Zod schemas in `packages/api-client`.

**Tech Stack:** Rust + Axum, SQLx runtime string queries (no `query!` macro), Postgres 16; TypeScript + Zod v4 + Vitest for the client schemas; `rust-integration` crate (Docker-backed Postgres/Redis) for endpoint tests.

---

## Background facts (verified against the codebase)

These are load-bearing facts the implementer must not re-derive:

- **No `query!` macro.** All SQL uses `sqlx::query`, `sqlx::query_as`, `sqlx::query_scalar` with runtime strings and `.bind(...)`. There is no `.sqlx` offline metadata; do not run `cargo sqlx prepare`.
- **Migrations are embedded** via `sqlx::migrate!("./migrations")`. Phase 1 migrations `0012_tenancy_foundation.sql` and `0013_team_layer.sql` are already merged. **This plan adds no migrations.**
- **`authorized_principal(state, headers, project_id: Option<&str>)`** and **`validate_csrf(headers, principal)`** are `pub(crate)` in `crates/knowledge-server/src/projects/routes.rs`. Reuse them from sibling modules via `crate::projects::routes::{authorized_principal, validate_csrf}`.
- **`resolve_principal(state, headers)`** lives in `crates/knowledge-server/src/auth/principal.rs`; returns `Principal { user_id, csrf_token: Option<String>, scope, token_id, project_id }`. `requires_csrf()` is true only for `AuthScope::Session`.
- **The resolver/predicates already exist** in `crates/knowledge-server/src/tenancy/access.rs`: `project_access_role(pool, project_id, user_id) -> Option<AccessRole>`, `can_manage_kb_access(pool, project_id, user_id) -> bool`, `is_org_admin` (private). `AccessRole` is `{ Owner, Editor, Viewer }`.
- **`AppState`** = `{ pool: PgPool, cache: CacheStore, project_root: String, session_ttl_hours: u64 }` (`app/state.rs`).
- **`ApiError`** (`http/error.rs`) has constructors `bad_request`, `forbidden`, `not_found`, `unauthorized`, `internal`, and `From<sqlx::Error>` → internal. `IntoResponse` renders `{ "error": message }`.
- **Login queries username only** (`auth/routes.rs`): `SELECT ... FROM users WHERE username = $1`. There is no email column. "Add member by usernameOrEmail" therefore resolves the user via `SELECT id FROM users WHERE username = $1`.
- **`build_router(state)`** (`http/router.rs`) merges module routers with `.merge(module::routes::router())`. New sub-routers must be merged here.
- **`tenancy` module** already has `pub mod access; pub mod spaces;` (see `tenancy/mod.rs`). New files are added as `pub mod` there.
- **Space creation today:** `tenancy/spaces.rs` `ensure_personal_space` inserts `(id, kind, owner_user_id, org_id, created_at)`. Org-space and team-space inserts must also set `org_id` / `team_id` respectively and leave the others NULL (enforced by `spaces_owner_check`).
- **Project creation today:** `projects/service.rs` `create_project` inserts into `projects (id, name, root_path, space_id, created_at)` then `project_members (id, project_id, user_id, role='owner', can_import=true, created_at)`. It currently hardcodes the creator's personal space via `ensure_personal_space`. This plan extends it to accept a target `spaceId`.
- **`list_projects_handler`** is currently unscoped: `SELECT id, name, root_path, created_at FROM projects ORDER BY created_at ASC`. This plan scopes it to the active space.
- **No `delete_project` handler exists.** `DELETE /api/projects/{id}` is net-new. All project child tables are `REFERENCES projects(id) ON DELETE CASCADE`, so `DELETE FROM projects WHERE id=$1` suffices.
- **ID format:** existing rows use UUID v4 strings (`uuid::Uuid::new_v4().to_string()`). New rows (orgs, teams, members, grants, spaces) use the same.
- **Timestamps:** RFC3339 strings via the `time` crate — `OffsetDateTime::now_utc().format(&Rfc3339)` (matches `created_at TEXT` columns). Do NOT use `chrono`; see "ID & slug conventions" for the exact call.
- **Integration tests** live in `tests/rust-integration/tests/`. Use `support::TestEnvironment::start("name")`, `AppConfig::for_tests`, `bootstrap_state`, `build_app(state).oneshot(...)`. Run with `-- --test-threads=1` to avoid concurrent `CREATE DATABASE` flakiness. The crate name is `rust-integration`.

## ID & slug conventions

- **IDs:** `uuid::Uuid::new_v4().to_string()` for every new row.
- **created_at:** match `projects/service.rs` exactly —
  `time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).map_err(|_| ApiError::internal("failed to format created_at"))?`.
  Do NOT use `chrono`; the crate uses the `time` crate.
- **Org/team slug validation:** lowercase `[a-z0-9-]`, 1–64 chars, no leading/trailing `-`. Implemented as a shared helper `fn validate_slug(s: &str) -> Result<(), ApiError>` in `tenancy/slug.rs`. Reused by org-create and team-create.

## Error contract

Every handler returns `Result<(StatusCode, Json<T>), ApiError>` (or `Result<Json<T>, ApiError>` for 200). Error bodies are `{ "error": "..." }`. Status mapping:

- Not authenticated → `ApiError::unauthorized` (401).
- Authenticated but not permitted → `ApiError::forbidden` (403).
- Missing/invalid CSRF on a session principal → `validate_csrf` returns
  `ApiError::unauthorized` (401). Reuse it as-is; do not re-map.
- Unknown org/team/project/user → `ApiError::not_found` (404).
- Bad slug / duplicate slug / bad role value / self-target where disallowed → `ApiError::bad_request` (400).

---

## File Structure

**New files (Rust):**
- `crates/knowledge-server/src/tenancy/slug.rs` — `validate_slug` helper.
- `crates/knowledge-server/src/tenancy/orgs.rs` — `POST /api/orgs`, `POST /api/orgs/{org}/members`, `DELETE /api/orgs/{org}/members/{userId}`; `router()`.
- `crates/knowledge-server/src/tenancy/teams.rs` — `POST /api/orgs/{org}/teams`, `GET /api/orgs/{org}/teams`, `POST /api/orgs/{org}/teams/{team}/members`, `DELETE /api/orgs/{org}/teams/{team}/members/{userId}`; `router()`.
- `crates/knowledge-server/src/tenancy/spaces_api.rs` — `GET /api/spaces`; `router()`.
- `crates/knowledge-server/src/tenancy/grants.rs` — `POST /api/projects/{id}/grants`, `DELETE /api/projects/{id}/grants/{userId}`; `router()`.

**Modified files (Rust):**
- `crates/knowledge-server/src/tenancy/mod.rs` — add `pub mod slug; pub mod orgs; pub mod teams; pub mod spaces_api; pub mod grants;`.
- `crates/knowledge-server/src/tenancy/spaces.rs` — add `create_org_space`, `create_team_space`, `space_id_for_org`, `space_kind` helpers.
- `crates/knowledge-server/src/http/router.rs` — merge the four new routers.
- `crates/knowledge-server/src/projects/service.rs` — extend `create_project` to accept `space_id` + capability gate.
- `crates/knowledge-server/src/projects/routes.rs` — extend `CreateProjectRequest` (add `spaceId`), extend `list_projects_handler` (space scoping via `?spaceId=`), add `delete_project_handler`.

**New files (TypeScript):**
- (none) — schemas appended to existing files.

**Modified files (TypeScript):**
- `packages/api-client/src/schemas.ts` — add org/team/member/grant/space/project Zod schemas + parse functions.
- `packages/api-client/src/schemas.test.ts` — add tests for each new schema.

**New files (tests):**
- `tests/rust-integration/tests/team_endpoints_api.rs` — endpoint integration tests for every new/extended route.

---

## Shared integration-test helpers

`tests/rust-integration/tests/team_endpoints_api.rs` reuses the harness patterns
from `tenancy_access_api.rs` and `project_api.rs`. **The file uses 2-space
indentation.** Put this preamble at the top of the test file (Task 0 creates it);
later tasks add `#[tokio::test]` functions below it.

```rust
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
```

> `users` columns confirmed against `migrations/0001_init.sql`:
> `(id, username, password_hash, role, created_at)` — all `TEXT NOT NULL`.
> `AppConfig::for_tests` takes **owned `String`s** (`env.database_url.clone()`),
> and `TestEnvironment::start(...)` returns a `Result` (call `.unwrap()`).

---

### Task 0: Test scaffold + verify users schema

**Files:**
- Create: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Create the test file with the shared preamble**

Paste the entire "Shared integration-test helpers" block above into the new file.
(`users` columns already confirmed: `id, username, password_hash, role, created_at`.)

- [ ] **Step 2: Add one smoke test that compiles and runs**

```rust
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
```

- [ ] **Step 3: Run it (Docker must be running)**

Run: `cargo test -p rust-integration --test team_endpoints_api -- --test-threads=1`
Expected: PASS (1 test). Confirms the harness wiring compiles.

- [ ] **Step 4: Commit**

```bash
git add tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "test(tenancy): scaffold team endpoints integration test file"
```

---

### Task 1: Slug validation helper

**Files:**
- Create: `crates/knowledge-server/src/tenancy/slug.rs`
- Modify: `crates/knowledge-server/src/tenancy/mod.rs`

- [ ] **Step 1: Register the module**

In `crates/knowledge-server/src/tenancy/mod.rs`, the file currently reads:

```rust
pub mod access;
pub mod spaces;
```

Change it to:

```rust
pub mod access;
pub mod grants;
pub mod orgs;
pub mod slug;
pub mod spaces;
pub mod spaces_api;
pub mod teams;
```

> The `grants`/`orgs`/`spaces_api`/`teams` modules don't exist yet; this step
> will not compile until Task 1 creates `slug.rs`. To keep the build green,
> add only `pub mod slug;` now and add the others in their respective tasks.
> Net result after Task 1: `mod.rs` has `access`, `slug`, `spaces`.

- [ ] **Step 2: Write `slug.rs` with a failing unit test**

```rust
use crate::http::error::ApiError;

/// Validate an org/team slug: lowercase ASCII letters/digits and single dashes,
/// 1-64 chars, no leading or trailing dash.
pub fn validate_slug(slug: &str) -> Result<(), ApiError> {
  let len = slug.len();
  if len == 0 || len > 64 {
    return Err(ApiError::bad_request("slug must be 1-64 characters"));
  }
  if slug.starts_with('-') || slug.ends_with('-') {
    return Err(ApiError::bad_request("slug must not start or end with '-'"));
  }
  if !slug
    .chars()
    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
  {
    return Err(ApiError::bad_request(
      "slug may only contain lowercase letters, digits, and '-'",
    ));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn accepts_valid_slug() {
    assert!(validate_slug("acme-corp-2").is_ok());
  }

  #[test]
  fn rejects_empty() {
    assert!(validate_slug("").is_err());
  }

  #[test]
  fn rejects_uppercase_and_spaces() {
    assert!(validate_slug("Acme Corp").is_err());
  }

  #[test]
  fn rejects_leading_and_trailing_dash() {
    assert!(validate_slug("-acme").is_err());
    assert!(validate_slug("acme-").is_err());
  }
}
```

- [ ] **Step 3: Run the unit tests**

Run: `cargo test -p knowledge-server tenancy::slug -- --nocapture`
Expected: PASS (4 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/tenancy/slug.rs crates/knowledge-server/src/tenancy/mod.rs
git commit -m "feat(tenancy): add slug validation helper"
```

---

### Task 2: Expose `is_org_admin` + add org-membership-role helper

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/access.rs`

`orgs.rs`/`teams.rs`/`grants.rs` need to check org membership. `is_org_admin`
already exists but is private; we also need "is this user a member at all, and
with what role".

- [ ] **Step 1: Make `is_org_admin` public**

In `access.rs`, change the signature:

```rust
async fn is_org_admin(pool: &PgPool, org_id: &str, user_id: &str) -> Result<bool, sqlx::Error> {
```

to:

```rust
pub async fn is_org_admin(pool: &PgPool, org_id: &str, user_id: &str) -> Result<bool, sqlx::Error> {
```

- [ ] **Step 2: Add `org_member_role` below `is_org_admin`**

```rust
/// The caller's role within an org (`org_admin` / `org_member`), or `None` if
/// they are not a member.
pub async fn org_member_role(
    pool: &PgPool,
    org_id: &str,
    user_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
    )
    .bind(org_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// The caller's role within a team (`leader` / `member`), or `None`.
pub async fn team_member_role(
    pool: &PgPool,
    team_id: &str,
    user_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}
```

- [ ] **Step 3: Build to confirm it compiles**

Run: `cargo build -p knowledge-server`
Expected: success (warnings about unused `org_member_role`/`team_member_role`
are acceptable until later tasks consume them).

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/tenancy/access.rs
git commit -m "feat(tenancy): expose org/team membership-role helpers"
```

---

### Task 3: `POST /api/orgs` (create organization)

Creates the `organizations` row + the creator's `organization_members(org_admin)`
row + the org's `spaces(kind='org')` row. Any authenticated session user.

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/spaces.rs` (add `create_org_space`)
- Create: `crates/knowledge-server/src/tenancy/orgs.rs`
- Modify: `crates/knowledge-server/src/tenancy/mod.rs` (add `pub mod orgs;`)
- Modify: `crates/knowledge-server/src/http/router.rs` (merge `tenancy::orgs::router()`)
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration test**

Add to the test file:

```rust
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
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rust-integration --test team_endpoints_api create_org -- --test-threads=1`
Expected: compile error / 404 (route not mounted yet).

- [ ] **Step 3: Add `create_org_space` to `spaces.rs`**

Append to `crates/knowledge-server/src/tenancy/spaces.rs`:

```rust
/// Create the `spaces(kind='org')` row for a freshly created org.
pub async fn create_org_space(
    pool: &PgPool,
    org_id: &str,
    created_at: &str,
) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO spaces (id, kind, owner_user_id, org_id, created_at) \
         VALUES ($1, 'org', NULL, $2, $3)",
    )
    .bind(&id)
    .bind(org_id)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(id)
}
```

- [ ] **Step 4: Create `crates/knowledge-server/src/tenancy/orgs.rs`**

```rust
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::tenancy::slug::validate_slug;
use crate::tenancy::spaces::create_org_space;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/orgs", post(create_org_handler))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateOrgRequest {
    name: String,
    slug: String,
}

async fn create_org_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateOrgRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    validate_slug(&payload.slug)?;

    let taken = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organizations WHERE slug = $1")
        .bind(&payload.slug)
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::from)?;
    if taken > 0 {
        return Err(ApiError::bad_request("slug already taken"));
    }

    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    let org_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO organizations (id, name, slug, created_by, created_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&org_id)
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    sqlx::query(
        "INSERT INTO organization_members (id, org_id, user_id, role, created_at) \
         VALUES ($1, $2, $3, 'org_admin', $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&org_id)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let space_id = create_org_space(&state.pool, &org_id, &created_at)
        .await
        .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": org_id,
            "name": payload.name,
            "slug": payload.slug,
            "spaceId": space_id,
        })),
    ))
}
```

- [ ] **Step 5: Register the module + merge the router**

In `crates/knowledge-server/src/tenancy/mod.rs` add `pub mod orgs;` (keep alphabetical):

```rust
pub mod access;
pub mod orgs;
pub mod slug;
pub mod spaces;
```

In `crates/knowledge-server/src/http/router.rs`, add `use crate::tenancy;` to the
imports and add `.merge(tenancy::orgs::router())` to the chain in `build_router`:

```rust
    .merge(projects::routes::router())
    .merge(settings::routes::router())
    .merge(tenancy::orgs::router())
    .merge(users::routes::router())
    .merge(web_search::routes::router())
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api create_org -- --test-threads=1`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/knowledge-server/src/tenancy/orgs.rs crates/knowledge-server/src/tenancy/spaces.rs crates/knowledge-server/src/tenancy/mod.rs crates/knowledge-server/src/http/router.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(tenancy): add POST /api/orgs"
```

---

### Task 4: `POST /api/orgs/{org}/members` (add existing user)

org_admin only. Resolves the target by username (login queries username only;
there is no email column). Inserts an `organization_members` grant.

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/orgs.rs`
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api add_org_member -- --test-threads=1`
Expected: FAIL (route not mounted).

- [ ] **Step 3: Add the handler + route to `orgs.rs`**

Add to the imports of `orgs.rs`:

```rust
use axum::extract::Path;
use axum::routing::{delete, post};
use crate::tenancy::access::is_org_admin;
```

(Replace the existing `use axum::routing::post;` line with the combined
`use axum::routing::{delete, post};`.)

Extend `router()`:

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/orgs", post(create_org_handler))
        .route("/api/orgs/{org_id}/members", post(add_org_member_handler))
        .route(
            "/api/orgs/{org_id}/members/{user_id}",
            delete(remove_org_member_handler),
        )
}
```

> `remove_org_member_handler` is added in Task 5. To keep this task compiling on
> its own, add a temporary stub now and replace it in Task 5, OR implement Task 5
> immediately after Step 3 here. Recommended: implement Task 5's handler body in
> the same edit so the file compiles once. The plan keeps them as separate tasks
> for review granularity; the executor may merge Steps.

Add the handler:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AddOrgMemberRequest {
    username_or_email: String,
    role: String,
}

async fn add_org_member_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
    Json(payload): Json<AddOrgMemberRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    if !is_org_admin(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::forbidden("only org admins may add members"));
    }
    if payload.role != "org_admin" && payload.role != "org_member" {
        return Err(ApiError::bad_request("role must be org_admin or org_member"));
    }

    let target = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
        .bind(&payload.username_or_email)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("user not found"))?;

    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM organization_members WHERE org_id = $1 AND user_id = $2",
    )
    .bind(&org_id)
    .bind(&target)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;
    if existing > 0 {
        return Err(ApiError::bad_request("user is already a member"));
    }

    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    sqlx::query(
        "INSERT INTO organization_members (id, org_id, user_id, role, created_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&org_id)
    .bind(&target)
    .bind(&payload.role)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "orgId": org_id, "userId": target, "role": payload.role })),
    ))
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api add_org_member -- --test-threads=1`
Expected: PASS (3 tests). (Requires Task 5's `remove_org_member_handler` to exist
for the file to compile — implement it now if you haven't.)

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/orgs.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(tenancy): add POST /api/orgs/{org}/members"
```

---

### Task 5: `DELETE /api/orgs/{org}/members/{userId}` (remove member)

org_admin only. Idempotent delete (deleting a non-member is a no-op 204).

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/orgs.rs`
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration test**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api remove_org_member -- --test-threads=1`
Expected: FAIL (route/handler missing) unless already implemented in Task 4.

- [ ] **Step 3: Add the handler to `orgs.rs`**

```rust
async fn remove_org_member_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((org_id, user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    if !is_org_admin(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::forbidden("only org admins may remove members"));
    }
    sqlx::query("DELETE FROM organization_members WHERE org_id = $1 AND user_id = $2")
        .bind(&org_id)
        .bind(&user_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
```

(The route was already added in Task 4 Step 3.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api _org_member -- --test-threads=1`
Expected: PASS (Task 4 + Task 5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/orgs.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(tenancy): add DELETE /api/orgs/{org}/members/{userId}"
```

---

### Task 6: `POST /api/orgs/{org}/teams` (create team)

Any org member may create a team and becomes its single `leader`. Creates the
`teams` row + `team_members(leader)` row + `spaces(kind='team')` row. Slug unique
within the org.

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/spaces.rs` (add `create_team_space`)
- Create: `crates/knowledge-server/src/tenancy/teams.rs`
- Modify: `crates/knowledge-server/src/tenancy/mod.rs` (add `pub mod teams;`)
- Modify: `crates/knowledge-server/src/http/router.rs` (merge `tenancy::teams::router()`)
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api create_team -- --test-threads=1`
Expected: FAIL (route not mounted).

- [ ] **Step 3: Add `create_team_space` to `spaces.rs`**

```rust
/// Create the `spaces(kind='team')` row for a freshly created team.
pub async fn create_team_space(
    pool: &PgPool,
    team_id: &str,
    created_at: &str,
) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO spaces (id, kind, owner_user_id, org_id, team_id, created_at) \
         VALUES ($1, 'team', NULL, NULL, $2, $3)",
    )
    .bind(&id)
    .bind(team_id)
    .bind(created_at)
    .execute(pool)
    .await?;
    Ok(id)
}
```

- [ ] **Step 4: Create `crates/knowledge-server/src/tenancy/teams.rs`**

```rust
use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::tenancy::access::org_member_role;
use crate::tenancy::slug::validate_slug;
use crate::tenancy::spaces::create_team_space;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/orgs/{org_id}/teams", post(create_team_handler))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateTeamRequest {
    name: String,
    slug: String,
}

async fn create_team_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
    Json(payload): Json<CreateTeamRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    if org_member_role(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
        .is_none()
    {
        return Err(ApiError::forbidden("not a member of this org"));
    }
    validate_slug(&payload.slug)?;

    let taken =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teams WHERE org_id = $1 AND slug = $2")
            .bind(&org_id)
            .bind(&payload.slug)
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::from)?;
    if taken > 0 {
        return Err(ApiError::bad_request("slug already taken in this org"));
    }

    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    let team_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO teams (id, org_id, name, slug, created_by, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&team_id)
    .bind(&org_id)
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    sqlx::query(
        "INSERT INTO team_members (id, team_id, user_id, role, created_at) \
         VALUES ($1, $2, $3, 'leader', $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&team_id)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let space_id = create_team_space(&state.pool, &team_id, &created_at)
        .await
        .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": team_id,
            "name": payload.name,
            "slug": payload.slug,
            "orgId": org_id,
            "spaceId": space_id,
        })),
    ))
}
```

- [ ] **Step 5: Register the module + merge the router**

`tenancy/mod.rs`:

```rust
pub mod access;
pub mod orgs;
pub mod slug;
pub mod spaces;
pub mod teams;
```

`http/router.rs` chain:

```rust
    .merge(tenancy::orgs::router())
    .merge(tenancy::teams::router())
    .merge(users::routes::router())
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api create_team -- --test-threads=1`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/knowledge-server/src/tenancy/teams.rs crates/knowledge-server/src/tenancy/spaces.rs crates/knowledge-server/src/tenancy/mod.rs crates/knowledge-server/src/http/router.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(tenancy): add POST /api/orgs/{org}/teams"
```

---

### Task 7: `GET /api/orgs/{org}/teams` (list visible teams)

org_admin sees all teams in the org; an org_member sees only the teams they
belong to. Read-only (no CSRF).

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/teams.rs`
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api list_teams -- --test-threads=1`
Expected: FAIL (GET route not mounted; only POST exists).

- [ ] **Step 3: Add the handler + extend the route in `teams.rs`**

Change the routing import:

```rust
use axum::routing::{get, post};
```

Change the route to add GET:

```rust
        .route(
            "/api/orgs/{org_id}/teams",
            get(list_teams_handler).post(create_team_handler),
        )
```

Add the handler:

```rust
async fn list_teams_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    let Some(role) = org_member_role(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    else {
        return Err(ApiError::forbidden("not a member of this org"));
    };

    let rows = if role == "org_admin" {
        sqlx::query_as::<_, (String, String, String)>(
            "SELECT id, name, slug FROM teams WHERE org_id = $1 ORDER BY created_at ASC",
        )
        .bind(&org_id)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::from)?
    } else {
        sqlx::query_as::<_, (String, String, String)>(
            "SELECT t.id, t.name, t.slug FROM teams t \
             JOIN team_members tm ON tm.team_id = t.id \
             WHERE t.org_id = $1 AND tm.user_id = $2 ORDER BY t.created_at ASC",
        )
        .bind(&org_id)
        .bind(&principal.user_id)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::from)?
    };

    let teams = rows
        .into_iter()
        .map(|(id, name, slug)| json!({ "id": id, "name": name, "slug": slug, "orgId": org_id }))
        .collect::<Vec<_>>();
    Ok(Json(json!({ "teams": teams })))
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api list_teams -- --test-threads=1`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/teams.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(tenancy): add GET /api/orgs/{org}/teams"
```

---

### Task 8: `POST /api/orgs/{org}/teams/{team}/members` (add team member)

org_admin or the team leader may add an **existing org member** as a team
`member`. (Single-leader model: this endpoint never promotes to leader.)

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/teams.rs`
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api add_team_member -- --test-threads=1`
Expected: FAIL (route not mounted).

- [ ] **Step 3: Add the handler + route in `teams.rs`**

Extend the access import:

```rust
use crate::tenancy::access::{is_org_admin, org_member_role, team_member_role};
```

Add the members route to `router()`:

```rust
        .route(
            "/api/orgs/{org_id}/teams/{team_id}/members",
            post(add_team_member_handler),
        )
```

> `remove_team_member_handler` (Task 9) shares this route via `.delete(...)`.
> Implement Task 9 in the same edit so the file compiles, or add a stub now.

Add the handler:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AddTeamMemberRequest {
    username_or_email: String,
}

async fn add_team_member_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((org_id, team_id)): Path<(String, String)>,
    Json(payload): Json<AddTeamMemberRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;

    let is_admin = is_org_admin(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?;
    let is_leader = team_member_role(&state.pool, &team_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
        .as_deref()
        == Some("leader");
    if !is_admin && !is_leader {
        return Err(ApiError::forbidden(
            "only org admins or the team leader may add members",
        ));
    }

    let belongs =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teams WHERE id = $1 AND org_id = $2")
            .bind(&team_id)
            .bind(&org_id)
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::from)?;
    if belongs == 0 {
        return Err(ApiError::not_found("team not found"));
    }

    let target = sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
        .bind(&payload.username_or_email)
        .fetch_optional(&state.pool)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("user not found"))?;

    if org_member_role(&state.pool, &org_id, &target)
        .await
        .map_err(ApiError::from)?
        .is_none()
    {
        return Err(ApiError::bad_request("user is not a member of the org"));
    }

    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM team_members WHERE team_id = $1 AND user_id = $2",
    )
    .bind(&team_id)
    .bind(&target)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;
    if existing > 0 {
        return Err(ApiError::bad_request("user is already a team member"));
    }

    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    sqlx::query(
        "INSERT INTO team_members (id, team_id, user_id, role, created_at) \
         VALUES ($1, $2, $3, 'member', $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&team_id)
    .bind(&target)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "teamId": team_id, "userId": target, "role": "member" })),
    ))
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api add_team_member -- --test-threads=1`
Expected: PASS (3 tests). (Requires Task 9's handler for the file to compile.)

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/teams.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(tenancy): add POST /api/orgs/{org}/teams/{team}/members"
```

---

### Task 9: `DELETE /api/orgs/{org}/teams/{team}/members/{userId}` (remove team member)

org_admin or leader. Refuses to remove the leader (single-leader model, no
transfer). Removing a non-member is an idempotent 204.

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/teams.rs`
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api remove_team_member -- --test-threads=1`
Expected: FAIL (route/handler missing) unless implemented alongside Task 8.

- [ ] **Step 3: Add the handler + route in `teams.rs`**

Change the routing import to include `delete`:

```rust
use axum::routing::{delete, get, post};
```

Add the route:

```rust
        .route(
            "/api/orgs/{org_id}/teams/{team_id}/members/{user_id}",
            delete(remove_team_member_handler),
        )
```

Add the handler:

```rust
async fn remove_team_member_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((org_id, team_id, user_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;

    let is_admin = is_org_admin(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?;
    let is_leader = team_member_role(&state.pool, &team_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
        .as_deref()
        == Some("leader");
    if !is_admin && !is_leader {
        return Err(ApiError::forbidden(
            "only org admins or the team leader may remove members",
        ));
    }

    let target_role = team_member_role(&state.pool, &team_id, &user_id)
        .await
        .map_err(ApiError::from)?;
    if target_role.as_deref() == Some("leader") {
        return Err(ApiError::bad_request("cannot remove the team leader"));
    }

    sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id = $2")
        .bind(&team_id)
        .bind(&user_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api team_member -- --test-threads=1`
Expected: PASS (Task 8 + Task 9 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/teams.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(tenancy): add DELETE /api/orgs/{org}/teams/{team}/members/{userId}"
```

---

### Task 10: `GET /api/spaces` (context switcher)

Returns the caller's available contexts: their personal space, each org they
belong to (with org role), and each team they belong to (with team role).
Read-only (no CSRF).

**Files:**
- Create: `crates/knowledge-server/src/tenancy/spaces_api.rs`
- Modify: `crates/knowledge-server/src/tenancy/mod.rs` (add `pub mod spaces_api;`)
- Modify: `crates/knowledge-server/src/http/router.rs` (merge `tenancy::spaces_api::router()`)
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration test**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api list_spaces -- --test-threads=1`
Expected: FAIL (route not mounted).

- [ ] **Step 3: Create `crates/knowledge-server/src/tenancy/spaces_api.rs`**

```rust
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::get;
use serde_json::json;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::routes::authorized_principal;
use crate::tenancy::spaces::personal_space_id;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/spaces", get(list_spaces_handler))
}

async fn list_spaces_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    let user_id = &principal.user_id;

    let personal = personal_space_id(&state.pool, user_id)
        .await
        .map_err(ApiError::from)?;

    let org_rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT o.id, o.slug, o.name, s.id, om.role \
         FROM organization_members om \
         JOIN organizations o ON o.id = om.org_id \
         JOIN spaces s ON s.kind = 'org' AND s.org_id = o.id \
         WHERE om.user_id = $1 \
         ORDER BY o.created_at ASC",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;
    let orgs = org_rows
        .into_iter()
        .map(|(id, slug, name, space_id, role)| {
            json!({ "id": id, "slug": slug, "name": name, "spaceId": space_id, "role": role })
        })
        .collect::<Vec<_>>();

    let team_rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT t.id, t.org_id, t.slug, t.name, s.id, tm.role \
         FROM team_members tm \
         JOIN teams t ON t.id = tm.team_id \
         JOIN spaces s ON s.kind = 'team' AND s.team_id = t.id \
         WHERE tm.user_id = $1 \
         ORDER BY t.created_at ASC",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;
    let teams = team_rows
        .into_iter()
        .map(|(id, org_id, slug, name, space_id, role)| {
            json!({
                "id": id,
                "orgId": org_id,
                "slug": slug,
                "name": name,
                "spaceId": space_id,
                "role": role,
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "personal": { "spaceId": personal },
        "orgs": orgs,
        "teams": teams,
    })))
}
```

- [ ] **Step 4: Register the module + merge the router**

`tenancy/mod.rs` (keep alphabetical):

```rust
pub mod access;
pub mod orgs;
pub mod slug;
pub mod spaces;
pub mod spaces_api;
pub mod teams;
```

`http/router.rs` chain (after the teams router):

```rust
    .merge(tenancy::orgs::router())
    .merge(tenancy::spaces_api::router())
    .merge(tenancy::teams::router())
    .merge(users::routes::router())
```

- [ ] **Step 5: Run the test**

Run: `cargo test -p rust-integration --test team_endpoints_api list_spaces -- --test-threads=1`
Expected: PASS (1 test).

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/tenancy/spaces_api.rs crates/knowledge-server/src/tenancy/mod.rs crates/knowledge-server/src/http/router.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(tenancy): add GET /api/spaces"
```

---

### Task 11: `POST /api/projects` — space-aware creation

Extend `CreateProjectRequest` with an optional `spaceId` and gate creation by the
target space's kind: personal → must be the caller's own personal space; org →
`org_admin`; team → leader/member of that team. Omitting `spaceId` keeps the
current behavior (creator's personal space).

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs` (extend `CreateProjectRequest`)
- Modify: `crates/knowledge-server/src/projects/service.rs` (add gate + `resolve_target_space`)
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api create_project_in -- --test-threads=1`
Expected: FAIL — `deny_unknown_fields` rejects `spaceId` (422) until the field is added.

- [ ] **Step 3: Add `spaceId` to `CreateProjectRequest`**

In `crates/knowledge-server/src/projects/routes.rs`, replace:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub name: String,
}
```

with:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub space_id: Option<String>,
}
```

- [ ] **Step 4: Rewrite `create_project` + add `resolve_target_space` in `service.rs`**

Replace the body of `create_project` (lines 23-91) so the space is resolved and
authorized **before** the filesystem is touched, and append the helper:

```rust
pub async fn create_project(
    input: &super::routes::CreateProjectRequest,
    state: &AppState,
    user_id: &str,
) -> Result<ProjectDto, ApiError> {
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;

    let space_id =
        resolve_target_space(state, user_id, input.space_id.as_deref(), &created_at).await?;

    let id = Uuid::new_v4().to_string();
    let root_path = PathBuf::from(&state.project_root).join(&id);
    let root =
        initialize_project(&root_path).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let root_path = normalize_project_path_string(root.as_path());

    sqlx::query(
        "INSERT INTO projects (id, name, root_path, space_id, created_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&root_path)
    .bind(&space_id)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    sqlx::query(
        "INSERT INTO project_members (id, project_id, user_id, role, can_import, created_at) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&id)
    .bind(user_id)
    .bind("owner")
    .bind(true)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    append_audit_log(
        state,
        CreateAuditLog {
            project_id: Some(id.clone()),
            actor_id: user_id.to_string(),
            action: "project.created".to_string(),
            target_type: "project".to_string(),
            target_id: id.clone(),
            task_id: None,
            summary: format!("Created project {}", input.name),
            metadata: json!({
              "name": input.name,
              "rootPath": root_path
            }),
        },
    )
    .await?;

    Ok(ProjectDto {
        id,
        name: input.name.clone(),
        root_path,
        created_at,
    })
}

/// Resolve the target space for a new project and authorize the caller to create
/// in it. `None` → the caller's personal space (auto-created if missing).
async fn resolve_target_space(
    state: &AppState,
    user_id: &str,
    space_id: Option<&str>,
    created_at: &str,
) -> Result<String, ApiError> {
    let Some(space_id) = space_id else {
        return crate::tenancy::spaces::ensure_personal_space(&state.pool, user_id, created_at)
            .await
            .map_err(ApiError::from);
    };
    let row = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>)>(
        "SELECT kind, owner_user_id, org_id, team_id FROM spaces WHERE id = $1",
    )
    .bind(space_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::from)?
    .ok_or_else(|| ApiError::not_found("space not found"))?;
    let (kind, owner_user_id, org_id, team_id) = row;
    match kind.as_str() {
        "personal" => {
            if owner_user_id.as_deref() != Some(user_id) {
                return Err(ApiError::forbidden("not your personal space"));
            }
        }
        "org" => {
            let org_id = org_id.ok_or_else(|| ApiError::internal("org space missing org_id"))?;
            if !crate::tenancy::access::is_org_admin(&state.pool, &org_id, user_id)
                .await
                .map_err(ApiError::from)?
            {
                return Err(ApiError::forbidden("only org admins may create public KBs"));
            }
        }
        "team" => {
            let team_id = team_id.ok_or_else(|| ApiError::internal("team space missing team_id"))?;
            if crate::tenancy::access::team_member_role(&state.pool, &team_id, user_id)
                .await
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::forbidden("not a member of this team"));
            }
        }
        _ => return Err(ApiError::bad_request("unknown space kind")),
    }
    Ok(space_id.to_string())
}
```

> No new imports are needed in `service.rs`: `OffsetDateTime`, `Rfc3339`,
> `PathBuf`, `Uuid`, `initialize_project`, `ApiError`, and the audit helpers are
> already imported; `crate::tenancy::access::*` is referenced by fully-qualified
> path.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api create_project_in -- --test-threads=1`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/projects/routes.rs crates/knowledge-server/src/projects/service.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(projects): space-aware project creation"
```

---

### Task 12: `GET /api/projects` — space-scoped listing

Scope the project list to an active space via `?spaceId=`. Personal context lists
the caller's personal KBs; an org context lists public org KBs **plus** team KBs
in that org the caller can access; a team context lists that team's KBs. Every
candidate is filtered through `project_access_role` (the single authority) and
annotated with `spaceKind`, optional `teamId`/`teamSlug`, and the resolved
`role`. Existing fields (`id`, `name`, `rootPath`, `createdAt`) are preserved.

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs` (`list_projects_handler`)
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api list_projects_ -- --test-threads=1`
Expected: FAIL — the current handler ignores `spaceId` and returns all projects
without `spaceKind`/`role`.

- [ ] **Step 3: Rewrite `list_projects_handler`**

In `crates/knowledge-server/src/projects/routes.rs`, replace the whole
`list_projects_handler` function (the current `(State, HeaderMap)` version) with:

```rust
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectsQuery {
    #[serde(default)]
    pub space_id: Option<String>,
}

async fn list_projects_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListProjectsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_principal(&state, &headers, None).await?;
    let user_id = session.user_id.clone();

    let space_id = match query.space_id {
        Some(id) => id,
        None => {
            match crate::tenancy::spaces::personal_space_id(&state.pool, &user_id)
                .await
                .map_err(ApiError::from)?
            {
                Some(id) => id,
                None => return Ok(Json(json!({ "projects": [] }))),
            }
        }
    };

    let space = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>)>(
        "SELECT kind, owner_user_id, org_id, team_id FROM spaces WHERE id = $1",
    )
    .bind(&space_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::from)?
    .ok_or_else(|| ApiError::not_found("space not found"))?;
    let (kind, owner_user_id, org_id, _team_id) = space;

    // candidate rows: (id, name, root_path, created_at, space_kind, team_id, team_slug)
    type Candidate = (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    );
    let candidates: Vec<Candidate> = match kind.as_str() {
        "personal" => {
            if owner_user_id.as_deref() != Some(user_id.as_str()) {
                return Err(ApiError::forbidden("not your personal space"));
            }
            sqlx::query_as::<_, (String, String, String, String)>(
                "SELECT id, name, root_path, created_at FROM projects \
                 WHERE space_id = $1 ORDER BY created_at ASC",
            )
            .bind(&space_id)
            .fetch_all(&state.pool)
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .map(|(id, name, rp, ca)| (id, name, rp, ca, "personal".to_string(), None, None))
            .collect()
        }
        "org" => {
            let org_id = org_id.ok_or_else(|| ApiError::internal("org space missing org_id"))?;
            if crate::tenancy::access::org_member_role(&state.pool, &org_id, &user_id)
                .await
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::forbidden("not a member of this org"));
            }
            sqlx::query_as::<_, Candidate>(
                "SELECT p.id, p.name, p.root_path, p.created_at, 'org', NULL, NULL \
                 FROM projects p JOIN spaces s ON s.id = p.space_id \
                 WHERE s.kind = 'org' AND s.org_id = $1 \
                 UNION ALL \
                 SELECT p.id, p.name, p.root_path, p.created_at, 'team', t.id, t.slug \
                 FROM projects p JOIN spaces s ON s.id = p.space_id \
                 JOIN teams t ON t.id = s.team_id \
                 WHERE s.kind = 'team' AND t.org_id = $1 \
                 ORDER BY 4 ASC",
            )
            .bind(&org_id)
            .fetch_all(&state.pool)
            .await
            .map_err(ApiError::from)?
        }
        "team" => {
            let team_id = _team_id.ok_or_else(|| ApiError::internal("team space missing team_id"))?;
            let owning_org =
                sqlx::query_scalar::<_, String>("SELECT org_id FROM teams WHERE id = $1")
                    .bind(&team_id)
                    .fetch_optional(&state.pool)
                    .await
                    .map_err(ApiError::from)?
                    .ok_or_else(|| ApiError::not_found("team not found"))?;
            if crate::tenancy::access::org_member_role(&state.pool, &owning_org, &user_id)
                .await
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::forbidden("not a member of this org"));
            }
            let slug = sqlx::query_scalar::<_, String>("SELECT slug FROM teams WHERE id = $1")
                .bind(&team_id)
                .fetch_one(&state.pool)
                .await
                .map_err(ApiError::from)?;
            sqlx::query_as::<_, (String, String, String, String)>(
                "SELECT id, name, root_path, created_at FROM projects \
                 WHERE space_id = $1 ORDER BY created_at ASC",
            )
            .bind(&space_id)
            .fetch_all(&state.pool)
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .map(|(id, name, rp, ca)| {
                (
                    id,
                    name,
                    rp,
                    ca,
                    "team".to_string(),
                    Some(team_id.clone()),
                    Some(slug.clone()),
                )
            })
            .collect()
        }
        _ => return Err(ApiError::bad_request("unknown space kind")),
    };

    let mut projects = Vec::new();
    for (id, name, root_path, created_at, space_kind, team_id, team_slug) in candidates {
        let Some(role) =
            crate::tenancy::access::project_access_role(&state.pool, &id, &user_id)
                .await
                .map_err(ApiError::from)?
        else {
            continue;
        };
        let role_str = match role {
            crate::tenancy::access::AccessRole::Owner => "owner",
            crate::tenancy::access::AccessRole::Editor => "editor",
            crate::tenancy::access::AccessRole::Viewer => "viewer",
        };
        projects.push(json!({
            "id": id,
            "name": name,
            "rootPath": normalize_project_path_string(std::path::Path::new(&root_path)),
            "createdAt": created_at,
            "spaceKind": space_kind,
            "teamId": team_id,
            "teamSlug": team_slug,
            "role": role_str,
        }));
    }

    Ok(Json(json!({ "projects": projects })))
}
```

> `Query` is already imported at the top of `routes.rs`
> (`use axum::extract::{Path, Query, State};`).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api list_projects_ -- --test-threads=1`
Expected: PASS (2 tests).

- [ ] **Step 5: Regression — confirm the existing personal-list test still passes**

Run: `cargo test -p rust-integration --test project_api -- --test-threads=1`
Expected: PASS (the default `?spaceId` omitted path returns the caller's personal
KBs with the original fields still present).

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/projects/routes.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(projects): space-scoped project listing"
```

---

### Task 13: `DELETE /api/projects/{id}` — delete a KB

Net-new handler. Allowed when the caller resolves to `Owner` (personal owner /
org_admin) **or** `can_manage_kb_access` is true (org_admin on public+team KBs,
team leader on their team's KBs). All child rows cascade via FK
`ON DELETE CASCADE`, so deleting the `projects` row suffices.

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs` (route + `delete_project_handler`)
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api delete_project -- --test-threads=1`
Expected: FAIL (no DELETE route; the path currently only has GET).

- [ ] **Step 3: Add the route + handler**

In `routes.rs`, change the project-detail route to add `.delete(...)`:

```rust
        .route(
            "/api/projects/{project_id}",
            get(project_detail_handler).delete(delete_project_handler),
        )
```

Add the handler (next to `project_detail_handler`):

```rust
async fn delete_project_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &session)?;

    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM projects WHERE id = $1")
        .bind(&project_id)
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::from)?;
    if exists == 0 {
        return Err(ApiError::not_found("project not found"));
    }

    let role = crate::tenancy::access::project_access_role(
        &state.pool,
        &project_id,
        &session.user_id,
    )
    .await
    .map_err(ApiError::from)?;
    let can_manage =
        crate::tenancy::access::can_manage_kb_access(&state.pool, &project_id, &session.user_id)
            .await
            .map_err(ApiError::from)?;
    if role != Some(crate::tenancy::access::AccessRole::Owner) && !can_manage {
        return Err(ApiError::forbidden("not permitted to delete this project"));
    }

    sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(&project_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api delete_project -- --test-threads=1`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/projects/routes.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(projects): add DELETE /api/projects/{id}"
```

---

### Task 14: `POST /api/projects/{id}/grants` — upsert a per-KB grant

New `grants.rs` sub-router. Body `{ userId, role }`, `role ∈ {editor, viewer}`.
Gated by `can_manage_kb_access`. The target must be a member of the owning org
(and, for a team KB, a member of the owning team). Upserts `project_members`;
`can_import` is derived (`editor → true`, `viewer → false`).

**Files:**
- Create: `crates/knowledge-server/src/tenancy/grants.rs`
- Modify: `crates/knowledge-server/src/tenancy/mod.rs` (add `pub mod grants;`)
- Modify: `crates/knowledge-server/src/http/router.rs` (merge `tenancy::grants::router()`)
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration tests**

```rust
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p rust-integration --test team_endpoints_api grant_kb -- --test-threads=1`
Expected: FAIL (route not mounted).

- [ ] **Step 3: Create `crates/knowledge-server/src/tenancy/grants.rs`**

```rust
use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::projects::routes::{authorized_principal, validate_csrf};
use crate::tenancy::access::{can_manage_kb_access, org_member_role, team_member_role};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects/{project_id}/grants", post(upsert_grant_handler))
        .route(
            "/api/projects/{project_id}/grants/{user_id}",
            axum::routing::delete(delete_grant_handler),
        )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpsertGrantRequest {
    user_id: String,
    role: String,
}

/// Look up the owning (kind, org_id, team_id) for a project's space.
async fn project_space(
    pool: &sqlx::PgPool,
    project_id: &str,
) -> Result<(String, Option<String>, Option<String>), ApiError> {
    sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT s.kind, s.org_id, s.team_id \
         FROM spaces s JOIN projects p ON p.space_id = s.id \
         WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)?
    .ok_or_else(|| ApiError::not_found("project not found"))
}

async fn upsert_grant_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(payload): Json<UpsertGrantRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;

    if payload.role != "editor" && payload.role != "viewer" {
        return Err(ApiError::bad_request("role must be editor or viewer"));
    }
    if !can_manage_kb_access(&state.pool, &project_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::forbidden("not permitted to manage access"));
    }

    let (kind, org_id, team_id) = project_space(&state.pool, &project_id).await?;
    let owning_org = match kind.as_str() {
        "org" => org_id.ok_or_else(|| ApiError::internal("org space missing org_id"))?,
        "team" => {
            let team_id = team_id.ok_or_else(|| ApiError::internal("team space missing team_id"))?;
            if team_member_role(&state.pool, &team_id, &payload.user_id)
                .await
                .map_err(ApiError::from)?
                .is_none()
            {
                return Err(ApiError::bad_request("user is not a member of the team"));
            }
            sqlx::query_scalar::<_, String>("SELECT org_id FROM teams WHERE id = $1")
                .bind(&team_id)
                .fetch_one(&state.pool)
                .await
                .map_err(ApiError::from)?
        }
        _ => return Err(ApiError::bad_request("cannot grant on this space")),
    };
    if org_member_role(&state.pool, &owning_org, &payload.user_id)
        .await
        .map_err(ApiError::from)?
        .is_none()
    {
        return Err(ApiError::bad_request("user is not a member of the org"));
    }

    let can_import = payload.role == "editor";
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    sqlx::query(
        "INSERT INTO project_members (id, project_id, user_id, role, can_import, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (project_id, user_id) \
         DO UPDATE SET role = EXCLUDED.role, can_import = EXCLUDED.can_import",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&project_id)
    .bind(&payload.user_id)
    .bind(&payload.role)
    .bind(can_import)
    .bind(&created_at)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "projectId": project_id,
            "userId": payload.user_id,
            "role": payload.role,
            "canImport": can_import,
        })),
    ))
}

async fn delete_grant_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    if !can_manage_kb_access(&state.pool, &project_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::forbidden("not permitted to manage access"));
    }
    sqlx::query("DELETE FROM project_members WHERE project_id = $1 AND user_id = $2")
        .bind(&project_id)
        .bind(&user_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
```

> Both handlers live in this file from the start (the DELETE route is exercised in
> Task 15). Implementing them together keeps `grants.rs` compiling in one edit.

- [ ] **Step 4: Register the module + merge the router**

`tenancy/mod.rs`:

```rust
pub mod access;
pub mod grants;
pub mod orgs;
pub mod slug;
pub mod spaces;
pub mod spaces_api;
pub mod teams;
```

`http/router.rs` chain:

```rust
    .merge(tenancy::grants::router())
    .merge(tenancy::orgs::router())
    .merge(tenancy::spaces_api::router())
    .merge(tenancy::teams::router())
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rust-integration --test team_endpoints_api grant_kb -- --test-threads=1`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/tenancy/grants.rs crates/knowledge-server/src/tenancy/mod.rs crates/knowledge-server/src/http/router.rs tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "feat(tenancy): add POST /api/projects/{id}/grants"
```

---

### Task 15: `DELETE /api/projects/{id}/grants/{userId}` — revoke a grant

Same gate as Task 14 (`can_manage_kb_access`). Idempotent delete (revoking a
non-existent grant is a 204). The route + handler were added in Task 14; this
task only adds the test.

**Files:**
- Test: `tests/rust-integration/tests/team_endpoints_api.rs`

- [ ] **Step 1: Write the failing integration test**

```rust
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
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p rust-integration --test team_endpoints_api revoke_kb_grant -- --test-threads=1`
Expected: PASS (route/handler already exist from Task 14).

- [ ] **Step 3: Commit**

```bash
git add tests/rust-integration/tests/team_endpoints_api.rs
git commit -m "test(tenancy): cover DELETE /api/projects/{id}/grants/{userId}"
```

---

### Task 16: Zod schemas for the new endpoints

Add request/response Zod schemas + parse helpers to the api-client package, with
unit tests. These mirror the JSON contracts above so the frontend (Plan 3) can
validate responses.

**Files:**
- Modify: `packages/api-client/src/schemas.ts`
- Modify: `packages/api-client/src/schemas.test.ts`

- [ ] **Step 1: Write the failing schema tests**

Append to `packages/api-client/src/schemas.test.ts`:

```ts
import {
  parseSpaceList,
  parseProjectList,
  parseTeamList,
  parseGrant,
} from "./schemas";

describe("parseSpaceList", () => {
  it("parses personal + orgs + teams", () => {
    const result = parseSpaceList({
      personal: { spaceId: "sp1" },
      orgs: [{ id: "o1", slug: "acme", name: "Acme", spaceId: "os1", role: "org_admin" }],
      teams: [
        { id: "t1", orgId: "o1", slug: "plat", name: "Platform", spaceId: "ts1", role: "leader" },
      ],
    });
    expect(result.personal.spaceId).toBe("sp1");
    expect(result.orgs[0].role).toBe("org_admin");
    expect(result.teams[0].orgId).toBe("o1");
  });

  it("allows a null personal spaceId", () => {
    const result = parseSpaceList({ personal: { spaceId: null }, orgs: [], teams: [] });
    expect(result.personal.spaceId).toBeNull();
  });
});

describe("parseProjectList", () => {
  it("parses space-scoped projects", () => {
    const result = parseProjectList({
      projects: [
        {
          id: "p1",
          name: "KB",
          rootPath: "/tmp/p1",
          createdAt: "2026-06-19T00:00:00Z",
          spaceKind: "team",
          teamId: "t1",
          teamSlug: "plat",
          role: "editor",
        },
      ],
    });
    expect(result.projects[0].spaceKind).toBe("team");
    expect(result.projects[0].role).toBe("editor");
  });

  it("allows null team fields for non-team KBs", () => {
    const result = parseProjectList({
      projects: [
        {
          id: "p1",
          name: "KB",
          rootPath: "/tmp/p1",
          createdAt: "2026-06-19T00:00:00Z",
          spaceKind: "org",
          teamId: null,
          teamSlug: null,
          role: "viewer",
        },
      ],
    });
    expect(result.projects[0].teamId).toBeNull();
  });
});

describe("parseTeamList", () => {
  it("parses teams", () => {
    const result = parseTeamList({
      teams: [{ id: "t1", name: "Platform", slug: "plat", orgId: "o1" }],
    });
    expect(result.teams[0].slug).toBe("plat");
  });
});

describe("parseGrant", () => {
  it("parses a grant response", () => {
    const result = parseGrant({
      projectId: "p1",
      userId: "u1",
      role: "editor",
      canImport: true,
    });
    expect(result.role).toBe("editor");
    expect(result.canImport).toBe(true);
  });

  it("rejects an invalid role", () => {
    expect(() => parseGrant({ projectId: "p1", userId: "u1", role: "owner", canImport: true }))
      .toThrow();
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `cd packages/api-client && npx vitest run src/schemas.test.ts`
Expected: FAIL (the new parse functions are not exported yet).

- [ ] **Step 3: Add the schemas + parse helpers**

Append to `packages/api-client/src/schemas.ts`:

```ts
export const spaceListSchema = z.object({
  personal: z.object({ spaceId: z.string().nullable() }),
  orgs: z.array(
    z.object({
      id: z.string(),
      slug: z.string(),
      name: z.string(),
      spaceId: z.string(),
      role: z.enum(["org_admin", "org_member"]),
    }),
  ),
  teams: z.array(
    z.object({
      id: z.string(),
      orgId: z.string(),
      slug: z.string(),
      name: z.string(),
      spaceId: z.string(),
      role: z.enum(["leader", "member"]),
    }),
  ),
});

export function parseSpaceList(input: unknown) {
  return spaceListSchema.parse(input);
}

export const scopedProjectSchema = z.object({
  id: z.string(),
  name: z.string(),
  rootPath: z.string(),
  createdAt: z.string(),
  spaceKind: z.enum(["personal", "org", "team"]),
  teamId: z.string().nullable(),
  teamSlug: z.string().nullable(),
  role: z.enum(["owner", "editor", "viewer"]),
});

export const projectListSchema = z.object({
  projects: z.array(scopedProjectSchema),
});

export function parseProjectList(input: unknown) {
  return projectListSchema.parse(input);
}

export const teamListSchema = z.object({
  teams: z.array(
    z.object({
      id: z.string(),
      name: z.string(),
      slug: z.string(),
      orgId: z.string(),
    }),
  ),
});

export function parseTeamList(input: unknown) {
  return teamListSchema.parse(input);
}

export const grantSchema = z.object({
  projectId: z.string(),
  userId: z.string(),
  role: z.enum(["editor", "viewer"]),
  canImport: z.boolean(),
});

export function parseGrant(input: unknown) {
  return grantSchema.parse(input);
}
```

- [ ] **Step 4: Run the tests**

Run: `cd packages/api-client && npx vitest run src/schemas.test.ts`
Expected: PASS (existing `parseCurrentUser` test + all new tests).

- [ ] **Step 5: Commit**

```bash
git add packages/api-client/src/schemas.ts packages/api-client/src/schemas.test.ts
git commit -m "feat(api-client): add zod schemas for tenancy endpoints"
```

---

### Task 17: Full-suite verification

A final gate that runs everything affected and confirms no regressions.

**Files:**
- (none — verification only)

- [ ] **Step 1: Run the full Rust integration suite (Docker required)**

Run: `cargo test -p rust-integration -- --test-threads=1`
Expected: PASS — the new `team_endpoints_api` file plus all existing suites
(`tenancy_access_api`, `project_api`, etc.).

- [ ] **Step 2: Run the server unit tests**

Run: `cargo test -p knowledge-server -- --test-threads=1`
Expected: PASS (includes `tenancy::slug` unit tests).

- [ ] **Step 3: Lint**

Run: `cargo clippy -p knowledge-server --all-targets -- -D warnings`
Expected: no warnings. (Fix any unused-import / dead-code findings introduced by
the new modules.)

- [ ] **Step 4: Run the api-client tests**

Run: `cd packages/api-client && npx vitest run`
Expected: PASS.

- [ ] **Step 5: Commit (if Step 3 required fixes)**

```bash
git add -A
git commit -m "chore(tenancy): clippy clean-up for phase 2 endpoints"
```

---

## Self-review notes (author)

- **Spec coverage:** every endpoint in the design's "Endpoints (plan 2)" section
  maps to a task — `POST /api/orgs` (T3), `GET /api/spaces` (T10),
  `POST/DELETE /api/orgs/{org}/members` (T4/T5), `POST/GET /api/orgs/{org}/teams`
  (T6/T7), `POST/DELETE /api/orgs/{org}/teams/{team}/members` (T8/T9),
  `POST /api/projects` extended (T11), `GET /api/projects` extended (T12),
  `POST/DELETE /api/projects/{id}/grants` (T14/T15), `DELETE /api/projects/{id}`
  re-gated (T13). Zod schemas (T16). The Phase 1 resolver change is already
  merged (Plan 1), so no resolver edits here beyond exposing helpers (T2).
- **Type consistency:** `validate_slug`, `is_org_admin`, `org_member_role`,
  `team_member_role`, `create_org_space`, `create_team_space`, `resolve_target_space`,
  `project_space`, `AccessRole`, and the `ListProjectsQuery`/`CreateProjectRequest`
  field names are used identically across the tasks that define and consume them.
- **No new migrations** — all tables/columns exist from `0012`/`0013`.
