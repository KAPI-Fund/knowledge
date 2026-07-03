# Team-Layer Review Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the verified P1/P2 findings from the team-layer code review — lock control-plane endpoints to session+CSRF only, repair the `space_id`/`spaceId` query-param mismatch, let an `org_admin` manage teams they are not a member of, and make org/team creation atomic — plus close the testing and documentation gaps.

**Architecture:** Backend control-plane handlers (`tenancy/*`, `create_project_handler`) stop accepting Bearer tokens by switching from `authorized_principal` to a new cookie-only `require_session` helper. Org/team creation becomes transactional with a Postgres `23505` → 400 mapping (new `ApiError::from_db_unique`), dropping the racy COUNT precheck. `list_teams_handler` returns `spaceId` + the caller's `role` so the admin UI can manage non-member teams. Frontend sources team `spaceId`/`role` from `useOrgTeamsQuery` (not `/api/spaces`) and fixes the projects query param.

**Tech Stack:** Rust + Axum + SQLx (Postgres), `knowledge-server` crate; React + TypeScript + tanstack-query + Zod + vitest (`apps/admin`, `packages/api-client`); integration tests in `rust-integration`.

**Product decision honored (Open Question):** "leader + member 都可" — any team member (leader OR member) may create a KB in their own team. Backend code and the endpoint description stay as-is; only the capability-table row in the design doc is corrected for consistency (Task 9). No backend authz change for KB creation.

**Conventions:** `tenancy/*.rs`, `projects/routes.rs`, `projects/service.rs` use **4-space** indentation; `auth/*.rs`, `http/error.rs` use **2-space**; all TypeScript uses **2-space**. Integration test file `tests/rust-integration/tests/tenancy_admin_api.rs` uses **2-space**. Do not run `cargo fmt`. Commit after each task.

**Test commands:**
- Backend unit: `cargo test -p knowledge-server <name>`
- Backend compile: `cargo build -p knowledge-server`
- Integration: `cargo test -p rust-integration --test tenancy_admin_api <name> -- --test-threads=1`
- Frontend: `npm test --workspace @knowledge/admin -- <relative-path>`
- Frontend type-check: `npm run lint --workspace @knowledge/admin`

---

## File Structure

**Backend (knowledge-server):**
- `src/auth/principal.rs` — add `require_session` (cookie-only principal). [Task 1]
- `src/http/error.rs` — add `ApiError::from_db_unique` (23505 → 400). [Task 2]
- `src/tenancy/spaces.rs` — make `create_org_space` / `create_team_space` executor-generic so they can run inside a transaction. [Task 3]
- `src/tenancy/orgs.rs` — session-only handlers + atomic `create_org_handler`. [Task 4]
- `src/tenancy/teams.rs` — session-only handlers + atomic `create_team_handler` + `list_teams_handler` returns `spaceId`/`role`. [Task 5]
- `src/tenancy/spaces_api.rs`, `src/tenancy/grants.rs`, `src/projects/routes.rs` (`create_project_handler` only) — session-only. [Task 6]

**Integration tests:**
- `tests/rust-integration/tests/tenancy_admin_api.rs` — `mint_token` helper + Bearer-rejection / duplicate-slug / non-member-team tests. [Tasks 4, 5, 6]

**Frontend:**
- `packages/api-client/src/schemas.ts` — `teamListSchema` gains `spaceId` + nullable `role`. [Task 7]
- `apps/admin/src/features/orgs/workspace-page.tsx` + `.test.tsx` — source team `spaceId`/`role` from `useOrgTeamsQuery`. [Task 7]
- `apps/admin/src/features/teams/team-page.tsx` + `.test.tsx` — source team from `useOrgTeamsQuery` so admins-not-on-team can manage. [Task 7]
- `apps/admin/src/features/shared/tenancy-api.ts` + new `tenancy-api.test.ts` — `space_id` → `spaceId` query param. [Task 8]

**Docs:**
- `docs/superpowers/specs/2026-06-19-team-layer-design.md` — capability-table fix. [Task 9]

---

## Task 1: `require_session` cookie-only principal helper

**Why first:** Tasks 4–6 depend on this helper. It has no caller yet, so it is infrastructure only — its behavior is locked by the Bearer-rejection integration tests in Tasks 4/5/6 (a standalone unit test would require constructing an `AppState` with a live DB, which is not worthwhile here).

**Files:**
- Modify: `crates/knowledge-server/src/auth/principal.rs` (2-space)

- [ ] **Step 1: Add `require_session` after `resolve_principal`**

Insert this function immediately after the closing `}` of `resolve_principal` (currently line 100), before the `#[cfg(test)]` module. `find_session` is already imported (line 5) and `extract_session_cookie` is defined above (line 53).

