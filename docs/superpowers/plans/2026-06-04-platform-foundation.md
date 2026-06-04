# Platform Foundation Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bootstrap the repository into a working Rust and JS monorepo that supports login, user and project management, project membership, and a basic admin shell.

**Architecture:** The foundation uses a Rust workspace with `knowledge-core`, `knowledge-server`, and `knowledge-worker` crates. `knowledge-server` owns the HTTP surface, cookie auth, SQLite persistence, and project membership APIs; `knowledge-core` owns path-safe project registration primitives; `knowledge-worker` starts as an in-process stub that exposes future task wiring without implementing ingest yet. A Vite-based React admin app and a typed API client consume the server over cookie-authenticated REST APIs.

**Tech Stack:** Rust 2024, axum, tokio, serde, sqlx with SQLite, argon2, tower-http, tracing, uuid, time, React, TypeScript, Vite, React Router, TanStack Query, Zod, Vitest, Playwright

---

Use `@superpowers:test-driven-development` for behavior-bearing tasks and `@superpowers:verification-before-completion` before each commit or chunk handoff.

## Chunk 1: Workspace And Backend Foundation

**Chunk goal:** Establish the workspace, backend crate boundaries, database bootstrap, auth primitives, and project registration APIs.

**Planned file structure:**

- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `package.json`
- Create: `apps/admin/package.json`
- Create: `apps/admin/tsconfig.json`
- Create: `apps/admin/vite.config.ts`
- Create: `crates/knowledge-core/Cargo.toml`
- Create: `crates/knowledge-core/src/lib.rs`
- Create: `crates/knowledge-core/src/project/mod.rs`
- Create: `crates/knowledge-core/src/project/root.rs`
- Create: `crates/knowledge-server/Cargo.toml`
- Create: `crates/knowledge-server/src/main.rs`
- Create: `crates/knowledge-server/src/lib.rs`
- Create: `crates/knowledge-server/src/app/mod.rs`
- Create: `crates/knowledge-server/src/app/state.rs`
- Create: `crates/knowledge-server/src/config.rs`
- Create: `crates/knowledge-server/src/http/mod.rs`
- Create: `crates/knowledge-server/src/http/router.rs`
- Create: `crates/knowledge-server/src/http/error.rs`
- Create: `crates/knowledge-server/src/http/response.rs`
- Create: `crates/knowledge-server/src/auth/mod.rs`
- Create: `crates/knowledge-server/src/auth/password.rs`
- Create: `crates/knowledge-server/src/auth/session.rs`
- Create: `crates/knowledge-server/src/auth/routes.rs`
- Create: `crates/knowledge-server/src/users/mod.rs`
- Create: `crates/knowledge-server/src/users/routes.rs`
- Create: `crates/knowledge-server/src/projects/mod.rs`
- Create: `crates/knowledge-server/src/projects/routes.rs`
- Create: `crates/knowledge-server/src/projects/service.rs`
- Create: `crates/knowledge-server/src/db/mod.rs`
- Create: `crates/knowledge-server/src/db/pool.rs`
- Create: `crates/knowledge-server/src/db/migrate.rs`
- Create: `crates/knowledge-server/migrations/0001_init.sql`
- Create: `crates/knowledge-worker/Cargo.toml`
- Create: `crates/knowledge-worker/src/lib.rs`
- Create: `tests/rust-integration/Cargo.toml`
- Create: `tests/rust-integration/tests/auth_api.rs`
- Create: `tests/rust-integration/tests/project_api.rs`

### Task 1: Bootstrap Workspace Layout

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `package.json`
- Create: `apps/admin/package.json`
- Create: `apps/admin/tsconfig.json`
- Create: `apps/admin/vite.config.ts`
- Create: `crates/knowledge-core/Cargo.toml`
- Create: `crates/knowledge-core/src/lib.rs`
- Create: `crates/knowledge-server/Cargo.toml`
- Create: `crates/knowledge-server/src/lib.rs`
- Create: `crates/knowledge-server/src/main.rs`
- Create: `crates/knowledge-worker/Cargo.toml`
- Create: `crates/knowledge-worker/src/lib.rs`
- Create: `tests/rust-integration/Cargo.toml`

