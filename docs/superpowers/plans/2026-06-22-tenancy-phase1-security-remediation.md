# Tenancy Phase 1 Security Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close six verified authorization holes in the tenancy backend (capability gating, operator-only instance admin, API-token scope enforcement, token-minting authorization, and a personal-space race) and add the missing negative integration tests.

**Architecture:** Phase 1 deliberately deferred per-endpoint capability gating, documenting it as "safe" only because no viewer/editor could exist yet. Phase 2/3 have since shipped org/team spaces and per-KB grants, so viewers and editors now exist in production data — invalidating that assumption. This plan introduces a single capability-enforcing entry point (`authorized_principal_with_role`) backed by an `AccessRole` privilege ordering, routes every mutating per-project handler through it, gates instance-admin endpoints behind a shared `require_operator` helper, makes the three project-scoped handlers that currently pass `None` enforce token scope, switches token minting to the canonical tenancy resolver, and makes personal-space creation idempotent under concurrency. Read/query/chat handlers are intentionally left as viewer-accessible per the design's capability matrix.

**Tech Stack:** Rust, Axum, SQLx (runtime string queries with `$1` binds), Postgres, Redis. Integration tests via `tower::ServiceExt::oneshot` against `build_app`, each on an isolated database (`tests/rust-integration`).

**Design decisions already settled (do not revisit):**
- **P2-E (org public KB):** RESOLVED by the product owner — org-space KBs remain **public-readable** to all org members (an org member with no grant resolves to `Viewer`). `tenancy/access.rs:61-76` is **correct and must NOT change**. Capability gating (this plan) is what makes that safe: public readers get `Viewer`, and `Viewer` cannot mutate.
- The Codex finding that `create_query_task` (`projects/routes.rs:992`) needs gating is **rejected**: `query` is viewer-allowed per the capability matrix (`docs/superpowers/specs/2026-06-18-saas-multitenancy-design.md:187-192`). It keeps using `authorized_principal`.

**Capability classification (authoritative for this plan):**
- **Viewer-allowed (keep `authorized_principal`)** — all GETs, plus: `search_handler`, `query_handler`, `create_query_task_handler`, chat routes, deep-research route.
- **Editor+ (must use `authorized_principal_with_role(.., AccessRole::Editor)`)** — the 17 handlers listed in Task 3.
- **Owner-or-manager (keep existing manual gate, only fix token scope)** — `delete_project_handler`, `upsert_grant_handler`, `delete_grant_handler`.

---

## File Structure

**Modified backend files:**
- `crates/knowledge-server/src/tenancy/access.rs` — add `AccessRole::satisfies` privilege ordering (+ inline unit test).
- `crates/knowledge-server/src/projects/routes.rs` — add `authorized_principal_with_role`; route 17 Editor+ handlers through it; fix token scope on `delete_project_handler`.
- `crates/knowledge-server/src/tenancy/grants.rs` — fix token scope on both grant handlers.
- `crates/knowledge-server/src/auth/operator.rs` — **new** shared `require_operator` helper.
- `crates/knowledge-server/src/auth/mod.rs` — register `pub mod operator;`.
- `crates/knowledge-server/src/settings/routes.rs` — gate both handlers on `require_operator`; delete the now-dead local `require_session`.
- `crates/knowledge-server/src/users/routes.rs` — gate `list_users` on `require_operator`.
- `crates/knowledge-server/src/auth/routes.rs` — token minting authorizes via `project_access_role`.
- `crates/knowledge-server/src/tenancy/spaces.rs` — `ensure_personal_space` uses `ON CONFLICT … DO NOTHING RETURNING`.

**New / modified test files:**
- `tests/rust-integration/tests/tenancy_capability_api.rs` — **new**: capability gating (P1-A), token scope (P1-C), token minting (P2-F).
- `tests/rust-integration/tests/operator_gating_api.rs` — **new**: operator-only settings/users (P1-B, P2-D).
- `tests/rust-integration/tests/tenancy_access_api.rs` — **modify**: add concurrent `ensure_personal_space` regression test (P2-G).

**Indentation convention (match the file you edit):**
- 4-space: `access.rs`, `projects/routes.rs`, `grants.rs`, `spaces.rs`, `api_tokens_api.rs`.
- 2-space: `auth/*.rs` (incl. new `operator.rs`), `settings/routes.rs`, `users/routes.rs`, `tenancy_access_api.rs` and the two new test files (match sibling tenancy tests).

