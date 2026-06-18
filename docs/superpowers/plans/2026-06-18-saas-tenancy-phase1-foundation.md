# SaaS Tenancy — Phase 1: Backend Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce the tenancy data model (organizations, personal/org spaces) and a single space-based authorization path for knowledge bases, while keeping the existing single-admin deployment working.

**Architecture:** Add a `spaces` namespace table that owns every project (personal space or org space), plus `organizations` / `organization_members`. Replace the bare `project_members` membership count in `authorized_principal` with a centralized `project_access_role()` resolver that understands personal-owner, org-admin, and granted-org-member access. Project creation and admin seeding ensure a personal space exists and stamp `projects.space_id`.

**Tech Stack:** Rust, Axum, SQLx (runtime-checked string queries — NOT the `query!` macro, so no `.sqlx` metadata to regenerate), Postgres 16, `tests/rust-integration` (tower `oneshot` against a per-test database).

**Spec:** `docs/superpowers/specs/2026-06-18-saas-multitenancy-design.md`

---

## Phase boundary (what this plan is and is NOT)

**In scope (Phase 1):**
- Migration `0012` creating `organizations`, `organization_members`, `spaces`; adding `projects.space_id`; backfilling existing data; renaming `project_members.role` (`project_owner` → `owner`) and `users.role` (`admin` → `operator`).
- A `tenancy` module: `spaces` (ensure/lookup personal space) and `access` (`AccessRole` + `project_access_role`).
- `seed_admin_user` and `create_project` updated to guarantee a personal space and stamp `space_id`.
- `authorized_principal` rewritten to use `project_access_role` for allow/deny.

**Explicitly NOT in this phase (later plans):**
- `organization_settings` / `user_settings` tables and the space-aware provider settings resolver (the settings loader is read in 7 files — its own phase).
- HTTP endpoints for signup, org CRUD, membership, and per-KB grants (Phase 2).
- Per-endpoint **capability** gating (viewer cannot import/edit). Phase 1 centralizes and tests the *role resolver* and enforces allow/deny only. Because no production code path can create an org KB or a viewer grant until Phase 2/3, no viewer/editor exists in practice yet, so allow/deny is sufficient and safe for this phase. Capability gating lands with the grant endpoints.

This phase produces working, integration-testable backend software: existing single-admin behavior is preserved (admin's projects move into the admin's personal space), and the new access resolver is fully tested via direct DB seeding.

---

## File structure

| File | Responsibility | Action |
|------|----------------|--------|
| `crates/knowledge-server/migrations/0012_tenancy_foundation.sql` | New tables, `projects.space_id`, backfill, role renames | Create |
| `crates/knowledge-server/src/tenancy/mod.rs` | Module root, re-exports | Create |
| `crates/knowledge-server/src/tenancy/spaces.rs` | `ensure_personal_space`, personal-space lookup | Create |
| `crates/knowledge-server/src/tenancy/access.rs` | `AccessRole` enum, `project_access_role` resolver | Create |
| `crates/knowledge-server/src/lib.rs` | Add `pub mod tenancy;`; update `seed_admin_user` | Modify |
| `crates/knowledge-server/src/projects/service.rs` | `create_project` stamps `space_id`, role `owner` | Modify |
| `crates/knowledge-server/src/projects/routes.rs` | `authorized_principal` uses `project_access_role` | Modify (`1455-1482`) |
| `tests/rust-integration/tests/tenancy_access_api.rs` | Authorization matrix + migration/seed assertions | Create |

---

## Task 1: Migration 0012 — schema + backfill

**Files:**
- Create: `crates/knowledge-server/migrations/0012_tenancy_foundation.sql`
- Test: `tests/rust-integration/tests/tenancy_access_api.rs`

- [ ] **Step 1: Write the migration**

Create `crates/knowledge-server/migrations/0012_tenancy_foundation.sql`:

```sql
CREATE TABLE organizations (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE organization_members (
  id TEXT PRIMARY KEY NOT NULL,
  org_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('org_admin', 'org_member')),
  created_at TEXT NOT NULL,
  UNIQUE(org_id, user_id),
  FOREIGN KEY(org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE spaces (
  id TEXT PRIMARY KEY NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('personal', 'org')),
  owner_user_id TEXT,
  org_id TEXT,
  created_at TEXT NOT NULL,
  CHECK (
    (kind = 'personal' AND owner_user_id IS NOT NULL AND org_id IS NULL) OR
    (kind = 'org' AND org_id IS NOT NULL AND owner_user_id IS NULL)
  ),
  FOREIGN KEY(owner_user_id) REFERENCES users(id) ON DELETE CASCADE,
  FOREIGN KEY(org_id) REFERENCES organizations(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX spaces_personal_owner ON spaces(owner_user_id) WHERE kind = 'personal';
CREATE UNIQUE INDEX spaces_org ON spaces(org_id) WHERE kind = 'org';

ALTER TABLE projects ADD COLUMN space_id TEXT REFERENCES spaces(id);

-- Backfill: one personal space per existing user.
INSERT INTO spaces (id, kind, owner_user_id, org_id, created_at)
SELECT gen_random_uuid()::text, 'personal', u.id, NULL, u.created_at
FROM users u;

-- Assign each existing project to its owner's personal space.
UPDATE projects p
SET space_id = s.id
FROM project_members pm
JOIN spaces s ON s.owner_user_id = pm.user_id AND s.kind = 'personal'
WHERE pm.project_id = p.id AND pm.role = 'project_owner';

-- Any project still unassigned (no project_owner row) falls back to the
-- seeded admin's personal space.
UPDATE projects p
SET space_id = s.id
FROM users u
JOIN spaces s ON s.owner_user_id = u.id AND s.kind = 'personal'
WHERE p.space_id IS NULL AND u.username = 'admin';

-- Rename per-KB grant role and instance role.
UPDATE project_members SET role = 'owner' WHERE role = 'project_owner';
UPDATE users SET role = 'operator' WHERE role = 'admin';
UPDATE users SET role = 'user' WHERE role <> 'operator';

-- Every project now belongs to a space.
ALTER TABLE projects ALTER COLUMN space_id SET NOT NULL;
```

Note: `gen_random_uuid()` is built into Postgres 13+ core (the stack runs PG16), so no extension is required. On a fresh test database the `users` table is empty at migration time (the admin is seeded *after* migrations run), so all backfill statements are no-ops there and `projects` is empty when `SET NOT NULL` runs.

- [ ] **Step 2: Write the failing test**

Create `tests/rust-integration/tests/tenancy_access_api.rs` with this first test (and the shared imports):

```rust
mod support;

use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, table_exists};
use support::TestEnvironment;
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
```

This test depends on Task 3 (seed creates the admin's personal space). Until Task 3 lands, the `space_count == 1` assertion will fail.

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_access_api migration_creates_tenancy_tables_and_personal_space_uniqueness`
Expected: FAIL — tables exist (migration applied) but `space_count` is `0` because the seed does not yet create a personal space (added in Task 3).

- [ ] **Step 4: Commit the migration**

```bash
git add crates/knowledge-server/migrations/0012_tenancy_foundation.sql tests/rust-integration/tests/tenancy_access_api.rs
git commit -m "feat(tenancy): add organizations, spaces, and project ownership migration"
```

---

## Task 2: `tenancy::spaces` module — ensure personal space

**Files:**
- Create: `crates/knowledge-server/src/tenancy/mod.rs`
- Create: `crates/knowledge-server/src/tenancy/spaces.rs`
- Modify: `crates/knowledge-server/src/lib.rs` (add `pub mod tenancy;`)
- Test: `crates/knowledge-server/src/tenancy/spaces.rs` (logic exercised via Task 3/4 integration tests; this task adds the function + a doc-level unit check is not possible without a DB, so verification is via compile + the integration tests in later tasks)

- [ ] **Step 1: Create the module root**

Create `crates/knowledge-server/src/tenancy/mod.rs`:

```rust
pub mod access;
pub mod spaces;
```

- [ ] **Step 2: Implement `ensure_personal_space`**

Create `crates/knowledge-server/src/tenancy/spaces.rs`:

```rust
use sqlx::PgPool;
use uuid::Uuid;

/// Return the id of the user's personal space, creating it if absent.
///
/// The `spaces_personal_owner` partial unique index guarantees at most one
/// personal space per user; this function is the single writer.
pub async fn ensure_personal_space(
  pool: &PgPool,
  user_id: &str,
  created_at: &str,
) -> Result<String, sqlx::Error> {
  if let Some(id) = personal_space_id(pool, user_id).await? {
    return Ok(id);
  }

  let id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO spaces (id, kind, owner_user_id, org_id, created_at) \
     VALUES ($1, 'personal', $2, NULL, $3)",
  )
  .bind(&id)
  .bind(user_id)
  .bind(created_at)
  .execute(pool)
  .await?;
  Ok(id)
}

