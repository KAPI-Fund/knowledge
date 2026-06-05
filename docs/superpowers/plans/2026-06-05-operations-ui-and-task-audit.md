# Operations UI And Task Audit Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the current platform foundation into a usable phase-one operations slice with persisted task lifecycle, audit logs, graph neighbors, and project-level admin screens.

**Architecture:** Keep the current file-first knowledge model. Add PostgreSQL-backed operational read models for tasks and audit logs inside `knowledge-server`, while preserving project-local queue and checkpoint files as the canonical recovery state. Expand the React admin app from system-level listing screens into project-level operational pages that consume the new APIs.

**Tech Stack:** Rust 2024, axum, tokio, serde, sqlx/postgres, redis, React, TypeScript, TanStack Query, React Router, Vitest, Playwright

---

Use `@superpowers:test-driven-development` for behavior-bearing tasks and `@superpowers:verification-before-completion` before each commit or chunk handoff.

## Chunk 1: Operational Persistence And APIs

**Chunk goal:** Add persisted task summaries, audit logs, graph neighbors, and richer project endpoints for the admin app.

**Planned file structure:**

- Modify: `crates/knowledge-server/migrations/0001_init.sql`
- Create: `crates/knowledge-server/src/projects/tasks.rs`
- Create: `crates/knowledge-server/src/projects/audit.rs`
- Modify: `crates/knowledge-server/src/projects/mod.rs`
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `crates/knowledge-server/src/projects/service.rs`
- Modify: `crates/knowledge-core/src/project/sources.rs`
- Modify: `crates/knowledge-core/src/graph.rs`
- Test: `tests/rust-integration/tests/task_audit_api.rs`

### Task 1: Persist Task Summaries For Source And Ingest Operations

**Files:**
- Create: `crates/knowledge-server/src/projects/tasks.rs`
- Modify: `crates/knowledge-server/migrations/0001_init.sql`
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `crates/knowledge-core/src/project/sources.rs`
- Modify: `crates/knowledge-core/src/ingest.rs`
- Test: `tests/rust-integration/tests/task_audit_api.rs`

- [ ] **Step 1: Write the failing task-summary integration test**

```rust
#[tokio::test]
async fn import_and_ingest_create_persisted_task_summaries() {
  let env = TestEnvironment::start("task-summary").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("task-summary-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

  import_source_via_api(state.clone(), &cookie, &csrf, &project_id, "task.md", "IyBUYXNrCg==").await;
  ingest_source_via_api(state.clone(), &cookie, &csrf, &project_id, "raw/sources/task.md").await;

  let response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/tasks"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  let payload = read_json(response.into_body()).await;
  let tasks = payload.get("tasks").and_then(Value::as_array).unwrap();
  assert_eq!(tasks.len(), 2);
  assert_eq!(tasks[0].get("status").and_then(Value::as_str), Some("completed"));
  assert!(tasks[0].get("id").and_then(Value::as_str).is_some());
}
```

- [ ] **Step 2: Run the new test to verify it fails**

Run: `cargo test -p rust-integration import_and_ingest_create_persisted_task_summaries -- --exact`
Expected: FAIL because `tasks` is still file-backed only and has no persisted operational summary model.

- [ ] **Step 3: Add database tables for task summaries**

```sql
CREATE TABLE project_tasks (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL,
  task_type TEXT NOT NULL,
  status TEXT NOT NULL,
  title TEXT NOT NULL,
  relative_path TEXT,
  detail JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE CASCADE
);
```

- [ ] **Step 4: Implement minimal task-summary persistence helpers**

```rust
pub async fn create_completed_task(
  state: &AppState,
  input: CreateTaskRecord,
) -> Result<TaskRecord, ApiError> {
  let id = Uuid::new_v4().to_string();
  let now = now_rfc3339()?;
  sqlx::query(
    "INSERT INTO project_tasks (id, project_id, task_type, status, title, relative_path, detail, created_by, created_at, updated_at)
     VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, $8, $9, $10)"
  )
  .bind(&id)
  .bind(&input.project_id)
  .bind(&input.task_type)
  .bind("completed")
  .bind(&input.title)
  .bind(&input.relative_path)
  .bind(serde_json::to_string(&input.detail)?)
  .bind(&input.created_by)
  .bind(&now)
  .bind(&now)
  .execute(&state.pool)
  .await?;
  // ...
}
```

- [ ] **Step 5: Wire source import, rescan, delete, and ingest to persisted task summaries**