- [ ] **Step 1: Create the root Cargo workspace manifest**

```toml
[workspace]
members = [
  "crates/knowledge-core",
  "crates/knowledge-server",
  "crates/knowledge-worker",
  "tests/rust-integration",
]
resolver = "3"

[workspace.package]
edition = "2024"
license = "MIT"
version = "0.1.0"

[workspace.dependencies]
anyhow = "1.0"
argon2 = "0.5"
axum = { version = "0.8", features = ["macros"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio-rustls", "macros", "migrate", "uuid", "time"] }
tokio = { version = "1.48", features = ["macros", "rt-multi-thread", "signal", "fs"] }
tracing = "0.1"
uuid = { version = "1.18", features = ["serde", "v4"] }
```

- [ ] **Step 2: Create the root Node workspace manifest and baseline scripts**

```json
{
  "name": "knowledge",
  "private": true,
  "workspaces": [
    "apps/*",
    "packages/*",
    "tests/*"
  ],
  "scripts": {
    "dev": "npm run dev --workspace @knowledge/admin",
    "build": "npm run build --workspace @knowledge/admin && cargo build --workspace",
    "test": "npm run test --workspace @knowledge/admin && cargo test --workspace",
    "lint": "npm run lint --workspace @knowledge/admin && cargo clippy --workspace --all-targets -- -D warnings"
  }
}
```

- [ ] **Step 3: Add toolchain and ignore rules**

Run:

```powershell
New-Item -ItemType Directory -Force apps/admin, crates/knowledge-core/src, crates/knowledge-server/src, crates/knowledge-worker/src, tests/rust-integration/tests
```

Expected: the directories exist and `git status --short` shows only new scaffold files.

- [ ] **Step 4: Create minimal crate entry points**

```rust
// crates/knowledge-core/src/lib.rs
pub mod project;

// crates/knowledge-worker/src/lib.rs
pub struct WorkerStub;

// crates/knowledge-server/src/main.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    knowledge_server::run().await
}

// tests/rust-integration/Cargo.toml
[package]
name = "rust-integration"
version = "0.1.0"
edition = "2024"

[dev-dependencies]
axum = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
```

- [ ] **Step 5: Verify the skeleton compiles**

Run:

```powershell
cargo check --workspace
```

Expected: `Finished` or `Checking` output with no errors.

- [ ] **Step 6: Commit the scaffold**

Run:

```powershell
git add Cargo.toml rust-toolchain.toml .gitignore package.json apps crates
git commit -m "chore: scaffold workspace foundation"
```

### Task 2: Add Path-Safe Project Root Primitives

**Files:**
- Create: `crates/knowledge-core/src/project/mod.rs`
- Create: `crates/knowledge-core/src/project/root.rs`
- Modify: `crates/knowledge-core/src/lib.rs`
- Test: `crates/knowledge-core/src/project/root.rs`

- [ ] **Step 1: Write the failing project-root tests**

```rust
#[cfg(test)]
mod tests {
    use super::ProjectRoot;

    #[test]
    fn rejects_relative_project_root() {
        assert!(ProjectRoot::new("relative/path").is_err());
    }

    #[test]
    fn rejects_missing_directory() {
        assert!(ProjectRoot::new("Z:/definitely-missing").is_err());
    }
}
```

- [ ] **Step 2: Run the tests to confirm the API is missing**

Run:

```powershell
cargo test -p knowledge-core rejects_relative_project_root -- --exact
```

Expected: FAIL with unresolved `ProjectRoot` items.

- [ ] **Step 3: Implement the minimal project root type**

```rust
#[derive(Debug, Clone)]
pub struct ProjectRoot(PathBuf);

impl ProjectRoot {
    pub fn new(input: impl AsRef<Path>) -> Result<Self, ProjectRootError> {
        let path = input.as_ref();
        if !path.is_absolute() {
            return Err(ProjectRootError::NotAbsolute);
        }
        let canonical = path.canonicalize().map_err(ProjectRootError::Io)?;
        if !canonical.is_dir() {
            return Err(ProjectRootError::NotDirectory);
        }
        Ok(Self(canonical))
    }
}
```

- [ ] **Step 4: Add one path traversal guard helper for later API work**