/// Look up the user's personal space id, if one exists.
pub async fn personal_space_id(
  pool: &PgPool,
  user_id: &str,
) -> Result<Option<String>, sqlx::Error> {
  sqlx::query_scalar::<_, String>(
    "SELECT id FROM spaces WHERE kind = 'personal' AND owner_user_id = $1",
  )
  .bind(user_id)
  .fetch_optional(pool)
  .await
}
```

- [ ] **Step 3: Register the module**

In `crates/knowledge-server/src/lib.rs`, add the module declaration alongside the other `pub mod` lines (e.g. directly after `pub mod auth;`):

```rust
pub mod tenancy;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p knowledge-server`
Expected: PASS (clean build; `access` module is created in Task 5 — to avoid a missing-module error, also create a stub now).

Create `crates/knowledge-server/src/tenancy/access.rs` as a temporary stub so `mod.rs` compiles:

```rust
// Filled in by Task 5.
```

Re-run: `cargo build -p knowledge-server`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/ crates/knowledge-server/src/lib.rs
git commit -m "feat(tenancy): add personal space helpers"
```

---

## Task 3: Seed admin with a personal space and operator role

**Files:**
- Modify: `crates/knowledge-server/src/lib.rs` (`seed_admin_user`, lines `132-167`)
- Test: `tests/rust-integration/tests/tenancy_access_api.rs`

- [ ] **Step 1: Update `seed_admin_user`**

In `crates/knowledge-server/src/lib.rs`, replace the body that inserts the admin user (current lines `149-166`) so it captures the new id, seeds role `operator`, and creates the personal space:

```rust
  let password_hash = crate::auth::password::hash_password(bootstrap_password)
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
  let now = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"));

  let admin_id = Uuid::new_v4().to_string();
  sqlx::query(
    "INSERT INTO users (id, username, password_hash, role, created_at)
     VALUES ($1, $2, $3, $4, $5)",
  )
  .bind(&admin_id)
  .bind("admin")
  .bind(password_hash)
  .bind("operator")
  .bind(&now)
  .execute(pool)
  .await?;

  crate::tenancy::spaces::ensure_personal_space(pool, &admin_id, &now).await?;

  Ok(())
```

Leave the early-return guards (`admin_exists`, missing `admin_password`) above unchanged.

- [ ] **Step 2: Add the failing test**

Append to `tests/rust-integration/tests/tenancy_access_api.rs`:

```rust
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
```

- [ ] **Step 3: Run the tests to verify the suite passes for seeding**

Run: `cargo test -p rust-integration --test tenancy_access_api seeded_admin_is_operator_with_personal_space`
Expected: PASS.

Run the Task 1 test again (now satisfied):
`cargo test -p rust-integration --test tenancy_access_api migration_creates_tenancy_tables_and_personal_space_uniqueness`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/lib.rs tests/rust-integration/tests/tenancy_access_api.rs
git commit -m "feat(tenancy): seed admin as operator with a personal space"
```

---

## Task 4: `create_project` assigns the creator's personal space

**Files:**
- Modify: `crates/knowledge-server/src/projects/service.rs` (`create_project`, lines `23-83`)
- Test: `tests/rust-integration/tests/tenancy_access_api.rs`

- [ ] **Step 1: Update `create_project`**

In `crates/knowledge-server/src/projects/service.rs`, change the project insert to include `space_id` and change the member role to `owner`. Replace the two `sqlx::query` blocks (lines `37-57`) with:

```rust
    let space_id =
        crate::tenancy::spaces::ensure_personal_space(&state.pool, user_id, &created_at)
            .await
            .map_err(ApiError::from)?;

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
```

- [ ] **Step 2: Add the failing test**

Append to `tests/rust-integration/tests/tenancy_access_api.rs` (add the new imports at the top of the file: `use axum::body::{Body, to_bytes}; use axum::http::{Request, StatusCode, header}; use knowledge_server::build_app; use serde_json::{json, Value}; use tower::util::ServiceExt;`):

```rust
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
```

Note: the login response field name for CSRF is read back from the JSON payload (`csrfToken`); if `build_app`/login returns it under a different key, adjust this single accessor. Verify against `crates/knowledge-server/src/auth` login handler output before running.

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_access_api created_project_belongs_to_owner_personal_space`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/projects/service.rs tests/rust-integration/tests/tenancy_access_api.rs
git commit -m "feat(tenancy): stamp created projects with the owner's personal space"
```

---

## Task 5: `tenancy::access` — `AccessRole` and `project_access_role`

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/access.rs` (replace the Task 2 stub)
- Test: `tests/rust-integration/tests/tenancy_access_api.rs`

