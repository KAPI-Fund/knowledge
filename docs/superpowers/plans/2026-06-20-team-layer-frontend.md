# Team Layer Frontend (Plan 3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the multi-tenancy Admin UI (space switcher, org workspace, org/team member management, per-KB access grants, create-KB flows) to `apps/admin`, plus the four small additive backend read/update endpoints the UI needs.

**Architecture:** New React feature folders (`spaces`, `orgs`, `teams`, `kb-access`) under `apps/admin/src/features`, each following the existing `page/queries/mutations/page.test` convention with `apiFetch` + api-client Zod schemas. Real routes mount tenancy pages under `AppShell`; the active space is derived from the URL. Four additive Rust/Axum endpoints (list org members, set org member role, list team members, enrich project-members with username) are added to the existing `tenancy/` and `projects/` modules.

**Tech Stack:** React 19, Vite 7, react-router-dom 7, @tanstack/react-query 5, shadcn/ui + Tailwind 4, Zod 4, `@knowledge/api-client`; Rust + Axum + SQLx (runtime string queries) on the backend; vitest + @testing-library/react and `rust-integration` for tests.

---

## File Structure

**Create:**
- `apps/admin/src/features/shared/tenancy-api.ts` — all tenancy `apiFetch` calls (fetchers + mutators) in one module.
- `apps/admin/src/features/spaces/use-spaces.ts` — `useSpacesQuery()`.
- `apps/admin/src/features/spaces/space-switcher.tsx` — top-nav dropdown + "New org".
- `apps/admin/src/features/spaces/space-switcher.test.tsx`
- `apps/admin/src/features/spaces/create-org-dialog.tsx`
- `apps/admin/src/features/orgs/workspace-queries.ts`
- `apps/admin/src/features/orgs/workspace-mutations.ts`
- `apps/admin/src/features/orgs/create-public-project-dialog.tsx`
- `apps/admin/src/features/orgs/create-team-dialog.tsx`
- `apps/admin/src/features/orgs/workspace-page.tsx`
- `apps/admin/src/features/orgs/workspace-page.test.tsx`
- `apps/admin/src/features/orgs/members-queries.ts`
- `apps/admin/src/features/orgs/members-mutations.ts`
- `apps/admin/src/features/orgs/members-page.tsx`
- `apps/admin/src/features/orgs/members-page.test.tsx`
- `apps/admin/src/features/teams/team-queries.ts`
- `apps/admin/src/features/teams/team-mutations.ts`
- `apps/admin/src/features/teams/team-page.tsx`
- `apps/admin/src/features/teams/team-page.test.tsx`
- `apps/admin/src/features/kb-access/manage-access-queries.ts`
- `apps/admin/src/features/kb-access/manage-access-mutations.ts`
- `apps/admin/src/features/kb-access/manage-access-dialog.tsx`
- `apps/admin/src/features/kb-access/manage-access-dialog.test.tsx`
- `tests/rust-integration/tests/tenancy_admin_api.rs` — integration tests for the 4 additive endpoints.

**Modify:**
- `packages/api-client/src/schemas.ts` — add 3 member-list schemas + parse fns.
- `packages/api-client/src/schemas.test.ts` — add cases for the 3 schemas.
- `crates/knowledge-server/src/tenancy/orgs.rs` — add GET members + PATCH member role.
- `crates/knowledge-server/src/tenancy/teams.rs` — add GET team members.
- `crates/knowledge-server/src/projects/routes.rs` — enrich `list_project_members` with `username`.
- `apps/admin/src/components/layout/top-bar.tsx` — mount `<SpaceSwitcher />`.
- `apps/admin/src/app/router.tsx` — add the three tenancy routes (LAST task).

**Task ordering rationale (avoid forward references):** schemas first → backend endpoints (each independently testable) → frontend plumbing (`tenancy-api.ts`) → space switcher → kb-access dialog (imported by workspace + team pages) → org workspace → org members → team page → router wiring last (imports every page).

---

## Phase 1 — api-client schemas

### Task 1: Member-list Zod schemas

**Files:**
- Modify: `packages/api-client/src/schemas.ts`
- Test: `packages/api-client/src/schemas.test.ts`

- [ ] **Step 1: Write the failing tests**

Append to `packages/api-client/src/schemas.test.ts`:

```ts
import {
  parseOrgMemberList,
  parseTeamMemberList,
  parseProjectMemberList,
} from "./schemas";

describe("parseOrgMemberList", () => {
  it("parses org members with username and role", () => {
    const result = parseOrgMemberList({
      members: [
        { userId: "u1", username: "alice", role: "org_admin" },
        { userId: "u2", username: "bob", role: "org_member" },
      ],
    });
    expect(result.members).toHaveLength(2);
    expect(result.members[0]).toEqual({
      userId: "u1",
      username: "alice",
      role: "org_admin",
    });
  });

  it("rejects an unknown role", () => {
    expect(() =>
      parseOrgMemberList({
        members: [{ userId: "u1", username: "alice", role: "wizard" }],
      }),
    ).toThrow();
  });
});

describe("parseTeamMemberList", () => {
  it("parses team members with leader/member roles", () => {
    const result = parseTeamMemberList({
      members: [
        { userId: "u1", username: "alice", role: "leader" },
        { userId: "u2", username: "bob", role: "member" },
      ],
    });
    expect(result.members[1]).toEqual({
      userId: "u2",
      username: "bob",
      role: "member",
    });
  });
});

describe("parseProjectMemberList", () => {
  it("parses project members with canImport", () => {
    const result = parseProjectMemberList({
      members: [
        { userId: "u1", username: "alice", role: "owner", canImport: true },
        { userId: "u2", username: "bob", role: "viewer", canImport: false },
      ],
    });
    expect(result.members[0].canImport).toBe(true);
    expect(result.members[1].role).toBe("viewer");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm run test --workspace @knowledge/api-client`
Expected: FAIL — `parseOrgMemberList is not a function` (or import error) for the new cases.

- [ ] **Step 3: Add the schemas**

Append to `packages/api-client/src/schemas.ts`:

```ts
export const orgMemberListSchema = z.object({
  members: z.array(
    z.object({
      userId: z.string(),
      username: z.string(),
      role: z.enum(["org_admin", "org_member"]),
    }),
  ),
});

export function parseOrgMemberList(input: unknown) {
  return orgMemberListSchema.parse(input);
}

export const teamMemberListSchema = z.object({
  members: z.array(
    z.object({
      userId: z.string(),
      username: z.string(),
      role: z.enum(["leader", "member"]),
    }),
  ),
});

export function parseTeamMemberList(input: unknown) {
  return teamMemberListSchema.parse(input);
}

export const projectMemberListSchema = z.object({
  members: z.array(
    z.object({
      userId: z.string(),
      username: z.string(),
      role: z.enum(["owner", "editor", "viewer"]),
      canImport: z.boolean(),
    }),
  ),
});

export function parseProjectMemberList(input: unknown) {
  return projectMemberListSchema.parse(input);
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm run test --workspace @knowledge/api-client`
Expected: PASS (all new cases green; existing cases still green).

- [ ] **Step 5: Commit**

```bash
git add packages/api-client/src/schemas.ts packages/api-client/src/schemas.test.ts
git commit -m "feat(api-client): add org/team/project member-list schemas"
```

---

## Phase 2 — Additive backend endpoints

> All four tasks share one new integration-test file. Task 2 creates the file with `mod support;` + the local helpers; Tasks 3–5 append tests to it. Indentation in `tenancy/` and `projects/` source is **4-space**; integration tests are **2-space**. Run rust tests with `--test-threads=1`.

### Task 2: GET /api/orgs/{org}/members (list org members)

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/orgs.rs`
- Create: `tests/rust-integration/tests/tenancy_admin_api.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/rust-integration/tests/tenancy_admin_api.rs`:

```rust
mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

use support::{bootstrap_state, build_app, TestEnvironment};

async fn login_admin(app_state: &knowledge_server::AppState) -> (String, String) {
  let response = build_app(app_state.clone())
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
    .get(header::SET_COOKIE)
    .unwrap()
    .to_str()
    .unwrap()
    .split(';')
    .next()
    .unwrap()
    .to_string();
  let body = read_json(response.into_body()).await;
  let csrf = body["csrfToken"].as_str().unwrap().to_string();
  (cookie, csrf)
}

async fn read_json(body: Body) -> Value {
  let bytes = body.collect().await.unwrap().to_bytes();
  serde_json::from_slice(&bytes).unwrap()
}