```rust
pub fn safe_join(&self, relative: &str) -> Result<PathBuf, ProjectRootError> {
    let joined = self.0.join(relative);
    let normalized = joined.components().fold(PathBuf::new(), |mut acc, part| {
        match part {
            Component::ParentDir => {
                acc.pop();
            }
            Component::CurDir => {}
            other => acc.push(other.as_os_str()),
        }
        acc
    });
    if !normalized.starts_with(&self.0) {
        return Err(ProjectRootError::EscapesRoot);
    }
    Ok(normalized)
}
```

- [ ] **Step 5: Re-run the core tests**

Run:

```powershell
cargo test -p knowledge-core
```

Expected: PASS for the new `project::root` tests.

- [ ] **Step 6: Commit the core path primitives**

Run:

```powershell
git add crates/knowledge-core
git commit -m "feat: add project root safety primitives"
```

### Task 3: Bootstrap SQLite, Migrations, And App State

**Files:**
- Create: `crates/knowledge-server/src/app/mod.rs`
- Create: `crates/knowledge-server/src/app/state.rs`
- Create: `crates/knowledge-server/src/config.rs`
- Create: `crates/knowledge-server/src/db/mod.rs`
- Create: `crates/knowledge-server/src/db/pool.rs`
- Create: `crates/knowledge-server/src/db/migrate.rs`
- Create: `crates/knowledge-server/src/http/mod.rs`
- Create: `crates/knowledge-server/src/http/router.rs`
- Create: `crates/knowledge-server/migrations/0001_init.sql`
- Modify: `crates/knowledge-server/src/lib.rs`
- Test: `tests/rust-integration/tests/auth_api.rs`

- [ ] **Step 1: Write the failing database bootstrap test**

```rust
#[tokio::test]
async fn bootstraps_schema_on_start() {
    let app = test_app().await;
    let response = app.get("/api/health").await;
    assert_eq!(response.status(), 200);
    assert!(database_has_table("users").await);
}
```

- [ ] **Step 2: Run the integration test to confirm startup is incomplete**

Run:

```powershell
cargo test -p rust-integration bootstraps_schema_on_start -- --exact
```

Expected: FAIL because the app bootstrap helper or migrations are missing.

- [ ] **Step 3: Create the initial schema migration**

```sql
CREATE TABLE users (
  id TEXT PRIMARY KEY NOT NULL,
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  role TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE sessions (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  csrf_token TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE projects (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  root_path TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE TABLE project_members (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  role TEXT NOT NULL,
  can_import INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  UNIQUE(project_id, user_id),
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);
```

- [ ] **Step 4: Implement config, pool creation, and migration runner**

```rust
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub session_ttl_hours: u64,
}

pub async fn connect_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .map_err(Into::into)
}

pub fn build_router() -> Router {
    Router::new().route("/api/health", get(|| async { StatusCode::OK }))
}
```

- [ ] **Step 5: Wire app state into `knowledge_server::run`**

Run:

```powershell
cargo test -p rust-integration bootstraps_schema_on_start -- --exact
```

Expected: PASS and the integration harness confirms migrations ran.

- [ ] **Step 6: Commit the bootstrap layer**

Run:

```powershell
git add crates/knowledge-server tests/rust-integration
git commit -m "feat: add server bootstrap and schema migration"
```

### Task 4: Implement Auth Password And Session Flows

**Files:**
- Create: `crates/knowledge-server/src/auth/mod.rs`
- Create: `crates/knowledge-server/src/auth/password.rs`
- Create: `crates/knowledge-server/src/auth/session.rs`
- Create: `crates/knowledge-server/src/auth/routes.rs`
- Create: `crates/knowledge-server/src/http/error.rs`
- Create: `crates/knowledge-server/src/http/response.rs`
- Modify: `crates/knowledge-server/src/http/mod.rs`
- Modify: `crates/knowledge-server/src/http/router.rs`
- Modify: `crates/knowledge-server/src/lib.rs`
- Test: `tests/rust-integration/tests/auth_api.rs`

- [ ] **Step 1: Write failing auth API tests**