```rust
/// Resolve a Principal from the session cookie ONLY (never Bearer). Control-plane
/// endpoints use this so ambient-credential CSRF protection always applies; API
/// tokens must not reach org/team/grant management.
pub async fn require_session(
  state: &AppState,
  headers: &HeaderMap,
) -> Result<Principal, ApiError> {
  let session_id =
    extract_session_cookie(headers).ok_or_else(|| ApiError::unauthorized("missing session"))?;
  let session = find_session(state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;
  Ok(Principal {
    user_id: session.user_id,
    csrf_token: Some(session.csrf_token),
    scope: AuthScope::Session,
    token_id: None,
    project_id: None,
  })
}
```

- [ ] **Step 2: Compile and run existing unit tests**

Run: `cargo build -p knowledge-server`
Expected: builds (a `dead_code` warning for `require_session` is acceptable — it gains callers in Tasks 4–6).

Run: `cargo test -p knowledge-server auth::principal`
Expected: existing principal unit tests still PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/src/auth/principal.rs
git commit -m "feat(auth): add cookie-only require_session principal helper"
```

---

## Task 2: `ApiError::from_db_unique` (23505 → 400)

**Files:**
- Modify: `crates/knowledge-server/src/http/error.rs` (2-space)

- [ ] **Step 1: Write the failing test**

Append this test module at the end of `crates/knowledge-server/src/http/error.rs` (after the `impl IntoResponse` block). `StatusCode` is already imported at the top of the file.

```rust
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn from_db_unique_maps_non_database_error_to_internal() {
    let err = ApiError::from_db_unique(sqlx::Error::RowNotFound, "slug already taken");
    assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
  }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server --lib http::error`
Expected: FAIL — compile error, `no function or associated item named from_db_unique found`.

- [ ] **Step 3: Add the `DatabaseError` import**

At the top of `crates/knowledge-server/src/http/error.rs`, add the trait import below the existing `use` lines (after line 4):

```rust
use sqlx::error::DatabaseError;
```

- [ ] **Step 4: Add the `from_db_unique` constructor**

Inside `impl ApiError { ... }`, after the `unauthorized` method (currently ends at line 60), add:

```rust
  /// Map a SQLx error to 400 when it is a Postgres unique-constraint violation
  /// (SQLSTATE 23505); otherwise treat it as an internal error. Lets handlers
  /// rely on the DB's UNIQUE index instead of a racy COUNT precheck.
  pub fn from_db_unique(error: sqlx::Error, conflict_message: impl Into<String>) -> Self {
    if let sqlx::Error::Database(db_error) = &error {
      if db_error.code().as_deref() == Some("23505") {
        return Self::bad_request(conflict_message);
      }
    }
    Self::internal(error.to_string())
  }
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p knowledge-server --lib http::error`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/http/error.rs
git commit -m "feat(http): add ApiError::from_db_unique mapping 23505 to 400"
```

---

## Task 3: Make space-creation helpers executor-generic

**Why:** Task 4/5 must run `create_org_space` / `create_team_space` inside a transaction. Today they take `&PgPool`. Making them generic over `sqlx::PgExecutor` lets the same function accept `&PgPool` (unchanged callers) or `&mut *tx`. Pure refactor — no behavior change; verified by compile.

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/spaces.rs` (4-space)

- [ ] **Step 1: Rewrite `create_org_space` as executor-generic**

Replace the current `create_org_space` (lines 52–69) with:

```rust
/// Create the `spaces(kind='org')` row for a freshly created org.
pub async fn create_org_space<'e, E>(
    executor: E,
    org_id: &str,
    created_at: &str,
) -> Result<String, sqlx::Error>
where
    E: sqlx::PgExecutor<'e>,
{
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO spaces (id, kind, owner_user_id, org_id, created_at) \
         VALUES ($1, 'org', NULL, $2, $3)",
    )
    .bind(&id)
    .bind(org_id)
    .bind(created_at)
    .execute(executor)
    .await?;
    Ok(id)
}
```

- [ ] **Step 2: Rewrite `create_team_space` as executor-generic**

Replace the current `create_team_space` (lines 71–88) with:

```rust
/// Create the `spaces(kind='team')` row for a freshly created team.
pub async fn create_team_space<'e, E>(
    executor: E,
    team_id: &str,
    created_at: &str,
) -> Result<String, sqlx::Error>
where
    E: sqlx::PgExecutor<'e>,
{
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO spaces (id, kind, owner_user_id, org_id, team_id, created_at) \
         VALUES ($1, 'team', NULL, NULL, $2, $3)",
    )
    .bind(&id)
    .bind(team_id)
    .bind(created_at)
    .execute(executor)
    .await?;
    Ok(id)
}
```

Leave `use sqlx::PgPool;` (line 1) in place — `personal_space_id` / `ensure_personal_space` still use it.

- [ ] **Step 3: Verify it compiles (existing callers unchanged)**

Run: `cargo build -p knowledge-server`
Expected: builds. `orgs.rs:88` / `teams.rs:102` still pass `&state.pool`, which implements `PgExecutor`, so they compile without edits.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/tenancy/spaces.rs
git commit -m "refactor(tenancy): make space-creation helpers executor-generic"
```