Run: `cargo test -p rust-integration import_and_ingest_create_persisted_task_summaries -- --exact`
Expected: PASS and `/api/projects/{projectId}/tasks` now returns database-backed task summaries.

- [ ] **Step 6: Commit the task summary layer**

```bash
git add crates/knowledge-server crates/knowledge-core tests/rust-integration/tests/task_audit_api.rs
git commit -m "feat: add persisted project task summaries"
```

### Task 2: Add Task Detail, Retry, And Cancel APIs

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `crates/knowledge-server/src/projects/tasks.rs`
- Test: `tests/rust-integration/tests/task_audit_api.rs`

- [ ] **Step 1: Write failing task detail and retry tests**

```rust
#[tokio::test]
async fn task_detail_and_retry_endpoints_return_operational_state() {
  // create task via import
  let detail = get_task_detail(...).await;
  assert_eq!(detail["status"], "completed");

  let retry = retry_task(...).await;
  assert_eq!(retry["status"], "queued");
}
```

- [ ] **Step 2: Run the new task API test**

Run: `cargo test -p rust-integration task_detail_and_retry_endpoints_return_operational_state -- --exact`
Expected: FAIL because the detail, retry, and cancel endpoints do not exist.

- [ ] **Step 3: Implement task detail, retry, and cancel routes**

```rust
.route("/api/projects/{project_id}/tasks/{task_id}", get(task_detail_handler))
.route("/api/projects/{project_id}/tasks/{task_id}:retry", post(retry_task_handler))
.route("/api/projects/{project_id}/tasks/{task_id}:cancel", post(cancel_task_handler))
```

- [ ] **Step 4: Use minimal status transitions without background worker orchestration**

Rules:
- completed -> queued on retry
- queued -> cancelled on cancel
- completed tasks cannot be cancelled

- [ ] **Step 5: Re-run the task API tests**

Run: `cargo test -p rust-integration --test task_audit_api`
Expected: PASS for task detail, retry, and cancel state transitions.

- [ ] **Step 6: Commit the task lifecycle endpoints**

```bash
git add crates/knowledge-server tests/rust-integration/tests/task_audit_api.rs
git commit -m "feat: add task detail retry and cancel endpoints"
```

### Task 3: Add Audit Log Persistence And Endpoints

**Files:**
- Create: `crates/knowledge-server/src/projects/audit.rs`
- Modify: `crates/knowledge-server/migrations/0001_init.sql`
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Test: `tests/rust-integration/tests/task_audit_api.rs`

- [ ] **Step 1: Write the failing audit-log test**

```rust
#[tokio::test]
async fn project_operations_are_recorded_in_audit_log() {
  // login, create project, import source
  let response = build_app(state)
    .oneshot(
      Request::builder()
        .uri(format!("/api/projects/{project_id}/audit-logs"))
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  let payload = read_json(response.into_body()).await;
  let items = payload.get("items").and_then(Value::as_array).unwrap();
  assert!(items.iter().any(|item| item.get("action").and_then(Value::as_str) == Some("project.created")));
  assert!(items.iter().any(|item| item.get("action").and_then(Value::as_str) == Some("source.imported")));
}
```

- [ ] **Step 2: Run the audit-log test to verify failure**

Run: `cargo test -p rust-integration project_operations_are_recorded_in_audit_log -- --exact`
Expected: FAIL because the table and route do not exist.

- [ ] **Step 3: Add audit log table and helper**

```sql
CREATE TABLE audit_logs (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT,
  actor_id TEXT NOT NULL,
  action TEXT NOT NULL,
  target_type TEXT NOT NULL,
  target_id TEXT NOT NULL,
  task_id TEXT,
  summary TEXT NOT NULL,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(actor_id) REFERENCES users(id) ON DELETE CASCADE
);
```

- [ ] **Step 4: Record minimal audit events for project creation, source import, source delete, ingest, review update, and settings update**

- [ ] **Step 5: Implement `GET /api/projects/{projectId}/audit-logs`**

Run: `cargo test -p rust-integration project_operations_are_recorded_in_audit_log -- --exact`
Expected: PASS with newest-first audit records.

- [ ] **Step 6: Commit the audit log layer**

```bash
git add crates/knowledge-server tests/rust-integration/tests/task_audit_api.rs
git commit -m "feat: add project audit log persistence"
```

### Task 4: Add Graph Neighbor And Project Detail APIs