**Test commands:**
- Backend unit: `cargo build -p knowledge-server && cargo test -p knowledge-server <filter>`
- Integration (one file): `cargo test -p rust-integration --test <name> -- --test-threads=1`
- Requires Docker Postgres on `127.0.0.1:55432` and Redis on `127.0.0.1:56379` (already running for this project). Seeded admin: `{username:"admin", password:"secret-password", role:"operator"}`.

---

### Task 1: AccessRole privilege ordering

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/access.rs` (add an `impl AccessRole` block after the enum at lines 5-13, and an inline `#[cfg(test)]` module at end of file)

- [ ] **Step 1: Write the failing test**

Append to the very end of `crates/knowledge-server/src/tenancy/access.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::AccessRole;

    #[test]
    fn satisfies_orders_viewer_editor_owner() {
        // Owner satisfies everything.
        assert!(AccessRole::Owner.satisfies(AccessRole::Owner));
        assert!(AccessRole::Owner.satisfies(AccessRole::Editor));
        assert!(AccessRole::Owner.satisfies(AccessRole::Viewer));
        // Editor satisfies Editor and Viewer, not Owner.
        assert!(AccessRole::Editor.satisfies(AccessRole::Editor));
        assert!(AccessRole::Editor.satisfies(AccessRole::Viewer));
        assert!(!AccessRole::Editor.satisfies(AccessRole::Owner));
        // Viewer satisfies only Viewer.
        assert!(AccessRole::Viewer.satisfies(AccessRole::Viewer));
        assert!(!AccessRole::Viewer.satisfies(AccessRole::Editor));
        assert!(!AccessRole::Viewer.satisfies(AccessRole::Owner));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server satisfies_orders_viewer_editor_owner`
Expected: FAIL — compile error `no method named 'satisfies' found for enum 'AccessRole'`.

- [ ] **Step 3: Write minimal implementation**

Insert this `impl` block immediately after the `AccessRole` enum definition (after line 13, before `project_access_role`):

```rust
impl AccessRole {
    /// Privilege rank for capability comparisons: `Viewer` < `Editor` < `Owner`.
    fn rank(self) -> u8 {
        match self {
            AccessRole::Viewer => 0,
            AccessRole::Editor => 1,
            AccessRole::Owner => 2,
        }
    }

    /// True when this role is at least as privileged as `required`.
    pub fn satisfies(self, required: AccessRole) -> bool {
        self.rank() >= required.rank()
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p knowledge-server satisfies_orders_viewer_editor_owner`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/access.rs
git commit -m "feat(tenancy): add AccessRole privilege ordering"
```

---

### Task 2: Capability gate + first Editor+ handler (reviews:sweep)

Introduce `authorized_principal_with_role` and route the simplest Editor+ handler (`sweep_reviews_handler`) through it. `reviews:sweep` only enqueues a task (no filesystem), so an editor's success path returns `202` cleanly — making it the reference case for the gate.

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs` (add helper next to `authorized_principal` at ~1642; change `sweep_reviews_handler` at ~1306)
- Test: `tests/rust-integration/tests/tenancy_capability_api.rs` (new file with shared helpers)

- [ ] **Step 1: Write the failing test (creates the new test file with its helper preamble)**