---

## Task 4: orgs.rs — session-only handlers + atomic org creation

**Depends on:** Tasks 1, 2, 3.

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/orgs.rs` (4-space)
- Modify: `tests/rust-integration/tests/tenancy_admin_api.rs` (2-space)

- [ ] **Step 1: Add the `mint_token` integration-test helper**

In `tests/rust-integration/tests/tenancy_admin_api.rs`, add this helper after `admin_id` (after line 71). It uses the public API-token factory to mint an unscoped Bearer token for a user.

```rust
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
```

- [ ] **Step 2: Write the failing Bearer-rejection test**

Append to `tests/rust-integration/tests/tenancy_admin_api.rs`:

```rust
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
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_admin_api create_org_rejects_bearer_token -- --test-threads=1`
Expected: FAIL — current `create_org_handler` accepts the Bearer token and returns `201 CREATED`, not `401`.

- [ ] **Step 4: Swap all 5 handlers to `require_session` and fix imports**

In `crates/knowledge-server/src/tenancy/orgs.rs`, replace the import line 15:

```rust
use crate::projects::routes::{authorized_principal, validate_csrf};
```

with:

```rust
use crate::auth::principal::require_session;
use crate::projects::routes::validate_csrf;
```

Then replace **all** occurrences of:

```rust
    let principal = authorized_principal(&state, &headers, None).await?;
```

with:

```rust
    let principal = require_session(&state, &headers).await?;
```

(There are 5 occurrences — `create_org_handler`, `add_org_member_handler`, `list_org_members_handler`, `set_org_member_role_handler`, `remove_org_member_handler`. Use `replace_all`.)

- [ ] **Step 5: Make `create_org_handler` atomic and drop the COUNT precheck**

Replace the body of `create_org_handler` from the line after `validate_slug(&payload.slug)?;` through the end of the function (currently lines 48–101, i.e. the COUNT precheck + both inserts + `create_org_space` + the `Ok((...))`) with:

```rust
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    let org_id = Uuid::new_v4().to_string();

    let mut tx = state.pool.begin().await.map_err(ApiError::from)?;

    sqlx::query(
        "INSERT INTO organizations (id, name, slug, created_by, created_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&org_id)
    .bind(&payload.name)
    .bind(&payload.slug)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::from_db_unique(e, "slug already taken"))?;

    sqlx::query(
        "INSERT INTO organization_members (id, org_id, user_id, role, created_at) \
         VALUES ($1, $2, $3, 'org_admin', $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&org_id)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::from)?;

    let space_id = create_org_space(&mut *tx, &org_id, &created_at)
        .await
        .map_err(ApiError::from)?;

    tx.commit().await.map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": org_id,
            "name": payload.name,
            "slug": payload.slug,
            "spaceId": space_id,
        })),
    ))
```

The final `create_org_handler` reads (for reference):

```rust
async fn create_org_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateOrgRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = require_session(&state, &headers).await?;
    validate_csrf(&headers, &principal)?;
    validate_slug(&payload.slug)?;

    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    let org_id = Uuid::new_v4().to_string();

    let mut tx = state.pool.begin().await.map_err(ApiError::from)?;
    // ...inserts + create_org_space + tx.commit + Ok((...)) as above
}
```

- [ ] **Step 6: Run the Bearer-rejection test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_admin_api create_org_rejects_bearer_token -- --test-threads=1`
Expected: PASS — no cookie → `require_session` returns `401 UNAUTHORIZED`.

- [ ] **Step 7: Add the duplicate-slug behavior-lock test**

This guards that removing the COUNT precheck still yields `400` on a duplicate slug (now via the DB UNIQUE index + `from_db_unique`). Append to `tests/rust-integration/tests/tenancy_admin_api.rs`:

```rust
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
```

- [ ] **Step 8: Run both orgs tests**

Run: `cargo test -p rust-integration --test tenancy_admin_api create_org_ -- --test-threads=1`
Expected: PASS — `create_org_rejects_bearer_token` and `create_org_duplicate_slug_returns_400` both pass.

- [ ] **Step 9: Compile-check the whole crate (catch the other 4 swapped handlers)**

Run: `cargo build -p knowledge-server`
Expected: builds.

- [ ] **Step 10: Commit**

```bash
git add crates/knowledge-server/src/tenancy/orgs.rs tests/rust-integration/tests/tenancy_admin_api.rs
git commit -m "fix(tenancy): session-only org endpoints + atomic org creation"
```

---

## Task 5: teams.rs — session-only handlers, atomic team creation, list_teams returns spaceId/role