**Files:**
- Modify: `crates/knowledge-core/src/graph.rs`
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Test: `tests/rust-integration/tests/task_audit_api.rs`

- [ ] **Step 1: Write failing graph-neighbor and project-detail tests**

```rust
#[tokio::test]
async fn graph_neighbors_and_project_detail_return_operational_view() {
  // seed wiki pages and a project
  let neighbors = get_json("/api/projects/{project_id}/graph/chain-of-thought/neighbors").await;
  assert_eq!(neighbors["node"]["id"], "chain-of-thought");
  assert_eq!(neighbors["neighbors"].as_array().unwrap().len(), 1);

  let project = get_json("/api/projects/{project_id}").await;
  assert_eq!(project["project"]["id"], project_id);
}
```

- [ ] **Step 2: Run the graph-neighbor test**

Run: `cargo test -p rust-integration graph_neighbors_and_project_detail_return_operational_view -- --exact`
Expected: FAIL because the routes do not exist.

- [ ] **Step 3: Add graph-neighbor helper in `knowledge-core`**

```rust
pub fn neighbors_for_node(project_root: &Path, node_id: &str) -> Result<GraphNeighborhood, std::io::Error> {
  let (nodes, edges) = build_graph(project_root)?;
  // ...
}
```

- [ ] **Step 4: Implement `GET /api/projects/{projectId}` and `GET /api/projects/{projectId}/graph/{nodeId}/neighbors`**

- [ ] **Step 5: Re-run the graph-neighbor test**

Run: `cargo test -p rust-integration --test task_audit_api`
Expected: PASS for project detail and graph neighborhood responses.

- [ ] **Step 6: Verify chunk 1**

Run:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all Rust verification passes.

- [ ] **Step 7: Commit chunk 1**

```bash
git add crates tests/rust-integration
git commit -m "feat: add operational task audit and graph APIs"
```

## Chunk 2: Project-Level Admin Operations UI

**Chunk goal:** Add a usable project operations console covering project detail, sources, tasks, reviews, audit, and settings.

**Planned file structure:**

- Modify: `apps/admin/src/app/router.tsx`
- Modify: `apps/admin/src/components/layout/app-shell.tsx`
- Modify: `apps/admin/src/features/shared/api.ts`
- Modify: `apps/admin/src/features/auth/use-session.ts`
- Modify: `apps/admin/src/styles.css`
- Create: `apps/admin/src/features/projects/detail-page.tsx`
- Create: `apps/admin/src/features/projects/detail-page.test.tsx`
- Create: `apps/admin/src/features/projects/detail-queries.ts`
- Create: `apps/admin/src/features/sources/page.tsx`
- Create: `apps/admin/src/features/sources/queries.ts`
- Create: `apps/admin/src/features/tasks/page.tsx`
- Create: `apps/admin/src/features/tasks/queries.ts`
- Create: `apps/admin/src/features/reviews/page.tsx`
- Create: `apps/admin/src/features/reviews/queries.ts`
- Create: `apps/admin/src/features/audit/page.tsx`
- Create: `apps/admin/src/features/audit/queries.ts`
- Create: `apps/admin/src/features/settings/page.tsx`
- Create: `apps/admin/src/features/settings/queries.ts`
- Modify: `tests/web/tests/login.spec.ts`
- Create: `tests/web/tests/project-operations.spec.ts`