async fn admin_id(pool: &PgPool) -> String {
  sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE username = $1")
    .bind("admin")
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_user(pool: &PgPool, username: &str) -> String {
  let id = Uuid::new_v4().to_string();
  let now = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
  sqlx::query(
    "INSERT INTO users (id, username, password_hash, role, created_at) \
     VALUES ($1, $2, $3, $4, $5)",
  )
  .bind(&id)
  .bind(username)
  .bind("x")
  .bind("member")
  .bind(&now)
  .execute(pool)
  .await
  .unwrap();
  id
}

async fn insert_org(pool: &PgPool, created_by: &str, slug: &str) -> (String, String) {
  let org_id = Uuid::new_v4().to_string();
  let space_id = Uuid::new_v4().to_string();
  let now = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
  sqlx::query("INSERT INTO spaces (id, kind, created_at) VALUES ($1, $2, $3)")
    .bind(&space_id)
    .bind("org")
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
  sqlx::query(
    "INSERT INTO organizations (id, slug, name, space_id, created_by, created_at) \
     VALUES ($1, $2, $3, $4, $5, $6)",
  )
  .bind(&org_id)
  .bind(slug)
  .bind(slug)
  .bind(&space_id)
  .bind(created_by)
  .bind(&now)
  .execute(pool)
  .await
  .unwrap();
  (org_id, space_id)
}

async fn add_org_member(pool: &PgPool, org_id: &str, user_id: &str, role: &str) {
  let now = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
  sqlx::query(
    "INSERT INTO organization_members (org_id, user_id, role, created_at) \
     VALUES ($1, $2, $3, $4)",
  )
  .bind(org_id)
  .bind(user_id)
  .bind(role)
  .bind(&now)
  .execute(pool)
  .await
  .unwrap();
}

#[tokio::test]
async fn list_org_members_returns_members_with_usernames() {
  let env = TestEnvironment::start("list_org_members").await;
  let state = bootstrap_state(&env).await;
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
  let env = TestEnvironment::start("list_org_members_403").await;
  let state = bootstrap_state(&env).await;
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_admin_api list_org_members -- --test-threads=1`
Expected: FAIL — route returns 404/405 (handler not yet registered), so the 200 assertion fails.

- [ ] **Step 3: Add the handler and route**

In `crates/knowledge-server/src/tenancy/orgs.rs`, add `get` to the routing import and `org_member_role` to the access import:

```rust
use axum::routing::{delete, get, post};
use crate::tenancy::access::{is_org_admin, org_member_role};
```

(If those `use` lines already import some of these names, merge — do not duplicate.)

Update the members route to also handle GET:

```rust
.route(
    "/api/orgs/{org_id}/members",
    post(add_org_member_handler).get(list_org_members_handler),
)
```

Add the handler (4-space indent, mirroring the existing handlers in this file):

```rust
async fn list_org_members_handler(
    State(state): State<AppState>,
    Path(org_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    if org_member_role(&state, &org_id, &principal.user_id)
        .await?
        .is_none()
    {
        return Err(ApiError::forbidden("not an organization member"));
    }
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT om.user_id, u.username, om.role \
         FROM organization_members om \
         JOIN users u ON u.id = om.user_id \
         WHERE om.org_id = $1 \
         ORDER BY om.created_at ASC",
    )
    .bind(&org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| ApiError::internal("failed to list organization members"))?;
    let members: Vec<Value> = rows
        .into_iter()
        .map(|(user_id, username, role)| {
            json!({ "userId": user_id, "username": username, "role": role })
        })
        .collect();
    Ok(Json(json!({ "members": members })))
}
```

> Match the actual imports already present at the top of `orgs.rs` for `State`, `Path`, `HeaderMap`, `Json`, `Value`, `json!`, `ApiError`, `AppState`, `authorized_principal`. They are already used by existing handlers in this file — reuse the same paths.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_admin_api list_org_members -- --test-threads=1`
Expected: PASS (both `list_org_members_*` tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/orgs.rs tests/rust-integration/tests/tenancy_admin_api.rs
git commit -m "feat(tenancy): GET org members endpoint"
```

---

### Task 3: PATCH /api/orgs/{org}/members/{userId} (set role)

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/orgs.rs`
- Test: `tests/rust-integration/tests/tenancy_admin_api.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/rust-integration/tests/tenancy_admin_api.rs`:

```rust
#[tokio::test]
async fn patch_org_member_role_updates_role() {
  let env = TestEnvironment::start("patch_org_role").await;
  let state = bootstrap_state(&env).await;
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
  let env = TestEnvironment::start("patch_org_role_400").await;
  let state = bootstrap_state(&env).await;
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
  let env = TestEnvironment::start("patch_org_role_403").await;
  let state = bootstrap_state(&env).await;
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
  let env = TestEnvironment::start("patch_org_role_404").await;
  let state = bootstrap_state(&env).await;
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rust-integration --test tenancy_admin_api patch_org_member_role -- --test-threads=1`
Expected: FAIL — PATCH route not registered (405/404), so the assertions fail.

- [ ] **Step 3: Add the request struct, handler, and route**

In `crates/knowledge-server/src/tenancy/orgs.rs`, ensure `patch` is imported:

```rust
use axum::routing::{delete, get, patch, post};
```

Update the member-by-id route to also handle PATCH:

```rust
.route(
    "/api/orgs/{org_id}/members/{user_id}",
    delete(remove_org_member_handler).patch(set_org_member_role_handler),
)
```

Add the request struct (near other request structs in the file) and the handler:

```rust
#[derive(serde::Deserialize)]
struct SetOrgMemberRoleRequest {
    role: String,
}

async fn set_org_member_role_handler(
    State(state): State<AppState>,
    Path((org_id, user_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<SetOrgMemberRoleRequest>,
) -> Result<Json<Value>, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    validate_csrf(&headers, &principal)?;
    if payload.role != "org_admin" && payload.role != "org_member" {
        return Err(ApiError::bad_request("invalid role"));
    }
    if !is_org_admin(&state, &org_id, &principal.user_id).await? {
        return Err(ApiError::forbidden("not an organization admin"));
    }
    let result = sqlx::query(
        "UPDATE organization_members SET role = $1 WHERE org_id = $2 AND user_id = $3",
    )
    .bind(&payload.role)
    .bind(&org_id)
    .bind(&user_id)
    .execute(&state.pool)
    .await
    .map_err(|_| ApiError::internal("failed to update member role"))?;
    if result.rows_affected() == 0 {
        return Err(ApiError::not_found("membership not found"));
    }
    Ok(Json(json!({
        "orgId": org_id,
        "userId": user_id,
        "role": payload.role,
    })))
}
```

> `validate_csrf` is already imported/used by the existing mutation handlers in this file (`add_org_member_handler`, `remove_org_member_handler`). Reuse that import; do not add a second one.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p rust-integration --test tenancy_admin_api patch_org_member_role -- --test-threads=1`
Expected: PASS (all four `patch_org_member_role_*` tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/orgs.rs tests/rust-integration/tests/tenancy_admin_api.rs
git commit -m "feat(tenancy): PATCH org member role endpoint"
```

---

### Task 4: GET /api/orgs/{org}/teams/{team}/members (list team members)

**Files:**
- Modify: `crates/knowledge-server/src/tenancy/teams.rs`
- Test: `tests/rust-integration/tests/tenancy_admin_api.rs`

- [ ] **Step 1: Write the failing test**

Append the team helpers (once) and tests to `tests/rust-integration/tests/tenancy_admin_api.rs`:

```rust
async fn insert_team(pool: &PgPool, org_id: &str, slug: &str) -> (String, String) {
  let team_id = Uuid::new_v4().to_string();
  let space_id = Uuid::new_v4().to_string();
  let now = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
  sqlx::query("INSERT INTO spaces (id, kind, created_at) VALUES ($1, $2, $3)")
    .bind(&space_id)
    .bind("team")
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
  sqlx::query(
    "INSERT INTO teams (id, org_id, slug, name, space_id, created_at) \
     VALUES ($1, $2, $3, $4, $5, $6)",
  )
  .bind(&team_id)
  .bind(org_id)
  .bind(slug)
  .bind(slug)
  .bind(&space_id)
  .bind(&now)
  .execute(pool)
  .await
  .unwrap();
  (team_id, space_id)
}

async fn add_team_member(pool: &PgPool, team_id: &str, user_id: &str, role: &str) {
  let now = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
  sqlx::query(
    "INSERT INTO team_members (team_id, user_id, role, created_at) \
     VALUES ($1, $2, $3, $4)",
  )
  .bind(team_id)
  .bind(user_id)
  .bind(role)
  .bind(&now)
  .execute(pool)
  .await
  .unwrap();
}

#[tokio::test]
async fn list_team_members_returns_members() {
  let env = TestEnvironment::start("list_team_members").await;
  let state = bootstrap_state(&env).await;
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
  let env = TestEnvironment::start("list_team_members_403").await;
  let state = bootstrap_state(&env).await;
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rust-integration --test tenancy_admin_api list_team_members -- --test-threads=1`
Expected: FAIL — route not registered (404/405).

- [ ] **Step 3: Add the handler and route**

In `crates/knowledge-server/src/tenancy/teams.rs`, the imports already include `routing::{delete, get, post}` and `access::{is_org_admin, org_member_role, team_member_role}` (verify; add any missing name).

Update the team-members route to also handle GET:

```rust
.route(
    "/api/orgs/{org_id}/teams/{team_id}/members",
    post(add_team_member_handler).get(list_team_members_handler),
)
```

Add the handler (4-space indent, mirroring this file's existing handlers):

```rust
async fn list_team_members_handler(
    State(state): State<AppState>,
    Path((org_id, team_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let principal = authorized_principal(&state, &headers, None).await?;
    let is_admin = is_org_admin(&state, &org_id, &principal.user_id).await?;
    let is_team_member = team_member_role(&state, &team_id, &principal.user_id)
        .await?
        .is_some();
    if !is_admin && !is_team_member {
        return Err(ApiError::forbidden("not permitted to view team members"));
    }
    let belongs = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM teams WHERE id = $1 AND org_id = $2",
    )
    .bind(&team_id)
    .bind(&org_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| ApiError::internal("failed to verify team"))?;
    if belongs == 0 {
        return Err(ApiError::not_found("team not found"));
    }
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT tm.user_id, u.username, tm.role \
         FROM team_members tm \
         JOIN users u ON u.id = tm.user_id \
         WHERE tm.team_id = $1 \
         ORDER BY tm.created_at ASC",
    )
    .bind(&team_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| ApiError::internal("failed to list team members"))?;
    let members: Vec<Value> = rows
        .into_iter()
        .map(|(user_id, username, role)| {
            json!({ "userId": user_id, "username": username, "role": role })
        })
        .collect();
    Ok(Json(json!({ "members": members })))
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p rust-integration --test tenancy_admin_api list_team_members -- --test-threads=1`
Expected: PASS (both `list_team_members_*` tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/tenancy/teams.rs tests/rust-integration/tests/tenancy_admin_api.rs
git commit -m "feat(tenancy): GET team members endpoint"
```

---

### Task 5: Enrich GET /api/projects/{id}/members with username

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs` (`list_project_members`, ~lines 505-532)
- Test: `tests/rust-integration/tests/tenancy_admin_api.rs`

- [ ] **Step 1: Write the failing test**

Append the project helpers (once) and the test to `tests/rust-integration/tests/tenancy_admin_api.rs`:

```rust
async fn insert_project_in_space(pool: &PgPool, space_id: &str, name: &str) -> String {
  let id = Uuid::new_v4().to_string();
  let now = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
  sqlx::query(
    "INSERT INTO projects (id, name, root_path, space_id, created_at) \
     VALUES ($1, $2, $3, $4, $5)",
  )
  .bind(&id)
  .bind(name)
  .bind(format!("/tmp/{name}"))
  .bind(space_id)
  .bind(&now)
  .execute(pool)
  .await
  .unwrap();
  id
}

async fn grant_kb(pool: &PgPool, project_id: &str, user_id: &str, role: &str) {
  let now = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
  sqlx::query(
    "INSERT INTO project_members (project_id, user_id, role, can_import, created_at) \
     VALUES ($1, $2, $3, $4, $5)",
  )
  .bind(project_id)
  .bind(user_id)
  .bind(role)
  .bind(true)
  .bind(&now)
  .execute(pool)
  .await
  .unwrap();
}

#[tokio::test]
async fn project_members_payload_includes_username() {
  let env = TestEnvironment::start("project_members_username").await;
  let state = bootstrap_state(&env).await;
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
```

> If `projects` requires additional NOT NULL columns the schema demands (verify against the migration), extend `insert_project_in_space` accordingly. The columns shown (`id,name,root_path,space_id,created_at`) match the create-project insert path.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p rust-integration --test tenancy_admin_api project_members_payload_includes_username -- --test-threads=1`
Expected: FAIL — `members[0]["username"]` is null (field absent), assertion fails.

- [ ] **Step 3: Enrich the query and JSON**

In `crates/knowledge-server/src/projects/routes.rs`, change `list_project_members` so the query joins `users` and the tuple/JSON include `username`:

```rust
let rows = sqlx::query_as::<_, (String, String, String, bool)>(
    "SELECT pm.user_id, u.username, pm.role, pm.can_import \
     FROM project_members pm \
     JOIN users u ON u.id = pm.user_id \
     WHERE pm.project_id = $1 \
     ORDER BY pm.created_at ASC",
)
.bind(&project_id)
.fetch_all(&state.pool)
.await
.map_err(|_| ApiError::internal("failed to list project members"))?;
let members: Vec<Value> = rows
    .into_iter()
    .map(|(user_id, username, role, can_import)| {
        json!({
            "userId": user_id,
            "username": username,
            "role": role,
            "canImport": can_import,
        })
    })
    .collect();
```

> Keep the surrounding handler signature, gate, and `Ok(Json(json!({ "members": members })))` return exactly as they are; only the query, tuple arity, and the per-row `json!` change. Match the existing variable names (`project_id`, `state.pool`) and the existing error constructor.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p rust-integration --test tenancy_admin_api project_members_payload_includes_username -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Run the full new test file to confirm no regressions**

Run: `cargo test -p rust-integration --test tenancy_admin_api -- --test-threads=1`
Expected: PASS (all tenancy_admin_api tests green).

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/projects/routes.rs tests/rust-integration/tests/tenancy_admin_api.rs
git commit -m "feat(projects): enrich project-members payload with username"
```

---

## Phase 3 — Frontend plumbing + space switcher

### Task 6: tenancy-api.ts (shared API module)

**Files:**
- Create: `apps/admin/src/features/shared/tenancy-api.ts`

This module has no dedicated test (it is exercised through the mocked page/query tests, matching the repo convention where `features/shared/api.ts` has no test). It is a plumbing task: create the file, verify it type-checks and the workspace build passes.

- [ ] **Step 1: Create the module**

Create `apps/admin/src/features/shared/tenancy-api.ts`. Use the SAME `apiFetch` call shape as the existing `apps/admin/src/features/shared/api.ts` (read it first — match its third-argument schema form verbatim). The contract below is the function names, URLs, methods, bodies, and `csrfHeader()` usage:

```ts
import {
  apiFetch,
  spaceListSchema,
  projectListSchema,
  teamListSchema,
  orgMemberListSchema,
  teamMemberListSchema,
  projectMemberListSchema,
  grantSchema,
} from "@knowledge/api-client";
import { z } from "zod";

function csrfHeader(): Record<string, string> {
  return {
    "x-csrf-token": window.sessionStorage.getItem("knowledge.csrfToken") ?? "",
  };
}

export function fetchSpaces() {
  return apiFetch("/api/spaces", { method: "GET" }, spaceListSchema);
}

export async function fetchOrgProjects(spaceId: string) {
  const response = await apiFetch(
    `/api/projects?space_id=${encodeURIComponent(spaceId)}`,
    { method: "GET" },
    projectListSchema,
  );
  return response.projects;
}

export function fetchOrgTeams(orgId: string) {
  return apiFetch(`/api/orgs/${orgId}/teams`, { method: "GET" }, teamListSchema);
}

export function fetchOrgMembers(orgId: string) {
  return apiFetch(
    `/api/orgs/${orgId}/members`,
    { method: "GET" },
    orgMemberListSchema,
  );
}

export function fetchTeamMembers(orgId: string, teamId: string) {
  return apiFetch(
    `/api/orgs/${orgId}/teams/${teamId}/members`,
    { method: "GET" },
    teamMemberListSchema,
  );
}

export function fetchProjectMembers(projectId: string) {
  return apiFetch(
    `/api/projects/${projectId}/members`,
    { method: "GET" },
    projectMemberListSchema,
  );
}

const createdOrgSchema = z.object({
  id: z.string(),
  name: z.string(),
  slug: z.string(),
  spaceId: z.string(),
});

export function createOrg(input: { name: string; slug: string }) {
  return apiFetch(
    "/api/orgs",
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify({ name: input.name, slug: input.slug }),
    },
    createdOrgSchema,
  );
}

const createdProjectSchema = z.object({
  id: z.string(),
  name: z.string(),
  rootPath: z.string(),
  createdAt: z.string(),
});

export function createSpaceProject(input: { name: string; spaceId: string }) {
  return apiFetch(
    "/api/projects",
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify({ name: input.name, spaceId: input.spaceId }),
    },
    createdProjectSchema,
  );
}

const createdTeamSchema = z.object({
  id: z.string(),
  name: z.string(),
  slug: z.string(),
  orgId: z.string(),
  spaceId: z.string(),
});

export function createTeam(orgId: string, input: { name: string; slug: string }) {
  return apiFetch(
    `/api/orgs/${orgId}/teams`,
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify({ name: input.name, slug: input.slug }),
    },
    createdTeamSchema,
  );
}

export function addOrgMember(
  orgId: string,
  input: { usernameOrEmail: string; role: "org_admin" | "org_member" },
) {
  return apiFetch(
    `/api/orgs/${orgId}/members`,
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify(input),
    },
    z.unknown(),
  );
}

export function setOrgMemberRole(
  orgId: string,
  userId: string,
  role: "org_admin" | "org_member",
) {
  return apiFetch(
    `/api/orgs/${orgId}/members/${userId}`,
    {
      method: "PATCH",
      headers: { ...csrfHeader() },
      body: JSON.stringify({ role }),
    },
    z.unknown(),
  );
}

export function removeOrgMember(orgId: string, userId: string) {
  return apiFetch(
    `/api/orgs/${orgId}/members/${userId}`,
    { method: "DELETE", headers: { ...csrfHeader() } },
    z.void(),
  );
}

export function addTeamMember(
  orgId: string,
  teamId: string,
  input: { usernameOrEmail: string },
) {
  return apiFetch(
    `/api/orgs/${orgId}/teams/${teamId}/members`,
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify(input),
    },
    z.unknown(),
  );
}

export function removeTeamMember(orgId: string, teamId: string, userId: string) {
  return apiFetch(
    `/api/orgs/${orgId}/teams/${teamId}/members/${userId}`,
    { method: "DELETE", headers: { ...csrfHeader() } },
    z.void(),
  );
}

export function upsertGrant(
  projectId: string,
  input: { userId: string; role: "editor" | "viewer" },
) {
  return apiFetch(
    `/api/projects/${projectId}/grants`,
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify(input),
    },
    grantSchema,
  );
}

export function removeGrant(projectId: string, userId: string) {
  return apiFetch(
    `/api/projects/${projectId}/grants/${userId}`,
    { method: "DELETE", headers: { ...csrfHeader() } },
    z.void(),
  );
}
```

> **Convention (verified against `apps/admin/src/features/shared/api.ts` + `packages/api-client/src/http.ts`):** `apiFetch<T>(path, init: RequestInit, schema: ZodType<T>)` — `init` is required (use `{ method: "GET" }` for reads), and the third arg is a **Zod schema object** (not a parse function). On 204 it returns `schema.parse(undefined)`, so deletes use `z.void()`. List endpoints that wrap their payload (`{projects:[...]}`, `{teams:[...]}`, `{members:[...]}`) are parsed with the wrapping schema; `fetchOrgProjects` unwraps to `.projects` to return a bare array (matching the existing `listProjects` in `api.ts`), while the others return the wrapping object so callers read `.members` / `.teams`.

- [ ] **Step 2: Type-check / build the workspace**

Run: `npm run build --workspace @knowledge/admin`
Expected: build succeeds (no TS errors). If a faster `typecheck` script exists, use it.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/shared/tenancy-api.ts
git commit -m "feat(admin): add tenancy-api shared module"
```

---

### Task 7: Spaces feature — use-spaces, space-switcher, create-org-dialog

**Files:**
- Create: `apps/admin/src/features/spaces/use-spaces.ts`
- Create: `apps/admin/src/features/spaces/create-org-dialog.tsx`
- Create: `apps/admin/src/features/spaces/space-switcher.tsx`
- Create: `apps/admin/src/features/spaces/space-switcher.test.tsx`
- Modify: `apps/admin/src/components/layout/top-bar.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/features/spaces/space-switcher.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const navigateMock = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>(
    "react-router-dom",
  );
  return { ...actual, useNavigate: () => navigateMock };
});

vi.mock("./use-spaces", () => ({
  useSpacesQuery: () => ({
    data: {
      personal: { spaceId: "personal-space" },
      orgs: [
        { id: "org-1", slug: "acme", name: "Acme", spaceId: "s1", role: "org_admin" },
        { id: "org-2", slug: "globex", name: "Globex", spaceId: "s2", role: "org_member" },
      ],
      teams: [],
    },
    isLoading: false,
  }),
}));

import { SpaceSwitcher } from "./space-switcher";

function renderSwitcher() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <SpaceSwitcher />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("SpaceSwitcher", () => {
  it("lists Personal and each org", () => {
    renderSwitcher();
    expect(screen.getByText("Personal")).toBeInTheDocument();
    expect(screen.getByText("Acme")).toBeInTheDocument();
    expect(screen.getByText("Globex")).toBeInTheDocument();
  });

  it("navigates to /projects when Personal is chosen", () => {
    renderSwitcher();
    fireEvent.click(screen.getByText("Personal"));
    expect(navigateMock).toHaveBeenCalledWith("/projects");
  });

  it("navigates to the org workspace when an org is chosen", () => {
    renderSwitcher();
    fireEvent.click(screen.getByText("Acme"));
    expect(navigateMock).toHaveBeenCalledWith("/orgs/org-1");
  });

  it("opens the New org dialog", () => {
    renderSwitcher();
    fireEvent.click(screen.getByText("New org"));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});
```

> Mirror an existing `apps/admin` `*.test.tsx` for the exact import/setup (whether `@testing-library/jest-dom` matchers are globally available). If `toBeInTheDocument` is not set up, assert truthiness on `screen.getByText(...)` / `screen.queryByRole(...)` as the existing tests do.

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- src/features/spaces/space-switcher.test.tsx`
Expected: FAIL — module `./space-switcher` not found / `SpaceSwitcher` undefined.

- [ ] **Step 3: Implement use-spaces**

Create `apps/admin/src/features/spaces/use-spaces.ts`:

```ts
import { useQuery } from "@tanstack/react-query";
import { fetchSpaces } from "../shared/tenancy-api";

export function useSpacesQuery() {
  return useQuery({ queryKey: ["spaces"], queryFn: fetchSpaces });
}
```

- [ ] **Step 4: Implement create-org-dialog**

Create `apps/admin/src/features/spaces/create-org-dialog.tsx`:

```tsx
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { createOrg } from "../shared/tenancy-api";

export function CreateOrgDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => createOrg({ name: name.trim(), slug: slug.trim() }),
    onSuccess: async (org) => {
      await queryClient.invalidateQueries({ queryKey: ["spaces"] });
      onOpenChange(false);
      navigate(`/orgs/${org.id}`);
    },
  });

  const submit = async () => {
    setErrorMessage(null);
    try {
      await mutation.mutateAsync();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create org");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New org</DialogTitle>
          <DialogDescription>Create a new organization.</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <Input
            placeholder="Name"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
          <Input
            placeholder="Slug"
            value={slug}
            onChange={(event) => setSlug(event.target.value)}
          />
          {errorMessage ? (
            <p className="text-sm text-destructive">{errorMessage}</p>
          ) : null}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} type="button">
            Cancel
          </Button>
          <Button onClick={submit} disabled={mutation.isPending} type="button">
            Create
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

> Match the actual prop names and import paths of the shadcn `Dialog`/`Button`/`Input` in `apps/admin/src/components/ui/`. `projects/page.tsx` is the canonical create-dialog example — copy its imports and class names. If `Button` has no `variant="outline"`, use the codebase's secondary-button variant.

- [ ] **Step 5: Implement space-switcher**

Create `apps/admin/src/features/spaces/space-switcher.tsx`:

```tsx
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSpacesQuery } from "./use-spaces";
import { CreateOrgDialog } from "./create-org-dialog";

export function SpaceSwitcher() {
  const navigate = useNavigate();
  const { data, isLoading } = useSpacesQuery();
  const [dialogOpen, setDialogOpen] = useState(false);

  if (isLoading || !data) {
    return null;
  }

  return (
    <div className="flex items-center gap-2">
      <button type="button" onClick={() => navigate("/projects")}>
        Personal
      </button>
      {data.orgs.map((org) => (
        <button key={org.id} type="button" onClick={() => navigate(`/orgs/${org.id}`)}>
          {org.name}
        </button>
      ))}
      <button type="button" onClick={() => setDialogOpen(true)}>
        New org
      </button>
      <CreateOrgDialog open={dialogOpen} onOpenChange={setDialogOpen} />
    </div>
  );
}
```

> This renders a flat row of buttons to satisfy the tests and keep behavior obvious. If the codebase has a `DropdownMenu` primitive and you prefer the spec's dropdown look, wrap the same options/handlers in it — but keep the accessible text labels ("Personal", each org name, "New org") so the tests still pass.

- [ ] **Step 6: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- src/features/spaces/space-switcher.test.tsx`
Expected: PASS (all four cases).

- [ ] **Step 7: Mount the switcher in the top bar**

In `apps/admin/src/components/layout/top-bar.tsx`, import and render `<SpaceSwitcher />` inside the existing header's flex container (next to the logo/title), matching the existing JSX:

```tsx
import { SpaceSwitcher } from "../../features/spaces/space-switcher";
```

- [ ] **Step 8: Verify the build**

Run: `npm run build --workspace @knowledge/admin`
Expected: build succeeds.

- [ ] **Step 9: Commit**

```bash
git add apps/admin/src/features/spaces apps/admin/src/components/layout/top-bar.tsx
git commit -m "feat(admin): space switcher with new-org dialog in top bar"
```

---

## Phase 4 — Tenancy pages

### Task 8: KB-access manage-access dialog

**Files:**
- Create: `apps/admin/src/features/kb-access/manage-access-queries.ts`
- Create: `apps/admin/src/features/kb-access/manage-access-mutations.ts`
- Create: `apps/admin/src/features/kb-access/manage-access-dialog.tsx`
- Create: `apps/admin/src/features/kb-access/manage-access-dialog.test.tsx`

The dialog is self-sufficient: props `{ projectId, spaceKind, orgId, teamId, open, onOpenChange }`. It lists current grantees and offers add/remove. Candidate users come from org members (public KB) or team members (team KB).

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/features/kb-access/manage-access-dialog.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const removeGrantMock = vi.fn();
const upsertGrantMock = vi.fn();

vi.mock("./manage-access-queries", () => ({
  useProjectGranteesQuery: () => ({
    data: {
      members: [
        { userId: "u1", username: "alice", role: "owner", canImport: true },
        { userId: "u2", username: "bob", role: "viewer", canImport: false },
      ],
    },
    isLoading: false,
  }),
  useGrantCandidatesQuery: () => ({
    data: [
      { userId: "u2", username: "bob" },
      { userId: "u3", username: "carol" },
    ],
    isLoading: false,
  }),
}));

vi.mock("./manage-access-mutations", () => ({
  useUpsertGrantMutation: () => ({ mutateAsync: upsertGrantMock, isPending: false }),
  useRemoveGrantMutation: () => ({ mutateAsync: removeGrantMock, isPending: false }),
}));

import { ManageAccessDialog } from "./manage-access-dialog";

function renderDialog() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <ManageAccessDialog
        projectId="p1"
        spaceKind="org"
        orgId="org-1"
        teamId={null}
        open
        onOpenChange={() => {}}
      />
    </QueryClientProvider>,
  );
}

describe("ManageAccessDialog", () => {
  it("lists current grantees by username", () => {
    renderDialog();
    expect(screen.getByText("alice")).toBeInTheDocument();
    expect(screen.getByText("bob")).toBeInTheDocument();
  });

  it("removes a grantee", () => {
    renderDialog();
    fireEvent.click(screen.getByRole("button", { name: /remove bob/i }));
    expect(removeGrantMock).toHaveBeenCalledWith({ userId: "u2" });
  });

  it("adds a grant for a chosen candidate", () => {
    renderDialog();
    fireEvent.change(screen.getByLabelText("Add user"), {
      target: { value: "u3" },
    });
    fireEvent.change(screen.getByLabelText("Grant role"), {
      target: { value: "editor" },
    });
    fireEvent.click(screen.getByRole("button", { name: /add grant/i }));
    expect(upsertGrantMock).toHaveBeenCalledWith({ userId: "u3", role: "editor" });
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- src/features/kb-access/manage-access-dialog.test.tsx`
Expected: FAIL — `./manage-access-dialog` not found.

- [ ] **Step 3: Implement manage-access-queries**

Create `apps/admin/src/features/kb-access/manage-access-queries.ts`:

```ts
import { useQuery } from "@tanstack/react-query";
import {
  fetchProjectMembers,
  fetchOrgMembers,
  fetchTeamMembers,
} from "../shared/tenancy-api";

export function useProjectGranteesQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-grants", projectId],
    queryFn: () => fetchProjectMembers(projectId),
  });
}

export function useGrantCandidatesQuery(params: {
  spaceKind: "org" | "team";
  orgId: string;
  teamId: string | null;
}) {
  const { spaceKind, orgId, teamId } = params;
  return useQuery({
    queryKey:
      spaceKind === "team"
        ? ["grant-candidates", "team", orgId, teamId]
        : ["grant-candidates", "org", orgId],
    queryFn: async () => {
      if (spaceKind === "team" && teamId) {
        const result = await fetchTeamMembers(orgId, teamId);
        return result.members.map((m) => ({
          userId: m.userId,
          username: m.username,
        }));
      }
      const result = await fetchOrgMembers(orgId);
      return result.members.map((m) => ({
        userId: m.userId,
        username: m.username,
      }));
    },
  });
}
```

- [ ] **Step 4: Implement manage-access-mutations**

Create `apps/admin/src/features/kb-access/manage-access-mutations.ts`:

```ts
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { upsertGrant, removeGrant } from "../shared/tenancy-api";

export function useUpsertGrantMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { userId: string; role: "editor" | "viewer" }) =>
      upsertGrant(projectId, input),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["project-grants", projectId],
      });
    },
  });
}