```rust
#[tokio::test]
async fn login_sets_session_cookie_and_csrf_token() {
    let app = seeded_app_with_admin().await;
    let response = app
        .post("/api/auth/login")
        .json(&serde_json::json!({
            "username": "admin",
            "password": "secret-password"
        }))
        .await;
    assert_eq!(response.status(), 200);
    assert!(response.cookie("knowledge_session").is_some());
    assert!(response.json()["csrfToken"].as_str().is_some());
}

#[tokio::test]
async fn me_requires_valid_session() {
    let app = test_app().await;
    let response = app.get("/api/auth/me").await;
    assert_eq!(response.status(), 401);
}
```

- [ ] **Step 2: Run the auth tests to verify failure**

Run:

```powershell
cargo test -p rust-integration login_sets_session_cookie_and_csrf_token -- --exact
```

Expected: FAIL because the auth router and session middleware do not exist yet.

- [ ] **Step 3: Implement password hashing and verification**

```rust
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHash)
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, AuthError> {
    let parsed = PasswordHash::new(hash).map_err(|_| AuthError::PasswordHash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}
```

- [ ] **Step 4: Implement session issuance and auth routes**

```rust
Router::new()
    .route("/api/auth/login", post(login))
    .route("/api/auth/logout", post(logout))
    .route("/api/auth/me", get(me))
```

- [ ] **Step 5: Re-run the auth integration suite**

Run:

```powershell
cargo test -p rust-integration --test auth_api
```

Expected: PASS for login, logout, and `me` behaviors.

- [ ] **Step 6: Commit the auth layer**

Run:

```powershell
git add crates/knowledge-server tests/rust-integration/tests/auth_api.rs
git commit -m "feat: add cookie auth and session endpoints"
```

### Task 5: Implement Users, Projects, And Membership APIs

**Files:**
- Create: `crates/knowledge-server/src/users/mod.rs`
- Create: `crates/knowledge-server/src/users/routes.rs`
- Create: `crates/knowledge-server/src/projects/mod.rs`
- Create: `crates/knowledge-server/src/projects/routes.rs`
- Create: `crates/knowledge-server/src/projects/service.rs`
- Modify: `crates/knowledge-server/src/http/router.rs`
- Modify: `crates/knowledge-server/src/app/state.rs`
- Test: `tests/rust-integration/tests/project_api.rs`

- [ ] **Step 1: Write failing API tests for project creation and membership isolation**

```rust
#[tokio::test]
async fn admin_can_create_project_with_absolute_root() {
    let app = seeded_app_with_admin().await;
    let temp = tempfile::tempdir().unwrap();
    let response = app
        .post("/api/projects")
        .session_as("admin")
        .csrf()
        .json(&serde_json::json!({
            "name": "knowledge-demo",
            "rootPath": temp.path()
        }))
        .await;
    assert_eq!(response.status(), 201);
}

#[tokio::test]
async fn non_member_cannot_list_project_members() {
    let app = seeded_app_with_users().await;
    let response = app
        .get("/api/projects/project-1/members")
        .session_as("outsider")
        .await;
    assert_eq!(response.status(), 403);
}
```

- [ ] **Step 2: Run the project API tests**

Run:

```powershell
cargo test -p rust-integration --test project_api
```

Expected: FAIL because the project and membership routes are not implemented.

- [ ] **Step 3: Implement the users routes needed by the admin shell**

```rust
Router::new()
    .route("/api/users", get(list_users).post(create_user))
    .route("/api/users/{user_id}", patch(update_user))
    .route("/api/users/{user_id}/password", post(set_password))
```

- [ ] **Step 4: Implement project registration using `knowledge_core::project::ProjectRoot`**

```rust
pub async fn create_project(input: CreateProjectInput, state: &AppState) -> Result<ProjectDto, ApiError> {
    let id = Uuid::new_v4().to_string();
    let root = ProjectRoot::new(&input.root_path)?;
    let created_at = OffsetDateTime::now_utc().format(&Rfc3339)?;
    sqlx::query!(
        "INSERT INTO projects (id, name, root_path, created_at) VALUES (?1, ?2, ?3, ?4)",
        id,
        input.name,
        root.as_str(),
        created_at
    )
    .execute(&state.pool)
    .await?;
    Ok(ProjectDto {
        id,
        name: input.name,
        root_path: root.as_str().to_owned(),
        created_at,
    })
}
```