**Depends on:** Tasks 1, 2, 3.

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/teams.rs` (4-space)
- Modify: `tests/rust-integration/tests/tenancy_admin_api.rs` (2-space)

- [ ] **Step 1: Write the failing "admin sees non-member team with spaceId + null role" test**

This is finding #3 at the API layer. Append to `tests/rust-integration/tests/tenancy_admin_api.rs`:

```rust
#[tokio::test]
async fn list_teams_includes_non_member_team_for_org_admin() {
  let env = TestEnvironment::start("list_teams_admin_nonmember").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, _csrf) = login_admin(&state).await;
  let admin = admin_id(&state.pool).await;
  let (org_id, _ospace) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  // Team the admin is org_admin over but NOT a member of:
  let (team_id, tspace) = insert_team(&state.pool, &org_id, "platform").await;
  let _ = team_id;

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
  assert_eq!(teams[0]["spaceId"].as_str().unwrap(), tspace);
  assert!(teams[0]["role"].is_null());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_admin_api list_teams_includes_non_member_team_for_org_admin -- --test-threads=1`
Expected: FAIL — current `list_teams_handler` selects only `id, name, slug`; `teams[0]["spaceId"]` is JSON null so `.as_str().unwrap()` panics.

- [ ] **Step 3: Swap all 5 handlers to `require_session` and fix imports**

In `crates/knowledge-server/src/tenancy/teams.rs`, replace import line 15:

```rust
use crate::projects::routes::{authorized_principal, validate_csrf};
```

with:

```rust
use crate::auth::principal::require_session;
use crate::projects::routes::validate_csrf;
```

Then replace **all** occurrences of:

```rust
    let principal = authorized_principal(&state, &headers, None).await?;
```

with:

```rust
    let principal = require_session(&state, &headers).await?;
```

(5 occurrences — `create_team_handler`, `list_teams_handler`, `add_team_member_handler`, `list_team_members_handler`, `remove_team_member_handler`. Use `replace_all`.)

- [ ] **Step 4: Rewrite `list_teams_handler` to return `spaceId` + caller's `role`**

Replace the entire `list_teams_handler` (currently lines 118–157) with:

```rust
async fn list_teams_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(org_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let principal = require_session(&state, &headers).await?;
    let Some(role) = org_member_role(&state.pool, &org_id, &principal.user_id)
        .await
        .map_err(ApiError::from)?
    else {
        return Err(ApiError::forbidden("not a member of this org"));
    };

    let rows = if role == "org_admin" {
        sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            "SELECT t.id, t.name, t.slug, s.id, tm.role \
             FROM teams t \
             JOIN spaces s ON s.kind = 'team' AND s.team_id = t.id \
             LEFT JOIN team_members tm ON tm.team_id = t.id AND tm.user_id = $2 \
             WHERE t.org_id = $1 \
             ORDER BY t.created_at ASC",
        )
        .bind(&org_id)
        .bind(&principal.user_id)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::from)?
    } else {
        sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            "SELECT t.id, t.name, t.slug, s.id, tm.role \
             FROM teams t \
             JOIN team_members tm ON tm.team_id = t.id \
             JOIN spaces s ON s.kind = 'team' AND s.team_id = t.id \
             WHERE t.org_id = $1 AND tm.user_id = $2 \
             ORDER BY t.created_at ASC",
        )
        .bind(&org_id)
        .bind(&principal.user_id)
        .fetch_all(&state.pool)
        .await
        .map_err(ApiError::from)?
    };

    let teams = rows
        .into_iter()
        .map(|(id, name, slug, space_id, member_role)| {
            json!({
                "id": id,
                "name": name,
                "slug": slug,
                "orgId": org_id,
                "spaceId": space_id,
                "role": member_role,
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({ "teams": teams })))
}
```

Note: both branches share the tuple type `(String, String, String, String, Option<String>)`. In the non-admin branch `tm.role` is non-null (inner join) and decodes into `Some(_)`; in the admin branch the `LEFT JOIN` yields `None` for teams the caller is not a member of. `json!` serializes `None` → JSON `null`.

- [ ] **Step 5: Make `create_team_handler` atomic and drop the COUNT precheck**

Replace the body of `create_team_handler` from the line after `validate_slug(&payload.slug)?;` through the end of the function (currently lines 59–116, i.e. the COUNT precheck + both inserts + `create_team_space` + the `Ok((...))`) with:

```rust
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format created_at"))?;
    let team_id = Uuid::new_v4().to_string();

    let mut tx = state.pool.begin().await.map_err(ApiError::from)?;

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
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::from_db_unique(e, "slug already taken in this org"))?;

    sqlx::query(
        "INSERT INTO team_members (id, team_id, user_id, role, created_at) \
         VALUES ($1, $2, $3, 'leader', $4)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&team_id)
    .bind(&principal.user_id)
    .bind(&created_at)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::from)?;

    let space_id = create_team_space(&mut *tx, &team_id, &created_at)
        .await
        .map_err(ApiError::from)?;

    tx.commit().await.map_err(ApiError::from)?;

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
```

- [ ] **Step 6: Run the non-member-team test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_admin_api list_teams_includes_non_member_team_for_org_admin -- --test-threads=1`
Expected: PASS.