Create `tests/rust-integration/tests/tenancy_capability_api.rs`:

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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rust-integration --test tenancy_capability_api -- --test-threads=1`
Expected: `viewer_cannot_sweep_reviews_editor_can` FAILS (viewer currently gets `202`, not `403`). `viewer_can_create_query_task` PASSES already (guards against over-gating).

- [ ] **Step 3a: Add the capability-gate helper**

In `crates/knowledge-server/src/projects/routes.rs`, immediately after the `authorized_principal` function (the block ending at line 1668), add:

```rust
/// Like [`authorized_principal`] for a concrete project, but additionally
/// requires the caller's effective role to be at least `required`.
///
/// Read/query handlers keep using `authorized_principal` (viewer-accessible);
/// mutating handlers (import/edit-wiki/ingest/review/dedup/task-control) use
/// this so that a `Viewer` — including a public-org-KB reader — gets 403.
pub(crate) async fn authorized_principal_with_role(
    state: &AppState,
    headers: &HeaderMap,
    project_id: &str,
    required: crate::tenancy::access::AccessRole,
) -> Result<crate::auth::principal::Principal, ApiError> {
    let principal = crate::auth::principal::resolve_principal(state, headers).await?;
    if !principal.permits_project(project_id) {
        return Err(ApiError::forbidden(
            "api token is not scoped to this project",
        ));
    }
    let role =
        crate::tenancy::access::project_access_role(&state.pool, project_id, &principal.user_id)
            .await
            .map_err(ApiError::from)?;
    match role {
        Some(role) if role.satisfies(required) => Ok(principal),
        Some(_) => Err(ApiError::forbidden(
            "insufficient permissions for this project",
        )),
        None => Err(ApiError::forbidden("not a project member")),
    }
}
```

- [ ] **Step 3b: Route `sweep_reviews_handler` through the gate**

In `sweep_reviews_handler` (~line 1306), change the first line of the body:

```rust
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
```
to:
```rust
    let session = authorized_principal_with_role(
        &state,
        &headers,
        &project_id,
        crate::tenancy::access::AccessRole::Editor,
    )
    .await?;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo build -p knowledge-server && cargo test -p rust-integration --test tenancy_capability_api -- --test-threads=1`
Expected: both tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/projects/routes.rs tests/rust-integration/tests/tenancy_capability_api.rs
git commit -m "feat(tenancy): add capability gate and apply to reviews:sweep"
```

---

### Task 3: Route the remaining 16 Editor+ handlers through the gate

Apply the identical change to every other mutating per-project handler. Each handler's body currently begins with one of:

```rust
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
    // — or —
    let _session = authorized_principal(&state, &headers, Some(&project_id)).await?;
```

Replace it (preserving the `session` vs `_session` binding name) with:

```rust
    let session = authorized_principal_with_role(
        &state,
        &headers,
        &project_id,
        crate::tenancy::access::AccessRole::Editor,
    )
    .await?;
```

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs`

**Handlers to change (locate each by name; line numbers are pre-edit and will drift):**

| Handler | ~line | binding |
|---|---|---|
| `update_source_watch_handler` | 566 | `_session` |
| `scan_source_watch_handler` | 619 | `session` |
| `import_source_handler` | 647 | `session` |
| `rescan_sources_handler` | 693 | `session` |
| `delete_source_handler` | 736 | `session` |
| `save_file_content_handler` | 818 | `session` |
| `delete_wiki_pages_handler` | 852 | `session` |
| `retry_task_handler` | 971 | `session` |
| `cancel_task_handler` | 982 | `session` |
| `save_query_task_handler` | 1059 | `session` |
| `create_lint_task_handler` | 1150 | `session` |
| `ingest_handler` | 1206 | `session` |
| `update_review_handler` | 1350 | `session` |
| `detect_dedup_handler` | 1422 | `session` |
| `merge_dedup_group_handler` | 1466 | `session` |
| `dismiss_dedup_group_handler` | 1522 | `session` |

> NOTE: where the binding is `_session`, keep it `_session` in the replacement (change `let session =` to `let _session =`). `update_source_watch_handler` is the only `_session` case in this list.

> DO NOT change these viewer-allowed handlers (they keep `authorized_principal`): `search_handler` (896), `create_query_task_handler` (998), `query_handler`, `query_task_detail_handler`, `list_*`, `*_detail_handler`, `graph_*`, `tasks_handler`, `dedup_overview_handler` (1406), `audit_logs_handler` (1396), `get_source_watch_handler`, `file_content_handler`. And DO NOT change `delete_project_handler` here — it is handled in Task 4.

- [ ] **Step 1: Write the failing tests**

Append to `tests/rust-integration/tests/tenancy_capability_api.rs` (helpers already exist from Task 2):

```rust
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
        .body(Body::from(json!({}).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(ingest.status(), StatusCode::FORBIDDEN);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_capability_api viewer_cannot_edit_wiki -- --test-threads=1`
Expected: FAIL — viewer currently passes the gate, so `save_file_content_handler` proceeds to `project_root_for_id` and returns a non-403 status (e.g. `500`).

- [ ] **Step 3: Apply the 16 handler edits**

Make the replacement described above for all 16 handlers in the table. After editing, sanity-check the count:

Run: `rg -n "authorized_principal_with_role" crates/knowledge-server/src/projects/routes.rs | wc -l`
Expected: `17` (16 from this task + `sweep_reviews_handler` from Task 2).

Run: `rg -n "authorized_principal\(&state, &headers, Some\(&project_id\)\)" crates/knowledge-server/src/projects/routes.rs`
Expected: only the **viewer-allowed** read/query handlers remain (search, query, create_query_task, query_task_detail, list/detail/graph/tasks/dedup_overview/audit_logs/get_source_watch/file_content, project_detail, list_project_members, list_sources, list_files) — plus `delete_project_handler` is still `None` (fixed in Task 4). None of the 17 Editor+ handlers should appear.

- [ ] **Step 4: Run the full capability suite to verify it passes**

Run: `cargo build -p knowledge-server && cargo test -p rust-integration --test tenancy_capability_api -- --test-threads=1`
Expected: all capability tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/projects/routes.rs tests/rust-integration/tests/tenancy_capability_api.rs
git commit -m "fix(tenancy): require Editor for all mutating project endpoints"
```

---

### Task 4: Enforce API-token scope on delete-project and grants (P1-C)

`delete_project_handler` and both grant handlers operate on a concrete `{project_id}` but resolve with `authorized_principal(.., None)`, so `permits_project` is never checked: a token scoped to project B can delete or re-grant project A (its user owns A). Switch these three to pass `Some(&project_id)`, which enforces token scope while leaving their existing owner/manager authorization intact (an org-public-KB viewer still resolves to `Viewer`, which passes `authorized_principal`'s `is_none()` check but is then rejected by the handler's own `Owner`/`can_manage_kb_access` gate).

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs` (`delete_project_handler`, ~line 470)
- Modify: `crates/knowledge-server/src/tenancy/grants.rs` (`upsert_grant_handler` line 57, `delete_grant_handler` line 134)
- Test: `tests/rust-integration/tests/tenancy_capability_api.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/rust-integration/tests/tenancy_capability_api.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rust-integration --test tenancy_capability_api project_scoped_token_cannot -- --test-threads=1`
Expected: both FAIL — `delete` currently returns `204` (A deleted) and `grant` currently passes the scope check and returns `200`/a validation error other than `403`.

- [ ] **Step 3a: Fix `delete_project_handler`**

In `crates/knowledge-server/src/projects/routes.rs`, `delete_project_handler` (~line 470), change:

```rust
    let session = authorized_principal(&state, &headers, None).await?;
```
to:
```rust
    let session = authorized_principal(&state, &headers, Some(&project_id)).await?;
```

(Leave the rest of the handler — the `Owner`/`can_manage_kb_access` gate — unchanged.)

- [ ] **Step 3b: Fix both grant handlers**

In `crates/knowledge-server/src/tenancy/grants.rs`, change line 57 (`upsert_grant_handler`) and line 134 (`delete_grant_handler`), each from:

```rust
    let principal = authorized_principal(&state, &headers, None).await?;
```
to:
```rust
    let principal = authorized_principal(&state, &headers, Some(&project_id)).await?;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo build -p knowledge-server && cargo test -p rust-integration --test tenancy_capability_api -- --test-threads=1`
Expected: all capability tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/projects/routes.rs crates/knowledge-server/src/tenancy/grants.rs tests/rust-integration/tests/tenancy_capability_api.rs
git commit -m "fix(tenancy): enforce api-token scope on delete-project and grants"
```

---

### Task 5: Operator-only instance admin (`/api/system/settings`, `/api/users`) — P1-B, P2-D

Both endpoints currently accept any valid session. Introduce a shared `require_operator(state, headers)` helper (resolve session, then assert `users.role = 'operator'`) and gate both endpoints on it, removing the duplicated cookie-extraction in the process.

**Files:**
- Create: `crates/knowledge-server/src/auth/operator.rs`
- Modify: `crates/knowledge-server/src/auth/mod.rs`
- Modify: `crates/knowledge-server/src/settings/routes.rs`
- Modify: `crates/knowledge-server/src/users/routes.rs`
- Test: `tests/rust-integration/tests/operator_gating_api.rs` (new)

- [ ] **Step 1: Write the failing tests (new file)**

Create `tests/rust-integration/tests/operator_gating_api.rs`:

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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rust-integration --test operator_gating_api -- --test-threads=1`
Expected: both FAIL — the non-operator currently gets `200`, not `403`.

- [ ] **Step 3a: Create the `require_operator` helper**

Create `crates/knowledge-server/src/auth/operator.rs` (2-space indent):

```rust
use axum::http::HeaderMap;

use crate::app::state::AppState;
use crate::auth::principal::extract_session_cookie;
use crate::auth::session::{find_session, SessionRecord};
use crate::http::error::ApiError;

/// Resolve the caller's session and require `users.role = 'operator'`.
///
/// Instance-admin endpoints (`/api/system/settings`, `/api/users`) are
/// operator-only: an authenticated non-operator gets 403.
pub async fn require_operator(
  state: &AppState,
  headers: &HeaderMap,
) -> Result<SessionRecord, ApiError> {
  let session_id =
    extract_session_cookie(headers).ok_or_else(|| ApiError::unauthorized("missing session"))?;
  let session = find_session(state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;
  let role = sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE id = $1")
    .bind(&session.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::from)?;
  if role.as_deref() != Some("operator") {
    return Err(ApiError::forbidden("operator role required"));
  }
  Ok(session)
}
```

- [ ] **Step 3b: Register the module**

In `crates/knowledge-server/src/auth/mod.rs`, add `pub mod operator;` (keep alphabetical-ish order):

```rust
pub mod api_token;
pub mod operator;
pub mod password;
pub mod principal;
pub mod routes;
pub mod session;
```

- [ ] **Step 3c: Gate the settings handlers and delete the dead `require_session`**

In `crates/knowledge-server/src/settings/routes.rs`:

1. In `get_settings` (line 133), change:
   ```rust
     let _session = require_session(&state, &headers).await?;
   ```
   to:
   ```rust
     let _session = crate::auth::operator::require_operator(&state, &headers).await?;
   ```
2. In `update_settings` (line 142), change:
   ```rust
     let session = require_session(&state, &headers).await?;
   ```
   to:
   ```rust
     let session = crate::auth::operator::require_operator(&state, &headers).await?;
   ```
3. Delete the now-unused local `require_session` function (lines 199-218 inclusive).
4. Fix the now-unused imports at the top of the file:
   - Change `use axum::http::{header, HeaderMap};` to `use axum::http::HeaderMap;`
   - Delete the line `use crate::auth::session::{find_session, SessionRecord};`

- [ ] **Step 3d: Gate the users handler**

In `crates/knowledge-server/src/users/routes.rs`, replace the cookie-extraction block in `list_users` (lines 19-33) — from `let session_id = headers` through the `.ok_or_else(...)?;` that ends the `find_session` block — with a single line:

```rust
  let _session = crate::auth::operator::require_operator(&state, &headers).await?;
```

Then fix the now-unused imports at the top of the file:
- Change `use axum::http::{header, HeaderMap};` to `use axum::http::HeaderMap;`
- Delete the line `use crate::auth::session::find_session;`

The function should now read:

```rust
async fn list_users(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
  let _session = crate::auth::operator::require_operator(&state, &headers).await?;

  let rows = sqlx::query_as::<_, (String, String, String)>(
    "SELECT id, username, role FROM users ORDER BY created_at ASC",
  )
  .fetch_all(&state.pool)
  .await
  .map_err(ApiError::from)?;

  let users = rows
    .into_iter()
    .map(|(id, username, role)| json!({ "id": id, "username": username, "role": role }))
    .collect::<Vec<_>>();

  Ok(Json(json!({ "users": users })))
}
```

- [ ] **Step 4: Run tests to verify they pass (and nothing else broke)**

Run: `cargo build -p knowledge-server && cargo test -p rust-integration --test operator_gating_api -- --test-threads=1`
Expected: both PASS, no warnings about unused imports/functions.

Also run the existing settings suite to confirm the operator path still works there:
Run: `cargo test -p rust-integration --test review_settings_api -- --test-threads=1`
Expected: PASS (these log in as the seeded operator admin).

> If `review_settings_api` has any test that exercises settings as a **non-operator** user, that test now correctly expects `403`; update its assertion. Otherwise no change is needed.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/auth/operator.rs crates/knowledge-server/src/auth/mod.rs crates/knowledge-server/src/settings/routes.rs crates/knowledge-server/src/users/routes.rs tests/rust-integration/tests/operator_gating_api.rs
git commit -m "fix(auth): gate /system/settings and /users behind operator role"
```

---

### Task 6: Token minting authorizes via the tenancy resolver (P2-F)

Minting a project-scoped token currently requires an explicit `project_members` row, so org admins and space owners who have access through org/team membership (but no per-KB grant row) are wrongly rejected. Replace the raw `project_members` count with `project_access_role`.

**Files:**
- Modify: `crates/knowledge-server/src/auth/routes.rs` (`create_api_token_handler`, lines 195-207)
- Test: `tests/rust-integration/tests/tenancy_capability_api.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/rust-integration/tests/tenancy_capability_api.rs` (reuses helpers from Tasks 2 & 4):

```rust
#[tokio::test]
async fn org_admin_can_mint_token_for_org_kb_without_grant_row() {
  let env = TestEnvironment::start("mint-org-admin").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let pool = &state.pool;

  // Org admin has access via membership, but NO project_members row.
  let admin = insert_login_user(pool, "mint-admin", "pw").await;
  let (org_id, org_space) = insert_org(pool, &admin, "mint-org").await;
  let project_id = insert_project_in_space(pool, &org_space, "mint-kb").await;
  add_org_member(pool, &org_id, &admin, "org_admin").await;

  let (cookie, csrf) = login(&state, "mint-admin", "pw").await;
  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/users/me/api-tokens")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({ "name": "kb-token", "projectId": project_id }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn stranger_cannot_mint_token_for_org_kb() {
  let env = TestEnvironment::start("mint-stranger").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let pool = &state.pool;

  let admin = insert_login_user(pool, "mint-owner", "pw").await;
  let (org_id, org_space) = insert_org(pool, &admin, "mint-org2").await;
  let project_id = insert_project_in_space(pool, &org_space, "mint-kb2").await;
  add_org_member(pool, &org_id, &admin, "org_admin").await;
  // `stranger` has no membership/grant on this org KB.
  insert_login_user(pool, "mint-stranger", "pw").await;

  let (cookie, csrf) = login(&state, "mint-stranger", "pw").await;
  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/users/me/api-tokens")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(
          json!({ "name": "kb-token", "projectId": project_id }).to_string(),
        ))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
```

- [ ] **Step 2: Run tests to verify the first one fails**

Run: `cargo test -p rust-integration --test tenancy_capability_api mint -- --test-threads=1`
Expected: `org_admin_can_mint_token_for_org_kb_without_grant_row` FAILS (currently `403`, because the org admin has no `project_members` row). `stranger_cannot_mint_token_for_org_kb` already PASSES (guards the fix doesn't over-grant).

- [ ] **Step 3: Replace the membership check**

In `crates/knowledge-server/src/auth/routes.rs`, replace lines 195-207 (the `if let Some(project_id) = payload.project_id.as_deref() { … COUNT(*) FROM project_members … }` block) with:

```rust
  if let Some(project_id) = payload.project_id.as_deref() {
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo build -p knowledge-server && cargo test -p rust-integration --test tenancy_capability_api mint -- --test-threads=1`
Expected: both PASS.

Also re-run the existing token suite to confirm no regression:
Run: `cargo test -p rust-integration --test api_tokens_api -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/auth/routes.rs tests/rust-integration/tests/tenancy_capability_api.rs
git commit -m "fix(auth): authorize token minting via project_access_role"
```

---

### Task 7: Make `ensure_personal_space` race-safe (P2-G)

`ensure_personal_space` does a `SELECT` then an unconditional `INSERT`; two concurrent callers for the same user both miss the `SELECT` and the second `INSERT` violates the `spaces_personal_owner` partial unique index. Make the insert idempotent with `ON CONFLICT … DO NOTHING RETURNING`, re-reading the winner's row when the insert is a no-op.

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/spaces.rs` (`ensure_personal_space`, lines 8-28)
- Test: `tests/rust-integration/tests/tenancy_access_api.rs` (append)

- [ ] **Step 1: Write the failing test**

Append to `tests/rust-integration/tests/tenancy_access_api.rs` (it already `use`s `bootstrap_state`, `AppConfig`, `TestEnvironment`, `Uuid`):

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_access_api ensure_personal_space_is_concurrency_safe -- --test-threads=1`
Expected: FAIL — at least one task errors with a `spaces_personal_owner` unique violation (the `.expect(...)` panics).

> If the race does not trip on the first run (timing-dependent), it still demonstrates the gap; the fix below makes the test deterministically green regardless of scheduling.

- [ ] **Step 3: Make the insert idempotent**

In `crates/knowledge-server/src/tenancy/spaces.rs`, replace the body of `ensure_personal_space` (lines 8-28) with:

```rust
pub async fn ensure_personal_space(
    pool: &PgPool,
    user_id: &str,
    created_at: &str,
) -> Result<String, sqlx::Error> {
    if let Some(id) = personal_space_id(pool, user_id).await? {
        return Ok(id);
    }

    let id = Uuid::new_v4().to_string();
    let inserted = sqlx::query_scalar::<_, String>(
        "INSERT INTO spaces (id, kind, owner_user_id, org_id, created_at) \
         VALUES ($1, 'personal', $2, NULL, $3) \
         ON CONFLICT (owner_user_id) WHERE kind = 'personal' DO NOTHING \
         RETURNING id",
    )
    .bind(&id)
    .bind(user_id)
    .bind(created_at)
    .fetch_optional(pool)
    .await?;

    match inserted {
        Some(id) => Ok(id),
        // Lost the race against a concurrent writer; re-read the winner's row.
        None => Ok(personal_space_id(pool, user_id)
            .await?
            .expect("personal space exists after ON CONFLICT DO NOTHING")),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo build -p knowledge-server && cargo test -p rust-integration --test tenancy_access_api ensure_personal_space_is_concurrency_safe -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/spaces.rs tests/rust-integration/tests/tenancy_access_api.rs
git commit -m "fix(tenancy): make ensure_personal_space idempotent under concurrency"
```

---

### Task 8: Full-suite regression verification

Confirm the whole backend still builds clean and the broader tenancy/project/auth surfaces pass.

**Files:** none (verification only)

- [ ] **Step 1: Build with warnings as a check**

Run: `cargo build -p knowledge-server`
Expected: success, no unused-import/dead-code warnings from the files touched in Tasks 1-7.

- [ ] **Step 2: Run the security-relevant integration suites**

Run each (Postgres + Redis must be up):
```bash
cargo test -p rust-integration --test tenancy_capability_api -- --test-threads=1
cargo test -p rust-integration --test operator_gating_api -- --test-threads=1
cargo test -p rust-integration --test tenancy_access_api -- --test-threads=1
cargo test -p rust-integration --test tenancy_admin_api -- --test-threads=1
cargo test -p rust-integration --test api_tokens_api -- --test-threads=1
cargo test -p rust-integration --test project_api -- --test-threads=1
cargo test -p rust-integration --test review_settings_api -- --test-threads=1
cargo test -p rust-integration --test auth_api -- --test-threads=1
```
Expected: all PASS.

- [ ] **Step 3: Run the knowledge-server unit tests**

Run: `cargo test -p knowledge-server`
Expected: PASS (includes the `AccessRole::satisfies` test from Task 1).

- [ ] **Step 4: No commit** (verification only). If any suite fails, return to the owning task and fix before finishing.

---

## Self-Review

**Spec coverage** (against the 7 Codex findings + testing gaps):
- P1-A capability gating → Tasks 1-3 (helper + ordering + all 17 Editor+ handlers). `create_query_task` correctly excluded.
- P1-B operator-only settings → Task 5.
- P1-C token scope on `None` handlers → Task 4 (`delete_project`, both grants). `create_project`/`list_projects` correctly left as `None` (no project context).
- P2-D operator-only users → Task 5.
- P2-E org default Viewer → intentionally NOT changed (product decision); documented at top.
- P2-F token minting via resolver → Task 6.
- P2-G personal-space race → Task 7.
- Testing gaps (non-operator denial; viewer-denied mutating routes) → operator tests (Task 5), capability tests (Tasks 2-3), plus token-scope (Task 4), token-mint (Task 6), and race (Task 7).

**Placeholder scan:** No TBD/TODO; every code step shows complete code; every test step shows full test bodies; every command shows expected output.

**Type/name consistency:** `authorized_principal_with_role(state, headers, project_id, required)` defined in Task 2, used identically in Tasks 2-3. `AccessRole::satisfies` defined in Task 1, used in Task 2's helper. `require_operator(state, headers)` defined in Task 5, used in settings + users. Test helpers (`insert_login_user`, `login`, `insert_org`, `insert_project_in_space`, `add_org_member`, `grant_kb`) are defined once in `tenancy_capability_api.rs` (Task 2) and reused by Tasks 3, 4, 6; `operator_gating_api.rs` defines its own minimal `insert_login_user`/`login` (separate test binary, no cross-file sharing).

**Scope:** Single subsystem (tenancy authorization). No frontend changes — these are backend-only enforcement fixes; the admin UI already hides operator-only and Editor-only affordances, so no user-visible regression is expected for legitimate roles.