### Task 5: Add Typed Client Support For Project Operations APIs

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts`
- Create: `apps/admin/src/features/projects/detail-queries.ts`
- Create: `apps/admin/src/features/sources/queries.ts`
- Create: `apps/admin/src/features/tasks/queries.ts`
- Create: `apps/admin/src/features/reviews/queries.ts`
- Create: `apps/admin/src/features/audit/queries.ts`
- Create: `apps/admin/src/features/settings/queries.ts`
- Test: `apps/admin/src/features/projects/detail-page.test.tsx`

- [ ] **Step 1: Write the failing project-detail page test**

```tsx
it("renders task, review, and source counts from the project detail payload", async () => {
  server.use(http.get("/api/projects/project-1", () => HttpResponse.json({
    project: {
      id: "project-1",
      name: "demo",
      rootPath: "E:/demo",
      createdAt: "2026-06-05T00:00:00Z",
      sourceCount: 2,
      taskCount: 3,
      reviewCount: 1
    }
  })));

  render(<ProjectDetailPage />);
  expect(await screen.findByText("2 sources")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the new admin test and verify failure**

Run: `npm run test --workspace @knowledge/admin -- detail-page.test.tsx`
Expected: FAIL because the project detail page and query hooks do not exist.

- [ ] **Step 3: Extend the shared API module with project operations methods**

Add:
- `getProjectDetail`
- `listProjectSources`
- `listProjectTasks`
- `getTaskDetail`
- `listProjectReviews`
- `listProjectAuditLogs`
- `getSystemSettings`
- `updateSystemSettings`

- [ ] **Step 4: Add query hooks per feature**

- [ ] **Step 5: Re-run the admin unit test**

Run: `npm run test --workspace @knowledge/admin -- detail-page.test.tsx`
Expected: PASS for the typed project detail flow.

- [ ] **Step 6: Commit the typed project operations client**

```bash
git add apps/admin/src/features
git commit -m "feat: add project operations client hooks"
```

### Task 6: Build Project Detail, Sources, Tasks, Reviews, Audit, And Settings Screens

**Files:**
- Create: `apps/admin/src/features/projects/detail-page.tsx`
- Create: `apps/admin/src/features/sources/page.tsx`
- Create: `apps/admin/src/features/tasks/page.tsx`
- Create: `apps/admin/src/features/reviews/page.tsx`
- Create: `apps/admin/src/features/audit/page.tsx`
- Create: `apps/admin/src/features/settings/page.tsx`
- Modify: `apps/admin/src/app/router.tsx`
- Modify: `apps/admin/src/components/layout/app-shell.tsx`
- Modify: `apps/admin/src/styles.css`
- Test: `apps/admin/src/features/projects/detail-page.test.tsx`

- [ ] **Step 1: Add the failing routing expectations to the existing admin tests**

```tsx
it("navigates from project list into project operations pages", async () => {
  // render router with seeded project list
  await user.click(await screen.findByRole("link", { name: "demo" }));
  expect(await screen.findByRole("heading", { name: "demo" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Tasks" })).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the admin tests to verify failure**

Run: `npm run test --workspace @knowledge/admin`
Expected: FAIL because project routes and pages do not exist.

- [ ] **Step 3: Build the project detail route tree**

Routes to add:
- `/projects/:projectId`
- `/projects/:projectId/sources`
- `/projects/:projectId/tasks`
- `/projects/:projectId/reviews`
- `/projects/:projectId/audit`
- `/settings`

- [ ] **Step 4: Implement minimal operational pages with live data**

Requirements:
- project detail summary counts
- sources table
- tasks table with status
- reviews table with status
- audit log list
- settings form for provider mode, language, query limit

- [ ] **Step 5: Re-run admin unit tests**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS with new route coverage.

- [ ] **Step 6: Commit the admin operations pages**

```bash
git add apps/admin
git commit -m "feat: add project operations admin screens"
```

### Task 7: Extend Playwright Coverage For Project Operations

**Files:**
- Modify: `tests/web/tests/login.spec.ts`
- Create: `tests/web/tests/project-operations.spec.ts`

- [ ] **Step 1: Write the failing project-operations Playwright test**

```ts
test("admin can inspect project operations data", async ({ page }) => {
  await page.goto("/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Password").fill("secret-password");
  await page.getByRole("button", { name: "Sign in" }).click();

  await page.getByRole("link", { name: "seed-project" }).click();
  await expect(page.getByRole("heading", { name: "seed-project" })).toBeVisible();
  await expect(page.getByText(/sources/i)).toBeVisible();
  await page.getByRole("link", { name: "Tasks" }).click();
  await expect(page.getByRole("heading", { name: "Tasks" })).toBeVisible();
});
```

- [ ] **Step 2: Run the Playwright suite to verify failure**

Run: `npm run test --workspace @knowledge/web`
Expected: FAIL because the project operations routes are not yet accessible from the UI.

- [ ] **Step 3: Seed a project in the Playwright server startup path**

Approach:
- rely on login bootstrap plus project creation API from the test, or
- add a deterministic first-run seed in the backend only when the `KNOWLEDGE_E2E_SEED` env var is set

Prefer the test-driven API creation path to avoid test-only backend behavior.

- [ ] **Step 4: Re-run the Playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: PASS for login and project-operations flows.

- [ ] **Step 5: Verify chunk 2**

Run:

```powershell
npm test
npm run lint
cargo test --workspace
```

Expected: JS and Rust verification passes end-to-end.

- [ ] **Step 6: Commit chunk 2**

```bash
git add apps/admin tests/web package.json
git commit -m "feat: add project operations workflows"
```