- [ ] **Step 7: Add the Bearer-rejection test for team creation**

Append to `tests/rust-integration/tests/tenancy_admin_api.rs`:

```rust
#[tokio::test]
async fn create_team_rejects_bearer_token() {
  let env = TestEnvironment::start("create_team_bearer").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (org_id, _ospace) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;
  let token = mint_token(&state, &admin).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/teams"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Platform", "slug": "platform" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
```

- [ ] **Step 8: Add the duplicate-slug behavior-lock test for teams**

Append to `tests/rust-integration/tests/tenancy_admin_api.rs`:

```rust
#[tokio::test]
async fn create_team_duplicate_slug_returns_400() {
  let env = TestEnvironment::start("create_team_dup_slug").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_admin(&state).await;
  let admin = admin_id(&state.pool).await;
  let (org_id, _ospace) = insert_org(&state.pool, &admin, "acme").await;
  add_org_member(&state.pool, &org_id, &admin, "org_admin").await;

  let first = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/teams"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Platform", "slug": "dup" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(first.status(), StatusCode::CREATED);

  let second = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/orgs/{org_id}/teams"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "Platform Two", "slug": "dup" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();
  assert_eq!(second.status(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 9: Run all teams tests + compile-check**

Run: `cargo build -p knowledge-server`
Expected: builds.

Run: `cargo test -p rust-integration --test tenancy_admin_api -- --test-threads=1`
Expected: PASS — all existing tests plus the three new team tests pass.

- [ ] **Step 10: Commit**

```bash
git add crates/knowledge-server/src/tenancy/teams.rs tests/rust-integration/tests/tenancy_admin_api.rs
git commit -m "fix(tenancy): session-only team endpoints, atomic creation, list_teams returns spaceId/role"
```

---

## Task 6: spaces_api / grants / create_project — session-only

**Depends on:** Task 1.

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/spaces_api.rs` (4-space)
- Modify: `crates/knowledge-server/src/tenancy/grants.rs` (4-space)
- Modify: `crates/knowledge-server/src/projects/routes.rs` (4-space) — `create_project_handler` ONLY
- Modify: `tests/rust-integration/tests/tenancy_admin_api.rs` (2-space)

**Critical scope note:** In `projects/routes.rs`, change ONLY `create_project_handler` (line 285). Do NOT touch `list_projects_handler` (line 303), `delete_project_handler` (line 470), or any data-plane handler — those intentionally accept API tokens. Use a context-anchored `Edit`, not `replace_all`.

- [ ] **Step 1: Write the failing Bearer-rejection tests**

Append to `tests/rust-integration/tests/tenancy_admin_api.rs`:

```rust
#[tokio::test]
async fn list_spaces_rejects_bearer_token() {
  let env = TestEnvironment::start("list_spaces_bearer").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let token = mint_token(&state, &admin).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("GET")
        .uri("/api/spaces")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_project_rejects_bearer_token() {
  let env = TestEnvironment::start("create_project_bearer").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let token = mint_token(&state, &admin).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "name": "scratch" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn upsert_grant_rejects_bearer_token() {
  let env = TestEnvironment::start("upsert_grant_bearer").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let admin = admin_id(&state.pool).await;
  let (_org_id, ospace) = insert_org(&state.pool, &admin, "acme").await;
  let project_id = insert_project_in_space(&state.pool, &ospace, "handbook").await;
  let token = mint_token(&state, &admin).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/grants"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "userId": admin, "role": "viewer" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rust-integration --test tenancy_admin_api _rejects_bearer_token -- --test-threads=1`
Expected: the 3 new tests FAIL (today these endpoints accept Bearer tokens, so they return 200/201/403/404 — not 401). `create_org_rejects_bearer_token` / `create_team_rejects_bearer_token` (already implemented) pass.

- [ ] **Step 3: Switch `spaces_api.rs` to `require_session`**

In `crates/knowledge-server/src/tenancy/spaces_api.rs`, replace import line 11:

```rust
use crate::projects::routes::authorized_principal;
```

with:

```rust
use crate::auth::principal::require_session;
```

Then replace line 22:

```rust
    let principal = authorized_principal(&state, &headers, None).await?;
```

with:

```rust
    let principal = require_session(&state, &headers).await?;
```

- [ ] **Step 4: Switch `grants.rs` to `require_session`**

In `crates/knowledge-server/src/tenancy/grants.rs`, replace import line 15:

```rust
use crate::projects::routes::{authorized_principal, validate_csrf};
```

with:

```rust
use crate::auth::principal::require_session;
use crate::projects::routes::validate_csrf;
```

Then replace **both** occurrences of:

```rust
    let principal = authorized_principal(&state, &headers, Some(&project_id)).await?;
```

with:

```rust
    let principal = require_session(&state, &headers).await?;
```

(2 occurrences — `upsert_grant_handler`, `delete_grant_handler`. Use `replace_all`. The real authz remains `can_manage_kb_access`.)

- [ ] **Step 5: Switch ONLY `create_project_handler` in `projects/routes.rs`**

Use this exact context-anchored replacement (old → new). This matches only the create handler, not the other handlers that also call `authorized_principal(&state, &headers, None)`:

Old:

```rust
async fn create_project_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_principal(&state, &headers, None).await?;
```

New:

```rust
async fn create_project_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = crate::auth::principal::require_session(&state, &headers).await?;
```

(Fully-qualified path — no import change needed in this large file.)

- [ ] **Step 6: Run the Bearer-rejection tests to verify they pass**

Run: `cargo build -p knowledge-server`
Expected: builds.

Run: `cargo test -p rust-integration --test tenancy_admin_api _rejects_bearer_token -- --test-threads=1`
Expected: PASS — all five `*_rejects_bearer_token` tests pass.

- [ ] **Step 7: Full integration sweep (no regressions)**

Run: `cargo test -p rust-integration --test tenancy_admin_api -- --test-threads=1`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/knowledge-server/src/tenancy/spaces_api.rs crates/knowledge-server/src/tenancy/grants.rs crates/knowledge-server/src/projects/routes.rs tests/rust-integration/tests/tenancy_admin_api.rs
git commit -m "fix(tenancy): session-only spaces, grants, and project creation"
```

---

## Task 7: Frontend — admin can manage non-member teams

**Depends on:** Task 5 (backend `list_teams` now returns `spaceId`/`role`).

**Goal:** Source each team's `spaceId` and the caller's `role` from `useOrgTeamsQuery` (which now carries them) instead of from `/api/spaces` (which omits teams the caller is not a member of). This makes "Manage" / "New team KB" / the whole team page work for an `org_admin` who is not on the team.

**Files:**
- Modify: `packages/api-client/src/schemas.ts` (2-space)
- Modify: `apps/admin/src/features/orgs/workspace-page.tsx` (2-space)
- Modify: `apps/admin/src/features/orgs/workspace-page.test.tsx` (2-space)
- Modify: `apps/admin/src/features/teams/team-page.tsx` (2-space)
- Modify: `apps/admin/src/features/teams/team-page.test.tsx` (2-space)

- [ ] **Step 1: Extend `teamListSchema` with `spaceId` + nullable `role`**

In `packages/api-client/src/schemas.ts`, replace `teamListSchema` (lines 59–68) with:

```ts
export const teamListSchema = z.object({
  teams: z.array(
    z.object({
      id: z.string(),
      name: z.string(),
      slug: z.string(),
      orgId: z.string(),
      spaceId: z.string(),
      role: z.enum(["leader", "member"]).nullable(),
    }),
  ),
});
```

(`role` is nullable because an `org_admin` who is not a team member gets `null`.)

- [ ] **Step 2: Update the workspace-page test mock first (failing-first via mock contract)**

In `apps/admin/src/features/orgs/workspace-page.test.tsx`, update the `useOrgTeamsQuery` mock (lines 29–32) to include `spaceId` + `role`, so the mock matches the new data the component will read:

```ts
  useOrgTeamsQuery: () => ({
    data: {
      teams: [
        { id: "team-1", name: "Platform", slug: "platform", orgId: "org-1", spaceId: "ts1", role: "leader" },
      ],
    },
    isLoading: false,
  }),
```

- [ ] **Step 3: Rewrite team sourcing in `workspace-page.tsx`**

In `apps/admin/src/features/orgs/workspace-page.tsx`, remove the `leaderTeamIds` block (lines 33–37 — the `const leaderTeamIds = new Set(...)` statement):

```ts
  const leaderTeamIds = new Set(
    (spaces.data?.teams ?? [])
      .filter((t) => t.orgId === orgId && t.role === "leader")
      .map((t) => t.id),
  );
```

Then, inside the `teamList.map((team) => { ... })` callback, replace the two derived constants (lines 91–92):

```ts
        const canManageTeam = isAdmin || leaderTeamIds.has(team.id);
        const teamSpaceId = (spaces.data?.teams ?? []).find((t) => t.id === team.id)?.spaceId ?? "";
```

with:

```ts
        const canManageTeam = isAdmin || team.role === "leader";
        const teamSpaceId = team.spaceId;
```

(`team` now comes from `useOrgTeamsQuery`, which carries `spaceId` and `role`.)

- [ ] **Step 4: Run the workspace-page test**

Run: `npm test --workspace @knowledge/admin -- src/features/orgs/workspace-page.test.tsx`
Expected: PASS — both existing assertions still hold (admin still sees "New team KB" because `canManageTeam` is true via `isAdmin`, and `teamSpaceId` is now `"ts1"` from the query mock).

- [ ] **Step 5: Rewrite `team-page.test.tsx` (failing-first for the admin-not-member case)**

Replace the entire contents of `apps/admin/src/features/teams/team-page.test.tsx` with:

```tsx
import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const mockState = vi.hoisted(() => ({
  orgRole: "org_member" as "org_admin" | "org_member",
  spacesTeams: [] as Array<Record<string, unknown>>,
  orgTeams: [] as Array<Record<string, unknown>>,
}));