- [ ] **Step 5: Implement project-members APIs and permission guards**

Run:

```powershell
cargo test -p rust-integration --test project_api
```

Expected: PASS for create, list, membership checks, and forbidden access cases.

- [ ] **Step 6: Verify the full backend foundation**

Run:

```powershell
cargo test -p knowledge-core
cargo test -p rust-integration
cargo clippy -p knowledge-core -p knowledge-server -p knowledge-worker --all-targets -- -D warnings
```

Expected: all commands pass with zero warnings.

- [ ] **Step 7: Commit the backend foundation APIs**

Run:

```powershell
git add crates/knowledge-core crates/knowledge-server tests/rust-integration
git commit -m "feat: add foundation auth and project APIs"
```

## Chunk 2: Admin Shell And Typed Client

**Chunk goal:** Add a typed browser client and a usable admin shell for login, dashboard navigation, users, and projects.

**Planned file structure:**

- Create: `packages/api-client/package.json`
- Create: `packages/api-client/tsconfig.json`
- Create: `packages/api-client/src/index.ts`
- Create: `packages/api-client/src/http.ts`
- Create: `packages/api-client/src/schemas.ts`
- Create: `apps/admin/index.html`
- Create: `apps/admin/src/main.tsx`
- Create: `apps/admin/src/app/router.tsx`
- Create: `apps/admin/src/app/providers.tsx`
- Create: `apps/admin/src/app/query-client.ts`
- Create: `apps/admin/src/features/auth/api.ts`
- Create: `apps/admin/src/features/auth/login-page.tsx`
- Create: `apps/admin/src/features/auth/use-session.ts`
- Create: `apps/admin/src/features/dashboard/page.tsx`
- Create: `apps/admin/src/features/projects/page.tsx`
- Create: `apps/admin/src/features/users/page.tsx`
- Create: `apps/admin/src/components/layout/app-shell.tsx`
- Create: `apps/admin/src/components/layout/projectless-state.tsx`
- Create: `apps/admin/src/styles.css`
- Create: `apps/admin/src/test/setup.ts`
- Create: `apps/admin/src/features/auth/login-page.test.tsx`
- Create: `tests/web/package.json`
- Create: `tests/web/playwright.config.ts`
- Create: `tests/web/tests/login.spec.ts`

### Task 6: Build The Typed API Client Package

**Files:**
- Create: `packages/api-client/package.json`
- Create: `packages/api-client/tsconfig.json`
- Create: `packages/api-client/src/index.ts`
- Create: `packages/api-client/src/http.ts`
- Create: `packages/api-client/src/schemas.ts`
- Test: `packages/api-client/src/http.ts`

- [ ] **Step 1: Write the failing client tests for cookie requests and schema parsing**

```ts
import { describe, expect, it } from "vitest";
import { parseCurrentUser } from "./schemas";

describe("parseCurrentUser", () => {
  it("parses a valid current-user payload", () => {
    expect(parseCurrentUser({ id: "u1", username: "admin", role: "admin" }).username).toBe("admin");
  });
});
```

- [ ] **Step 2: Run the client test to confirm the package is missing**

Run:

```powershell
npm run test --workspace @knowledge/api-client
```

Expected: FAIL because the workspace package and test script do not exist yet.

- [ ] **Step 3: Implement the fetch wrapper and zod schemas**

```ts
export async function apiFetch<T>(path: string, init: RequestInit, schema: ZodSchema<T>): Promise<T> {
  const response = await fetch(path, {
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      ...(init.headers ?? {}),
    },
    ...init,
  });
  const json = await response.json();
  return schema.parse(json);
}
```

- [ ] **Step 4: Re-run the package tests**

Run:

```powershell
npm run test --workspace @knowledge/api-client
```

Expected: PASS for the typed client unit tests.

- [ ] **Step 5: Commit the API client package**

Run:

```powershell
git add packages/api-client package.json
git commit -m "feat: add typed admin api client"
```

### Task 7: Scaffold The Admin App Shell And Login Flow