export function useRemoveGrantMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { userId: string }) =>
      removeGrant(projectId, input.userId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["project-grants", projectId],
      });
    },
  });
}
```

- [ ] **Step 5: Implement manage-access-dialog**

Create `apps/admin/src/features/kb-access/manage-access-dialog.tsx`:

```tsx
import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { Button } from "../../components/ui/button";
import {
  useProjectGranteesQuery,
  useGrantCandidatesQuery,
} from "./manage-access-queries";
import {
  useUpsertGrantMutation,
  useRemoveGrantMutation,
} from "./manage-access-mutations";

export function ManageAccessDialog({
  projectId,
  spaceKind,
  orgId,
  teamId,
  open,
  onOpenChange,
}: {
  projectId: string;
  spaceKind: "org" | "team";
  orgId: string;
  teamId: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const grantees = useProjectGranteesQuery(projectId);
  const candidates = useGrantCandidatesQuery({ spaceKind, orgId, teamId });
  const upsert = useUpsertGrantMutation(projectId);
  const remove = useRemoveGrantMutation(projectId);

  const [selectedUser, setSelectedUser] = useState("");
  const [selectedRole, setSelectedRole] = useState<"editor" | "viewer">("editor");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const addGrant = async () => {
    if (!selectedUser) return;
    setErrorMessage(null);
    try {
      await upsert.mutateAsync({ userId: selectedUser, role: selectedRole });
      setSelectedUser("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to add grant");
    }
  };

  const removeGrantee = async (userId: string) => {
    setErrorMessage(null);
    try {
      await remove.mutateAsync({ userId });
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to remove grant");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Manage access</DialogTitle>
        </DialogHeader>

        <ul className="flex flex-col gap-1">
          {grantees.data?.members.map((member) => (
            <li key={member.userId} className="flex items-center justify-between">
              <span>
                {member.username} ({member.role})
              </span>
              {member.role === "owner" ? null : (
                <Button
                  type="button"
                  variant="outline"
                  aria-label={`Remove ${member.username}`}
                  onClick={() => removeGrantee(member.userId)}
                >
                  Remove
                </Button>
              )}
            </li>
          ))}
        </ul>

        <div className="flex items-end gap-2">
          <label className="flex flex-col text-sm">
            Add user
            <select
              aria-label="Add user"
              value={selectedUser}
              onChange={(event) => setSelectedUser(event.target.value)}
            >
              <option value="">Select…</option>
              {candidates.data?.map((candidate) => (
                <option key={candidate.userId} value={candidate.userId}>
                  {candidate.username}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col text-sm">
            Grant role
            <select
              aria-label="Grant role"
              value={selectedRole}
              onChange={(event) =>
                setSelectedRole(event.target.value as "editor" | "viewer")
              }
            >
              <option value="editor">editor</option>
              <option value="viewer">viewer</option>
            </select>
          </label>
          <Button type="button" onClick={addGrant} disabled={upsert.isPending}>
            Add grant
          </Button>
        </div>

        {errorMessage ? (
          <p className="text-sm text-destructive">{errorMessage}</p>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}
```

> The test uses raw `<select aria-label=...>`; this matches the codebase's native `Select` (`components/ui/select.tsx` wraps `<select>` and exposes `role="combobox"`). You may swap the raw `<select>` for the `Select` component as long as the `aria-label`s and `<option>` values are preserved so the test selectors still resolve.

- [ ] **Step 6: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- src/features/kb-access/manage-access-dialog.test.tsx`
Expected: PASS (all three cases).

- [ ] **Step 7: Commit**

```bash
git add apps/admin/src/features/kb-access
git commit -m "feat(admin): per-KB manage-access dialog"
```

---

### Task 9: Org workspace page (stacked sections)

**Files:**
- Create: `apps/admin/src/features/orgs/workspace-queries.ts`
- Create: `apps/admin/src/features/orgs/workspace-mutations.ts`
- Create: `apps/admin/src/features/orgs/create-public-project-dialog.tsx`
- Create: `apps/admin/src/features/orgs/create-team-dialog.tsx`
- Create: `apps/admin/src/features/orgs/workspace-page.tsx`
- Create: `apps/admin/src/features/orgs/workspace-page.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/features/orgs/workspace-page.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

vi.mock("../spaces/use-spaces", () => ({
  useSpacesQuery: () => ({
    data: {
      personal: { spaceId: "personal-space" },
      orgs: [
        { id: "org-1", slug: "acme", name: "Acme", spaceId: "org-space-1", role: "org_admin" },
      ],
      teams: [
        { id: "team-1", orgId: "org-1", slug: "platform", name: "Platform", spaceId: "ts1", role: "leader" },
      ],
    },
    isLoading: false,
  }),
}));

vi.mock("./workspace-queries", () => ({
  useOrgProjectsQuery: () => ({
    data: [
      { id: "kb1", name: "Handbook", rootPath: "/h", createdAt: "t", spaceKind: "org", teamId: null, teamSlug: null, role: "owner" },
      { id: "kb2", name: "Runbook", rootPath: "/r", createdAt: "t", spaceKind: "team", teamId: "team-1", teamSlug: "platform", role: "editor" },
    ],
    isLoading: false,
  }),
  useOrgTeamsQuery: () => ({
    data: { teams: [{ id: "team-1", name: "Platform", slug: "platform", orgId: "org-1" }] },
    isLoading: false,
  }),
}));

import { OrgWorkspacePage } from "./workspace-page";

function renderPage() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/orgs/org-1"]}>
        <Routes>
          <Route path="/orgs/:orgId" element={<OrgWorkspacePage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("OrgWorkspacePage", () => {
  it("renders a Public projects section and a per-team section", () => {
    renderPage();
    expect(screen.getByText("Public projects")).toBeInTheDocument();
    expect(screen.getByText("Handbook")).toBeInTheDocument();
    expect(screen.getByText(/Platform/)).toBeInTheDocument();
    expect(screen.getByText("Runbook")).toBeInTheDocument();
  });

  it("shows admin actions for an org_admin", () => {
    renderPage();
    // Exact-string names: "New team" must not collide with the team section's
    // "New team KB" button (a /New team/i regex would match both and throw).
    expect(screen.getByRole("button", { name: "New public project" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New team" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New team KB" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Members" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- src/features/orgs/workspace-page.test.tsx`
Expected: FAIL — `./workspace-page` not found.

- [ ] **Step 3: Implement workspace-queries**

Create `apps/admin/src/features/orgs/workspace-queries.ts`:

```ts
import { useQuery } from "@tanstack/react-query";
import { fetchOrgProjects, fetchOrgTeams } from "../shared/tenancy-api";

export function useOrgProjectsQuery(orgSpaceId: string) {
  return useQuery({
    queryKey: ["org-projects", orgSpaceId],
    queryFn: () => fetchOrgProjects(orgSpaceId),
    enabled: Boolean(orgSpaceId),
  });
}

export function useOrgTeamsQuery(orgId: string) {
  return useQuery({
    queryKey: ["org-teams", orgId],
    queryFn: () => fetchOrgTeams(orgId),
    enabled: Boolean(orgId),
  });
}
```

- [ ] **Step 4: Implement workspace-mutations**

Create `apps/admin/src/features/orgs/workspace-mutations.ts`:

```ts
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createSpaceProject, createTeam } from "../shared/tenancy-api";

export function useCreateSpaceProjectMutation(orgSpaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { name: string; spaceId: string }) =>
      createSpaceProject(input),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["org-projects", orgSpaceId],
      });
    },
  });
}

export function useCreateTeamMutation(orgId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { name: string; slug: string }) =>
      createTeam(orgId, input),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["org-teams", orgId] });
      await queryClient.invalidateQueries({ queryKey: ["spaces"] });
    },
  });
}
```

- [ ] **Step 5: Implement create-public-project-dialog**

Create `apps/admin/src/features/orgs/create-public-project-dialog.tsx`. This one dialog
serves both the org-public and the per-team "New KB" flows, because the create endpoint is
identical (`POST /api/projects {name, spaceId}`) and only the target space differs:
- `targetSpaceId` — the space the new KB is created in (org space for public, team space for a team KB).
- `listSpaceId` — the space whose `["org-projects", listSpaceId]` cache the workspace page
  reads. Always the **org** space here, because `GET /api/projects?space_id=<orgSpaceId>`
  returns both org-public **and** team KBs (see Task 6 `fetchOrgProjects`), so creating a team
  KB must invalidate the org-space list the page renders from.
- `title` — dialog heading ("New public project" or "New team KB").

```tsx
import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "../../components/ui/dialog";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { useCreateSpaceProjectMutation } from "./workspace-mutations";

export function CreatePublicProjectDialog({
  targetSpaceId,
  listSpaceId,
  title,
  open,
  onOpenChange,
}: {
  targetSpaceId: string;
  listSpaceId: string;
  title: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [name, setName] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const mutation = useCreateSpaceProjectMutation(listSpaceId);

  const submit = async () => {
    setErrorMessage(null);
    try {
      await mutation.mutateAsync({ name: name.trim(), spaceId: targetSpaceId });
      onOpenChange(false);
      setName("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create project");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        <Input
          placeholder="Name"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        {errorMessage ? (
          <p className="text-sm text-destructive">{errorMessage}</p>
        ) : null}
        <DialogFooter>
          <Button type="button" onClick={submit} disabled={mutation.isPending}>
            Create
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 6: Implement create-team-dialog**

Create `apps/admin/src/features/orgs/create-team-dialog.tsx`:

```tsx
import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "../../components/ui/dialog";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { useCreateTeamMutation } from "./workspace-mutations";

export function CreateTeamDialog({
  orgId,
  open,
  onOpenChange,
}: {
  orgId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const mutation = useCreateTeamMutation(orgId);

  const submit = async () => {
    setErrorMessage(null);
    try {
      await mutation.mutateAsync({ name: name.trim(), slug: slug.trim() });
      onOpenChange(false);
      setName("");
      setSlug("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create team");
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>New team</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <Input
            placeholder="Name"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
          <Input
            placeholder="Slug"
            value={slug}
            onChange={(event) => setSlug(event.target.value)}
          />
        </div>
        {errorMessage ? (
          <p className="text-sm text-destructive">{errorMessage}</p>
        ) : null}
        <DialogFooter>
          <Button type="button" onClick={submit} disabled={mutation.isPending}>
            Create
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 7: Implement workspace-page**

Create `apps/admin/src/features/orgs/workspace-page.tsx`:

```tsx
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Button } from "../../components/ui/button";
import { useSpacesQuery } from "../spaces/use-spaces";
import { useOrgProjectsQuery, useOrgTeamsQuery } from "./workspace-queries";
import { CreatePublicProjectDialog } from "./create-public-project-dialog";
import { CreateTeamDialog } from "./create-team-dialog";
import { ManageAccessDialog } from "../kb-access/manage-access-dialog";

export function OrgWorkspacePage() {
  const { orgId = "" } = useParams();
  const spaces = useSpacesQuery();
  const org = spaces.data?.orgs.find((entry) => entry.id === orgId);
  const orgSpaceId = org?.spaceId ?? "";

  const projects = useOrgProjectsQuery(orgSpaceId);
  const teams = useOrgTeamsQuery(orgId);

  const [publicDialogOpen, setPublicDialogOpen] = useState(false);
  const [teamDialogOpen, setTeamDialogOpen] = useState(false);
  const [newKbTeamSpaceId, setNewKbTeamSpaceId] = useState<string | null>(null);
  const [manageProjectId, setManageProjectId] = useState<string | null>(null);
  const [manageTeamId, setManageTeamId] = useState<string | null>(null);

  if (spaces.isLoading) return <p>Loading…</p>;
  if (!org) return <p>Access denied or organization not found.</p>;

  const isAdmin = org.role === "org_admin";
  const allProjects = projects.data ?? [];
  const publicProjects = allProjects.filter((p) => p.spaceKind === "org");
  const teamList = teams.data?.teams ?? [];

  const leaderTeamIds = new Set(
    (spaces.data?.teams ?? [])
      .filter((t) => t.orgId === orgId && t.role === "leader")
      .map((t) => t.id),
  );

  const openManage = (projectId: string, teamId: string | null) => {
    setManageProjectId(projectId);
    setManageTeamId(teamId);
  };

  return (
    <div className="flex flex-col gap-6">
      <header className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">{org.name} workspace</h1>
        {isAdmin ? (
          <div className="flex items-center gap-2">
            <Button type="button" onClick={() => setPublicDialogOpen(true)}>
              New public project
            </Button>
            <Link to={`/orgs/${orgId}/members`}>Members</Link>
            <Button type="button" onClick={() => setTeamDialogOpen(true)}>
              New team
            </Button>
          </div>
        ) : (
          <Button type="button" onClick={() => setTeamDialogOpen(true)}>
            New team
          </Button>
        )}
      </header>

      <section>
        <h2 className="text-sm font-medium">Public projects</h2>
        {publicProjects.length === 0 ? (
          <p className="text-sm text-muted-foreground">No public projects yet.</p>
        ) : (
          <ul className="flex flex-col gap-1">
            {publicProjects.map((project) => (
              <li key={project.id} className="flex items-center justify-between">
                <Link to={`/projects/${project.id}`}>{project.name}</Link>
                {isAdmin ? (
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => openManage(project.id, null)}
                  >
                    Manage access
                  </Button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </section>

      {teamList.map((team) => {
        const teamProjects = allProjects.filter((p) => p.teamId === team.id);
        const canManageTeam = isAdmin || leaderTeamIds.has(team.id);
        const teamSpaceId = (spaces.data?.teams ?? []).find((t) => t.id === team.id)?.spaceId ?? "";
        return (
          <section key={team.id}>
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-medium">Team · {team.name}</h2>
              <div className="flex items-center gap-2">
                {canManageTeam && teamSpaceId ? (
                  <Button
                    type="button"
                    onClick={() => setNewKbTeamSpaceId(teamSpaceId)}
                  >
                    New team KB
                  </Button>
                ) : null}
                <Link to={`/orgs/${orgId}/teams/${team.id}`}>Manage</Link>
              </div>
            </div>
            {teamProjects.length === 0 ? (
              <p className="text-sm text-muted-foreground">No team KBs yet.</p>
            ) : (
              <ul className="flex flex-col gap-1">
                {teamProjects.map((project) => (
                  <li key={project.id} className="flex items-center justify-between">
                    <Link to={`/projects/${project.id}`}>{project.name}</Link>
                    {canManageTeam ? (
                      <Button
                        type="button"
                        variant="outline"
                        onClick={() => openManage(project.id, team.id)}
                      >
                        Manage access
                      </Button>
                    ) : null}
                  </li>
                ))}
              </ul>
            )}
          </section>
        );
      })}

      <CreatePublicProjectDialog
        targetSpaceId={orgSpaceId}
        listSpaceId={orgSpaceId}
        title="New public project"
        open={publicDialogOpen}
        onOpenChange={setPublicDialogOpen}
      />
      <CreatePublicProjectDialog
        targetSpaceId={newKbTeamSpaceId ?? ""}
        listSpaceId={orgSpaceId}
        title="New team KB"
        open={Boolean(newKbTeamSpaceId)}
        onOpenChange={(open) => {
          if (!open) setNewKbTeamSpaceId(null);
        }}
      />
      <CreateTeamDialog
        orgId={orgId}
        open={teamDialogOpen}
        onOpenChange={setTeamDialogOpen}
      />
      {manageProjectId ? (
        <ManageAccessDialog
          projectId={manageProjectId}
          spaceKind={manageTeamId ? "team" : "org"}
          orgId={orgId}
          teamId={manageTeamId}
          open={Boolean(manageProjectId)}
          onOpenChange={(open) => {
            if (!open) {
              setManageProjectId(null);
              setManageTeamId(null);
            }
          }}
        />
      ) : null}
    </div>
  );
}
```

> The "New team KB" button sets `newKbTeamSpaceId` to the team's space id, which opens the
> second `CreatePublicProjectDialog` (its `targetSpaceId` is the team space, `listSpaceId` is
> the org space so the page's `["org-projects", orgSpaceId]` list refreshes). Both dialogs
> share one component because the endpoint is identical and only the target space differs.

- [ ] **Step 8: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- src/features/orgs/workspace-page.test.tsx`
Expected: PASS (both cases).

- [ ] **Step 9: Verify the build**

Run: `npm run build --workspace @knowledge/admin`
Expected: build succeeds (no unresolved references; both create dialogs type-check).

- [ ] **Step 10: Commit**

```bash
git add apps/admin/src/features/orgs/workspace-queries.ts apps/admin/src/features/orgs/workspace-mutations.ts apps/admin/src/features/orgs/create-public-project-dialog.tsx apps/admin/src/features/orgs/create-team-dialog.tsx apps/admin/src/features/orgs/workspace-page.tsx apps/admin/src/features/orgs/workspace-page.test.tsx
git commit -m "feat(admin): org workspace page with stacked KB sections"
```

---

### Task 10: Org members page

**Files:**
- Create: `apps/admin/src/features/orgs/members-queries.ts`
- Create: `apps/admin/src/features/orgs/members-mutations.ts`
- Create: `apps/admin/src/features/orgs/members-page.tsx`
- Create: `apps/admin/src/features/orgs/members-page.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/features/orgs/members-page.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

let callerRole = "org_admin";
vi.mock("../spaces/use-spaces", () => ({
  useSpacesQuery: () => ({
    data: {
      personal: { spaceId: "p" },
      orgs: [{ id: "org-1", slug: "acme", name: "Acme", spaceId: "s1", role: callerRole }],
      teams: [],
    },
    isLoading: false,
  }),
}));

vi.mock("./members-queries", () => ({
  useOrgMembersQuery: () => ({
    data: {
      members: [
        { userId: "u1", username: "alice", role: "org_admin" },
        { userId: "u2", username: "bob", role: "org_member" },
      ],
    },
    isLoading: false,
  }),
}));

vi.mock("./members-mutations", () => ({
  useAddOrgMemberMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useSetOrgMemberRoleMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useRemoveOrgMemberMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

import { OrgMembersPage } from "./members-page";

function renderPage() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/orgs/org-1/members"]}>
        <Routes>
          <Route path="/orgs/:orgId/members" element={<OrgMembersPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("OrgMembersPage", () => {
  it("renders members for any member", () => {
    callerRole = "org_member";
    renderPage();
    expect(screen.getByText("alice")).toBeInTheDocument();
    expect(screen.getByText("bob")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Add member/i })).toBeNull();
  });

  it("shows admin controls for an org_admin", () => {
    callerRole = "org_admin";
    renderPage();
    expect(screen.getByRole("button", { name: /Add member/i })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- src/features/orgs/members-page.test.tsx`
Expected: FAIL — `./members-page` not found.

- [ ] **Step 3: Implement members-queries**

Create `apps/admin/src/features/orgs/members-queries.ts`:

```ts
import { useQuery } from "@tanstack/react-query";
import { fetchOrgMembers } from "../shared/tenancy-api";

export function useOrgMembersQuery(orgId: string) {
  return useQuery({
    queryKey: ["org-members", orgId],
    queryFn: () => fetchOrgMembers(orgId),
    enabled: Boolean(orgId),
  });
}
```

- [ ] **Step 4: Implement members-mutations**

Create `apps/admin/src/features/orgs/members-mutations.ts`:

```ts
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  addOrgMember,
  setOrgMemberRole,
  removeOrgMember,
} from "../shared/tenancy-api";

export function useAddOrgMemberMutation(orgId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: {
      usernameOrEmail: string;
      role: "org_admin" | "org_member";
    }) => addOrgMember(orgId, input),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["org-members", orgId] });
    },
  });
}

export function useSetOrgMemberRoleMutation(orgId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: {
      userId: string;
      role: "org_admin" | "org_member";
    }) => setOrgMemberRole(orgId, input.userId, input.role),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["org-members", orgId] });
    },
  });
}

export function useRemoveOrgMemberMutation(orgId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { userId: string }) =>
      removeOrgMember(orgId, input.userId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["org-members", orgId] });
    },
  });
}
```

- [ ] **Step 5: Implement members-page**

Create `apps/admin/src/features/orgs/members-page.tsx`:

```tsx
import { useState } from "react";
import { useParams } from "react-router-dom";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { useSpacesQuery } from "../spaces/use-spaces";
import { useOrgMembersQuery } from "./members-queries";
import {
  useAddOrgMemberMutation,
  useSetOrgMemberRoleMutation,
  useRemoveOrgMemberMutation,
} from "./members-mutations";

export function OrgMembersPage() {
  const { orgId = "" } = useParams();
  const spaces = useSpacesQuery();
  const org = spaces.data?.orgs.find((entry) => entry.id === orgId);
  const isAdmin = org?.role === "org_admin";

  const members = useOrgMembersQuery(orgId);
  const addMember = useAddOrgMemberMutation(orgId);
  const setRole = useSetOrgMemberRoleMutation(orgId);
  const removeMember = useRemoveOrgMemberMutation(orgId);

  const [username, setUsername] = useState("");
  const [role, setRoleValue] = useState<"org_admin" | "org_member">("org_member");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  if (spaces.isLoading) return <p>Loading…</p>;
  if (!org) return <p>Access denied or organization not found.</p>;

  const add = async () => {
    setErrorMessage(null);
    try {
      await addMember.mutateAsync({ usernameOrEmail: username.trim(), role });
      setUsername("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to add member");
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <h1 className="text-lg font-semibold">{org.name} members</h1>

      <table>
        <thead>
          <tr>
            <th>Username</th>
            <th>Role</th>
            {isAdmin ? <th>Actions</th> : null}
          </tr>
        </thead>
        <tbody>
          {members.data?.members.map((member) => (
            <tr key={member.userId}>
              <td>{member.username}</td>
              <td>
                {isAdmin ? (
                  <select
                    aria-label={`Role for ${member.username}`}
                    value={member.role}
                    onChange={(event) =>
                      setRole.mutateAsync({
                        userId: member.userId,
                        role: event.target.value as "org_admin" | "org_member",
                      })
                    }
                  >
                    <option value="org_admin">org_admin</option>
                    <option value="org_member">org_member</option>
                  </select>
                ) : (
                  member.role
                )}
              </td>
              {isAdmin ? (
                <td>
                  <Button
                    type="button"
                    variant="outline"
                    aria-label={`Remove ${member.username}`}
                    onClick={() => removeMember.mutateAsync({ userId: member.userId })}
                  >
                    Remove
                  </Button>
                </td>
              ) : null}
            </tr>
          ))}
        </tbody>
      </table>

      {isAdmin ? (
        <div className="flex items-end gap-2">
          <Input
            placeholder="Username"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
          />
          <select
            aria-label="New member role"
            value={role}
            onChange={(event) =>
              setRoleValue(event.target.value as "org_admin" | "org_member")
            }
          >
            <option value="org_member">org_member</option>
            <option value="org_admin">org_admin</option>
          </select>
          <Button type="button" onClick={add} disabled={addMember.isPending}>
            Add member
          </Button>
        </div>
      ) : null}

      {errorMessage ? (
        <p className="text-sm text-destructive">{errorMessage}</p>
      ) : null}
    </div>
  );
}
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- src/features/orgs/members-page.test.tsx`
Expected: PASS (both cases).

- [ ] **Step 7: Commit**

```bash
git add apps/admin/src/features/orgs/members-queries.ts apps/admin/src/features/orgs/members-mutations.ts apps/admin/src/features/orgs/members-page.tsx apps/admin/src/features/orgs/members-page.test.tsx
git commit -m "feat(admin): org members page with admin controls"
```

---

### Task 11: Team page

**Files:**
- Create: `apps/admin/src/features/teams/team-queries.ts`
- Create: `apps/admin/src/features/teams/team-mutations.ts`
- Create: `apps/admin/src/features/teams/team-page.tsx`
- Create: `apps/admin/src/features/teams/team-page.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `apps/admin/src/features/teams/team-page.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

vi.mock("../spaces/use-spaces", () => ({
  useSpacesQuery: () => ({
    data: {
      personal: { spaceId: "p" },
      orgs: [{ id: "org-1", slug: "acme", name: "Acme", spaceId: "s1", role: "org_member" }],
      teams: [
        { id: "team-1", orgId: "org-1", slug: "platform", name: "Platform", spaceId: "ts1", role: "leader" },
      ],
    },
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
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- src/features/teams/team-page.test.tsx`
Expected: FAIL — `./team-page` not found.

- [ ] **Step 3: Implement team-queries**

Create `apps/admin/src/features/teams/team-queries.ts`:

```ts
import { useQuery } from "@tanstack/react-query";
import { fetchTeamMembers, fetchOrgProjects } from "../shared/tenancy-api";

export function useTeamMembersQuery(orgId: string, teamId: string) {
  return useQuery({
    queryKey: ["team-members", orgId, teamId],
    queryFn: () => fetchTeamMembers(orgId, teamId),
    enabled: Boolean(orgId) && Boolean(teamId),
  });
}

export function useTeamProjectsQuery(teamSpaceId: string, teamId: string) {
  return useQuery({
    queryKey: ["team-projects", teamSpaceId],
    queryFn: async () => {
      const result = await fetchOrgProjects(teamSpaceId);
      return result.filter((project) => project.teamId === teamId);
    },
    enabled: Boolean(teamSpaceId),
  });
}
```

- [ ] **Step 4: Implement team-mutations**

Create `apps/admin/src/features/teams/team-mutations.ts`:

```ts
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  addTeamMember,
  removeTeamMember,
  createSpaceProject,
} from "../shared/tenancy-api";

export function useAddTeamMemberMutation(orgId: string, teamId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { usernameOrEmail: string }) =>
      addTeamMember(orgId, teamId, input),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["team-members", orgId, teamId],
      });
    },
  });
}

export function useRemoveTeamMemberMutation(orgId: string, teamId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { userId: string }) =>
      removeTeamMember(orgId, teamId, input.userId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["team-members", orgId, teamId],
      });
    },
  });
}

export function useCreateTeamKbMutation(teamSpaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { name: string }) =>
      createSpaceProject({ name: input.name, spaceId: teamSpaceId }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["team-projects", teamSpaceId],
      });
    },
  });
}
```

- [ ] **Step 5: Implement team-page**

Create `apps/admin/src/features/teams/team-page.tsx`:

```tsx
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { useSpacesQuery } from "../spaces/use-spaces";
import { useTeamMembersQuery, useTeamProjectsQuery } from "./team-queries";
import {
  useAddTeamMemberMutation,
  useRemoveTeamMemberMutation,
  useCreateTeamKbMutation,
} from "./team-mutations";
import { ManageAccessDialog } from "../kb-access/manage-access-dialog";

export function TeamPage() {
  const { orgId = "", teamId = "" } = useParams();
  const spaces = useSpacesQuery();
  const org = spaces.data?.orgs.find((entry) => entry.id === orgId);
  const team = spaces.data?.teams.find((entry) => entry.id === teamId);
  const teamSpaceId = team?.spaceId ?? "";

  const isAdmin = org?.role === "org_admin";
  const isLeader = team?.role === "leader";
  const canManage = Boolean(isAdmin || isLeader);

  const members = useTeamMembersQuery(orgId, teamId);
  const projects = useTeamProjectsQuery(teamSpaceId, teamId);
  const addMember = useAddTeamMemberMutation(orgId, teamId);
  const removeMember = useRemoveTeamMemberMutation(orgId, teamId);
  const createKb = useCreateTeamKbMutation(teamSpaceId);

  const [username, setUsername] = useState("");
  const [kbName, setKbName] = useState("");
  const [manageProjectId, setManageProjectId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  if (spaces.isLoading) return <p>Loading…</p>;
  if (!team) return <p>Access denied or team not found.</p>;

  const add = async () => {
    setErrorMessage(null);
    try {
      await addMember.mutateAsync({ usernameOrEmail: username.trim() });
      setUsername("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to add member");
    }
  };

  const addKb = async () => {
    setErrorMessage(null);
    try {
      await createKb.mutateAsync({ name: kbName.trim() });
      setKbName("");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : "Failed to create KB");
    }
  };

  return (
    <div className="flex flex-col gap-6">
      <header className="flex items-center justify-between">
        <h1 className="text-lg font-semibold">Team · {team.name}</h1>
        <Link to={`/orgs/${orgId}`}>Back to workspace</Link>
      </header>

      <section className="flex flex-col gap-2">
        <h2 className="text-sm font-medium">Members</h2>
        <table>
          <thead>
            <tr>
              <th>Username</th>
              <th>Role</th>
              {canManage ? <th>Actions</th> : null}
            </tr>
          </thead>
          <tbody>
            {members.data?.members.map((member) => (
              <tr key={member.userId}>
                <td>{member.username}</td>
                <td>{member.role}</td>
                {canManage ? (
                  <td>
                    {member.role === "leader" ? null : (
                      <Button
                        type="button"
                        variant="outline"
                        aria-label={`Remove ${member.username}`}
                        onClick={() => removeMember.mutateAsync({ userId: member.userId })}
                      >
                        Remove
                      </Button>
                    )}
                  </td>
                ) : null}
              </tr>
            ))}
          </tbody>
        </table>
        {canManage ? (
          <div className="flex items-end gap-2">
            <Input
              placeholder="Username"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
            />
            <Button type="button" onClick={add} disabled={addMember.isPending}>
              Add member
            </Button>
          </div>
        ) : null}
      </section>

      <section className="flex flex-col gap-2">
        <h2 className="text-sm font-medium">Team KBs</h2>
        {(projects.data ?? []).length === 0 ? (
          <p className="text-sm text-muted-foreground">No team KBs yet.</p>
        ) : (
          <ul className="flex flex-col gap-1">
            {projects.data?.map((project) => (
              <li key={project.id} className="flex items-center justify-between">
                <Link to={`/projects/${project.id}`}>{project.name}</Link>
                {canManage ? (
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => setManageProjectId(project.id)}
                  >
                    Manage access
                  </Button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
        {canManage ? (
          <div className="flex items-end gap-2">
            <Input
              placeholder="New team KB name"
              value={kbName}
              onChange={(event) => setKbName(event.target.value)}
            />
            <Button type="button" onClick={addKb} disabled={createKb.isPending}>
              New team KB
            </Button>
          </div>
        ) : null}
      </section>

      {errorMessage ? (
        <p className="text-sm text-destructive">{errorMessage}</p>
      ) : null}

      {manageProjectId ? (
        <ManageAccessDialog
          projectId={manageProjectId}
          spaceKind="team"
          orgId={orgId}
          teamId={teamId}
          open={Boolean(manageProjectId)}
          onOpenChange={(open) => {
            if (!open) setManageProjectId(null);
          }}
        />
      ) : null}
    </div>
  );
}
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- src/features/teams/team-page.test.tsx`
Expected: PASS (both cases).

- [ ] **Step 7: Commit**

```bash
git add apps/admin/src/features/teams
git commit -m "feat(admin): team page with members and team KBs"
```

---

### Task 12: Router wiring (LAST — imports all pages)

**Files:**
- Modify: `apps/admin/src/app/router.tsx`

- [ ] **Step 1: Add imports and routes**

In `apps/admin/src/app/router.tsx`, add imports near the other page imports:

```tsx
import { OrgWorkspacePage } from "../features/orgs/workspace-page";
import { OrgMembersPage } from "../features/orgs/members-page";
import { TeamPage } from "../features/teams/team-page";
```

Add three routes inside the `<Route path="/" element={<AppShell />}>` block, alongside the existing `projects` routes:

```tsx
<Route path="orgs/:orgId" element={<OrgWorkspacePage />} />
<Route path="orgs/:orgId/members" element={<OrgMembersPage />} />
<Route path="orgs/:orgId/teams/:teamId" element={<TeamPage />} />
```

> Match the exact JSX style of the existing routes (self-closing `<Route ... />` vs nested). The `members-page` export is `OrgMembersPage` and the `workspace-page` export is `OrgWorkspacePage` — verify these names match Tasks 9 and 10.

- [ ] **Step 2: Verify the build**

Run: `npm run build --workspace @knowledge/admin`
Expected: build succeeds (all three pages resolve).

- [ ] **Step 3: Run the full admin test suite**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS (all existing + new tests green).

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/app/router.tsx
git commit -m "feat(admin): mount tenancy routes under AppShell"
```

---

## Final verification

- [ ] **Run the api-client tests**

Run: `npm run test --workspace @knowledge/api-client`
Expected: PASS.

- [ ] **Run the full admin suite + build**

Run: `npm run test --workspace @knowledge/admin && npm run build --workspace @knowledge/admin`
Expected: PASS + build succeeds.

- [ ] **Run the backend integration tests**

Run: `cargo test -p rust-integration --test tenancy_admin_api -- --test-threads=1`
Expected: PASS (all 9 tests: 2 list-org-members, 4 patch-role, 2 list-team-members, 1 enriched-project-members).

- [ ] **Run the backend unit/build check**

Run: `cargo build -p knowledge-server`
Expected: compiles cleanly.

When all boxes are checked, finish with the **superpowers:finishing-a-development-branch** skill.