vi.mock("../spaces/use-spaces", () => ({
  useSpacesQuery: () => ({
    data: {
      personal: { spaceId: "p" },
      orgs: [{ id: "org-1", slug: "acme", name: "Acme", spaceId: "s1", role: mockState.orgRole }],
      teams: mockState.spacesTeams,
    },
    isLoading: false,
  }),
}));

vi.mock("../orgs/workspace-queries", () => ({
  useOrgTeamsQuery: () => ({
    data: { teams: mockState.orgTeams },
    isLoading: false,
  }),
}));

vi.mock("./team-queries", () => ({
  useTeamMembersQuery: () => ({
    data: {
      members: [
        { userId: "u1", username: "lead", role: "leader" },
        { userId: "u2", username: "dev", role: "member" },
      ],
    },
    isLoading: false,
  }),
  useTeamProjectsQuery: () => ({
    data: [
      { id: "kb1", name: "Runbook", rootPath: "/r", createdAt: "t", spaceKind: "team", teamId: "team-1", teamSlug: "platform", role: "editor" },
    ],
    isLoading: false,
  }),
}));

vi.mock("./team-mutations", () => ({
  useAddTeamMemberMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useRemoveTeamMemberMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useCreateTeamKbMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

import { TeamPage } from "./team-page";

function renderPage() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/orgs/org-1/teams/team-1"]}>
        <Routes>
          <Route path="/orgs/:orgId/teams/:teamId" element={<TeamPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  mockState.orgRole = "org_member";
  mockState.spacesTeams = [
    { id: "team-1", orgId: "org-1", slug: "platform", name: "Platform", spaceId: "ts1", role: "leader" },
  ];
  mockState.orgTeams = [
    { id: "team-1", name: "Platform", slug: "platform", orgId: "org-1", spaceId: "ts1", role: "leader" },
  ];
});

describe("TeamPage", () => {
  it("renders members and team KBs", () => {
    renderPage();
    expect(screen.getByText("lead")).toBeInTheDocument();
    expect(screen.getByText("dev")).toBeInTheDocument();
    expect(screen.getByText("Runbook")).toBeInTheDocument();
  });

  it("leader sees add controls but cannot remove the leader row", () => {
    renderPage();
    expect(screen.getByRole("button", { name: /Add member/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Remove dev/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Remove lead/i })).toBeNull();
  });

  it("org_admin who is not a team member can still manage the team", () => {
    // /api/spaces omits teams the caller does not belong to:
    mockState.orgRole = "org_admin";
    mockState.spacesTeams = [];
    // useOrgTeamsQuery still surfaces the team (with role null for a non-member admin):
    mockState.orgTeams = [
      { id: "team-1", name: "Platform", slug: "platform", orgId: "org-1", spaceId: "ts1", role: null },
    ];
    renderPage();
    expect(screen.getByText("Team · Platform")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Add member/i })).toBeInTheDocument();
  });
});
```

- [ ] **Step 6: Run the team-page test to verify the new case fails**

Run: `npm test --workspace @knowledge/admin -- src/features/teams/team-page.test.tsx`
Expected: FAIL — the third test fails because `team-page.tsx` currently sources `team` from `spaces.data?.teams` (now empty for the admin), so it renders "Access denied or team not found." The first two tests may also fail to find the team because the component does not yet read from `useOrgTeamsQuery`.

- [ ] **Step 7: Rewrite team sourcing in `team-page.tsx`**

In `apps/admin/src/features/teams/team-page.tsx`, add the import after line 6 (`import { useTeamMembersQuery, useTeamProjectsQuery } from "./team-queries";`):

```ts
import { useOrgTeamsQuery } from "../orgs/workspace-queries";
```

Replace lines 18–19:

```ts
  const team = spaces.data?.teams.find((entry) => entry.id === teamId);
  const teamSpaceId = team?.spaceId ?? "";
```

with:

```ts
  const teamsQuery = useOrgTeamsQuery(orgId);
  const team = teamsQuery.data?.teams.find((entry) => entry.id === teamId);
  const teamSpaceId = team?.spaceId ?? "";
```

Replace the loading guard on line 36:

```ts
  if (spaces.isLoading) return <p>Loading…</p>;
```

with:

```ts
  if (spaces.isLoading || teamsQuery.isLoading) return <p>Loading…</p>;