**Files:**
- Create: `apps/admin/index.html`
- Create: `apps/admin/src/main.tsx`
- Create: `apps/admin/src/app/router.tsx`
- Create: `apps/admin/src/app/providers.tsx`
- Create: `apps/admin/src/app/query-client.ts`
- Create: `apps/admin/src/features/auth/api.ts`
- Create: `apps/admin/src/features/auth/login-page.tsx`
- Create: `apps/admin/src/features/auth/use-session.ts`
- Create: `apps/admin/src/components/layout/app-shell.tsx`
- Create: `apps/admin/src/styles.css`
- Test: `apps/admin/src/features/auth/login-page.test.tsx`

- [ ] **Step 1: Write the failing login-page test**

```tsx
it("submits username and password to the login mutation", async () => {
  render(<LoginPage />);
  await user.type(screen.getByLabelText(/username/i), "admin");
  await user.type(screen.getByLabelText(/password/i), "secret-password");
  await user.click(screen.getByRole("button", { name: /sign in/i }));
  expect(mockLogin).toHaveBeenCalledWith({
    username: "admin",
    password: "secret-password",
  });
});
```

- [ ] **Step 2: Run the admin unit test to verify the page is missing**

Run:

```powershell
npm run test --workspace @knowledge/admin -- login-page.test.tsx
```

Expected: FAIL because the admin app and test harness are not created yet.

- [ ] **Step 3: Implement the router, query provider, and login page**

```tsx
const router = createBrowserRouter([
  { path: "/login", element: <LoginPage /> },
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <DashboardPage /> },
      { path: "projects", element: <ProjectsPage /> },
      { path: "users", element: <UsersPage /> },
    ],
  },
]);
```

- [ ] **Step 4: Implement session bootstrap with `/api/auth/me`**

Run:

```powershell
npm run test --workspace @knowledge/admin -- login-page.test.tsx
```

Expected: PASS for the login-page tests and route-level auth guard behavior.

- [ ] **Step 5: Commit the admin shell scaffold**

Run:

```powershell
git add apps/admin
git commit -m "feat: add admin shell and login flow"
```

### Task 8: Add Dashboard, Projects, Users Pages, And Browser Smoke Test

**Files:**
- Create: `apps/admin/src/features/dashboard/page.tsx`
- Create: `apps/admin/src/features/projects/page.tsx`
- Create: `apps/admin/src/features/users/page.tsx`
- Create: `apps/admin/src/components/layout/projectless-state.tsx`
- Create: `apps/admin/src/test/setup.ts`
- Create: `tests/web/package.json`
- Create: `tests/web/playwright.config.ts`
- Create: `tests/web/tests/login.spec.ts`

- [ ] **Step 1: Write the failing Playwright smoke test**

```ts
import { test, expect } from "@playwright/test";

test("admin can sign in and reach the projects page", async ({ page }) => {
  await page.goto("/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Password").fill("secret-password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("heading", { name: "Projects" })).toBeVisible();
});
```

- [ ] **Step 2: Run the smoke test to verify the flow is incomplete**

Run:

```powershell
npm run test --workspace @knowledge/web
```

Expected: FAIL because the dashboard pages or local test runner wiring are not complete yet.

- [ ] **Step 3: Implement the first operational pages**

```tsx
export function ProjectsPage() {
  const { data } = useProjects();
  return (
    <section>
      <h1>Projects</h1>
      {data?.length ? <ProjectTable projects={data} /> : <ProjectlessState />}
    </section>
  );
}
```

- [ ] **Step 4: Wire the frontend dev scripts and smoke test environment**

Run:

```powershell
npm run lint --workspace @knowledge/admin
npm run test --workspace @knowledge/admin
npm run test --workspace @knowledge/web
```

Expected: lint and unit tests pass; the Playwright smoke test passes against the local dev server or preview server configured in `tests/web/playwright.config.ts`.

- [ ] **Step 5: Verify the full platform foundation slice**

Run:

```powershell
npm test
cargo test --workspace
```

Expected: both commands pass; this is the final proof that the platform foundation slice is ready.

- [ ] **Step 6: Commit the admin operational pages**

Run:

```powershell
git add apps/admin tests/web package.json
git commit -m "feat: add admin foundation screens"
```