- [ ] **Step 1: Implement the resolver**

Replace the contents of `crates/knowledge-server/src/tenancy/access.rs` with:

```rust
use sqlx::PgPool;

/// Effective access a user has to a project, resolved from space ownership,
/// org membership, and per-KB grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessRole {
  /// Personal-space owner or org admin: full control incl. managing access.
  Owner,
  /// Read + import + edit.
  Editor,
  /// Read-only (query/search/read wiki).
  Viewer,
}

/// Resolve the requesting user's effective role on a project, or `None` if the
/// user has no access (or the project does not exist).
///
/// Rules:
/// - personal space  -> the owner is `Owner`; anyone else has no access.
/// - org space       -> `org_admin` is `Owner`; an `org_member` has access only
///   via a `project_members` grant (`owner`/`editor` -> Editor, `viewer` ->
///   Viewer); a non-member has no access.
pub async fn project_access_role(
  pool: &PgPool,
  project_id: &str,
  user_id: &str,
) -> Result<Option<AccessRole>, sqlx::Error> {
  let space = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
    "SELECT s.kind, s.owner_user_id, s.org_id \
     FROM spaces s JOIN projects p ON p.space_id = s.id \
     WHERE p.id = $1",
  )
  .bind(project_id)
  .fetch_optional(pool)
  .await?;

  let Some((kind, owner_user_id, org_id)) = space else {
    return Ok(None);
  };

  match kind.as_str() {
    "personal" => {
      if owner_user_id.as_deref() == Some(user_id) {
        Ok(Some(AccessRole::Owner))
      } else {
        Ok(None)
      }
    }
    "org" => {
      let Some(org_id) = org_id else {
        return Ok(None);
      };
      let member_role = sqlx::query_scalar::<_, String>(
        "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
      )
      .bind(&org_id)
      .bind(user_id)
      .fetch_optional(pool)
      .await?;

      match member_role.as_deref() {
        None => Ok(None),
        Some("org_admin") => Ok(Some(AccessRole::Owner)),
        Some(_) => {
          let grant = sqlx::query_scalar::<_, String>(
            "SELECT role FROM project_members WHERE project_id = $1 AND user_id = $2",
          )
          .bind(project_id)
          .bind(user_id)
          .fetch_optional(pool)
          .await?;
          match grant.as_deref() {
            Some("owner") | Some("editor") => Ok(Some(AccessRole::Editor)),
            Some("viewer") => Ok(Some(AccessRole::Viewer)),
            _ => Ok(None),
          }
        }
      }
    }
    _ => Ok(None),
  }
}
```

- [ ] **Step 2: Add the failing matrix test**

Append to `tests/rust-integration/tests/tenancy_access_api.rs`:

```rust
use knowledge_server::tenancy::access::{project_access_role, AccessRole};

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
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_access_api access_role_matrix`
Expected: PASS — all six access scenarios resolve to the expected role.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/tenancy/access.rs tests/rust-integration/tests/tenancy_access_api.rs
git commit -m "feat(tenancy): add space-based project access resolver"
```

---

## Task 6: Route `authorized_principal` through the resolver

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs` (`authorized_principal`, lines `1455-1482`)
- Test: `tests/rust-integration/tests/tenancy_access_api.rs`

- [ ] **Step 1: Replace the membership count with the resolver**

In `crates/knowledge-server/src/projects/routes.rs`, replace the body of `authorized_principal` (the `if let Some(project_id) = project_id { ... }` block, lines `1462-1480`) with:

```rust
    if let Some(project_id) = project_id {
        if !principal.permits_project(project_id) {
            return Err(ApiError::forbidden(
                "api token is not scoped to this project",
            ));
        }
        let role = crate::tenancy::access::project_access_role(
            &state.pool,
            project_id,
            &principal.user_id,
        )
        .await
        .map_err(ApiError::from)?;

        if role.is_none() {
            return Err(ApiError::forbidden("not a project member"));
        }
    }
    Ok(principal)
```

The function signature and return type are unchanged, so all existing call sites (`chat/routes.rs`, `query_stream.rs`, `deep_research/routes.rs`, `projects/routes.rs`) keep compiling.

- [ ] **Step 2: Add the failing HTTP test**

Append to `tests/rust-integration/tests/tenancy_access_api.rs`:

```rust
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
```

Note: this assumes `GET /api/projects/{id}` is gated by `authorized_principal`. Confirm the route and exact path in `crates/knowledge-server/src/projects/routes.rs` before running; if the project-detail GET uses a different path, adjust the two `uri(...)` calls to a project-scoped endpoint that calls `authorized_principal` (e.g. the files-listing GET).

- [ ] **Step 3: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_access_api project_detail_requires_space_access`
Expected: PASS — owner gets `200`, outsider gets `403`.

- [ ] **Step 4: Run the full tenancy suite and clippy**

Run: `cargo test -p rust-integration --test tenancy_access_api`
Expected: all tests PASS.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/projects/routes.rs tests/rust-integration/tests/tenancy_access_api.rs
git commit -m "feat(tenancy): authorize project access via space-based resolver"
```

---

## Task 7: Full regression run

**Files:** none (verification only)

- [ ] **Step 1: Run the backend test suite**

Run: `cargo test -p knowledge-server`
Expected: PASS. If any test asserted the admin `role == "admin"` or relied on `project_members.role == "project_owner"`, update that assertion to the new values (`operator` / `owner`) — these are the only expected breakages from the rename.

- [ ] **Step 2: Run the integration suite**

Run: `cargo test -p rust-integration`
Expected: PASS (requires Postgres on `55432` and Redis on `56379`; start them with `npm run docker:up` for the DB/Redis services if not already running).

- [ ] **Step 3: Lint the whole workspace**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 4: Commit any test fixups**

```bash
git add -A
git commit -m "test(tenancy): align existing assertions with operator/owner roles"
```

(Skip this commit if Steps 1-3 produced no changes.)

---

## Self-Review

**1. Spec coverage (Phase 1 portion of `2026-06-18-saas-multitenancy-design.md`):**
- `organizations`, `organization_members`, `spaces` tables → Task 1. ✓
- `projects.space_id` + unified-spaces ownership → Task 1 (schema), Task 4 (new projects), Task 1 backfill (existing). ✓
- `project_members.role` rename `project_owner → owner` → Task 1 backfill + Task 4 (new inserts). ✓
- `users.role` rename `admin → operator` → Task 1 backfill + Task 3 (new seed). ✓
- Migration: personal space per existing user; existing projects → owner's (admin's) personal space → Task 1. ✓
- Access-control rules (personal owner; org_admin = owner; org_member needs grant; viewer/editor) → Task 5 resolver + Task 6 wiring. ✓
- Deferred by design (documented in "Phase boundary"): `organization_settings`/`user_settings` + settings resolver, signup/org/membership/grant endpoints, per-endpoint capability gating. These map to Phase 2/3 plans, not gaps.

**2. Placeholder scan:** No "TBD"/"handle edge cases"/"similar to" placeholders. The two "confirm the route/field before running" notes are concrete verification instructions with named fallbacks, not deferred work.

**3. Type/name consistency:**
- `ensure_personal_space(pool, user_id, created_at) -> Result<String, sqlx::Error>` defined in Task 2, called identically in Tasks 3, 4, 5. ✓
- `personal_space_id(pool, user_id) -> Result<Option<String>, sqlx::Error>` defined Task 2, used Tasks 3, 4. ✓
- `AccessRole { Owner, Editor, Viewer }` and `project_access_role(pool, project_id, user_id) -> Result<Option<AccessRole>, sqlx::Error>` defined Task 5, used Tasks 5 (tests) and 6 (route). ✓
- Role string literals consistent: org roles `org_admin`/`org_member`; per-KB grant roles `owner`/`editor`/`viewer`; instance role `operator`/`user`. ✓
- `space.kind` values `personal`/`org` consistent across migration, `spaces.rs`, `access.rs`. ✓