```

(`org` / `isAdmin` keep coming from `spaces`; `isLeader = team?.role === "leader"` is unchanged and now reads from the `useOrgTeamsQuery` team.)

- [ ] **Step 8: Run the team-page test to verify it passes**

Run: `npm test --workspace @knowledge/admin -- src/features/teams/team-page.test.tsx`
Expected: PASS — all three tests pass.

- [ ] **Step 9: Type-check the admin app**

Run: `npm run lint --workspace @knowledge/admin`
Expected: PASS (no `tsc` errors). `team.spaceId` is a `string` and `team.role` is `"leader" | "member" | null` per the updated schema.

- [ ] **Step 10: Commit**

```bash
git add packages/api-client/src/schemas.ts apps/admin/src/features/orgs/workspace-page.tsx apps/admin/src/features/orgs/workspace-page.test.tsx apps/admin/src/features/teams/team-page.tsx apps/admin/src/features/teams/team-page.test.tsx
git commit -m "fix(admin): source team spaceId/role from useOrgTeamsQuery so org admins can manage non-member teams"
```

---

## Task 8: Fix the `space_id` → `spaceId` projects query param

**Independent** (no dependency on other tasks).

**Files:**
- Modify: `apps/admin/src/features/shared/tenancy-api.ts` (2-space)
- Create: `apps/admin/src/features/shared/tenancy-api.test.ts` (2-space)

- [ ] **Step 1: Write the failing contract test**

Create `apps/admin/src/features/shared/tenancy-api.test.ts`:

```ts
import { describe, expect, it, vi, afterEach } from "vitest";
import { fetchOrgProjects } from "./tenancy-api";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("fetchOrgProjects", () => {
  it("requests /api/projects with the camelCase spaceId query param", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ projects: [] }), { status: 200 }),
    );

    await fetchOrgProjects("s1");

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][0]).toBe("/api/projects?spaceId=s1");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test --workspace @knowledge/admin -- src/features/shared/tenancy-api.test.ts`
Expected: FAIL — the request URL is currently `/api/projects?space_id=s1` (snake_case), so the `toBe("/api/projects?spaceId=s1")` assertion fails.

- [ ] **Step 3: Fix the query param**

In `apps/admin/src/features/shared/tenancy-api.ts`, in `fetchOrgProjects` (line 25), change:

```ts
    `/api/projects?space_id=${encodeURIComponent(spaceId)}`,
```

to:

```ts
    `/api/projects?spaceId=${encodeURIComponent(spaceId)}`,
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm test --workspace @knowledge/admin -- src/features/shared/tenancy-api.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/shared/tenancy-api.ts apps/admin/src/features/shared/tenancy-api.test.ts
git commit -m "fix(admin): send spaceId (camelCase) query param to GET /api/projects"
```

---

## Task 9: Documentation — capability-table consistency

**Independent** (no dependency on other tasks). Doc-only; no automated test — verify by reading.

**Decision honored:** any team member (leader OR member) may create a KB in their own team. Backend + endpoint description are unchanged; only the capability table is corrected so it stops implying that KB *creation* is leader-only.

**Files:**
- Modify: `docs/superpowers/specs/2026-06-19-team-layer-design.md`

- [ ] **Step 1: Rename the conflated "Create / delete KB" row to "Delete KB"**

Replace line 153:

```markdown
| Create / delete KB | | | ✓ (own team) | ✓ |
```

with:

```markdown
| Delete KB | | | ✓ (own team) | ✓ |
```

(Delete is what is actually gated to leader-of-own-team + org_admin via `project_access_role == Owner || can_manage_kb_access` in `delete_project_handler`.)

- [ ] **Step 2: Extend the prose note to cover KB creation**

Replace the note (lines 156–157):

```markdown
Plus: **any org member** may create a team and becomes its leader (independent of
any KB role).
```

with:

```markdown
Plus: **any org member** may create a team and becomes its leader (independent of
any KB role). **Any team member** (leader or member) may create a KB in their own
team's space; creating a **public** org KB requires `org_admin`. This matches the
`POST /api/projects` gate ("team space → leader/member of that team").
```

- [ ] **Step 3: Verify by reading**

Read `docs/superpowers/specs/2026-06-19-team-layer-design.md` lines 146–162 and confirm the capability table and the prose note are internally consistent and agree with the endpoint description at lines 177–178.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/specs/2026-06-19-team-layer-design.md
git commit -m "docs(team-layer): clarify any team member may create a team KB"
```

---

## Final Verification (after all tasks)

- [ ] **Backend:** `cargo build -p knowledge-server` && `cargo test -p knowledge-server` — PASS
- [ ] **Integration:** `cargo test -p rust-integration --test tenancy_admin_api -- --test-threads=1` — PASS
- [ ] **Frontend:** `npm test --workspace @knowledge/admin` — PASS
- [ ] **Frontend types:** `npm run lint --workspace @knowledge/admin` — PASS
- [ ] Dispatch a final code review over the whole branch, then use **superpowers:finishing-a-development-branch**.
