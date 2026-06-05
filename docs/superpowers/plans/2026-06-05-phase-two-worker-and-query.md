# Phase Two Worker And Query Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a server-embedded worker that executes durable project and Query/RAG tasks, expose asynchronous query task APIs, and extend the admin UI with a task-oriented query workbench and richer task operations.

**Architecture:** Keep `knowledge-server` as the single process boundary and add an embedded scheduler plus executor registry behind the existing HTTP layer. Persist task truth in PostgreSQL, use Redis only for wake-up signaling, and route Query/RAG execution through a provider abstraction with a single OpenAI-compatible adapter. Extend the admin UI to submit async query tasks, poll task detail, and manage provider settings without introducing streaming or a separate worker process.

**Tech Stack:** Rust 2024, axum, tokio, serde, sqlx/postgres, redis, React, TypeScript, TanStack Query, React Router, Vitest, Playwright

---

Use `@superpowers:test-driven-development` for behavior-bearing tasks and `@superpowers:verification-before-completion` before each commit or chunk handoff.

## Chunk 1: Durable Task Model And Embedded Scheduler

**Chunk goal:** Introduce the durable task schema, lease-based state transitions, scheduler startup, and restart recovery inside `knowledge-server`.

**Planned file structure:**

- Modify: `crates/knowledge-server/migrations/0001_init.sql`
- Create: `crates/knowledge-server/src/tasks/mod.rs`
- Create: `crates/knowledge-server/src/tasks/model.rs`
- Create: `crates/knowledge-server/src/tasks/store.rs`
- Create: `crates/knowledge-server/src/tasks/scheduler.rs`
- Create: `crates/knowledge-server/src/tasks/recovery.rs`
- Modify: `crates/knowledge-server/src/app/state.rs`
- Modify: `crates/knowledge-server/src/cache/mod.rs`
- Modify: `crates/knowledge-server/src/lib.rs`
- Modify: `crates/knowledge-server/src/config.rs`
- Test: `tests/rust-integration/tests/task_runtime_api.rs`

### Task 1: Expand The Task Schema For Durable Execution

**Files:**
- Modify: `crates/knowledge-server/migrations/0001_init.sql`
- Create: `crates/knowledge-server/src/tasks/model.rs`
- Test: `tests/rust-integration/tests/task_runtime_api.rs`

- [ ] **Step 1: Write the failing schema-oriented integration test**

```rust
#[tokio::test]
async fn created_tasks_start_with_runtime_fields() {
  let env = TestEnvironment::start("runtime-schema").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("runtime-schema-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query-tasks"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "query": "attention", "topK": 3 }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::ACCEPTED);
  let payload = read_json(response.into_body()).await;
  let task_id = payload.get("taskId").and_then(Value::as_str).unwrap();
  let detail = get_task_detail(state, &cookie, &project_id, task_id).await;
  assert_eq!(detail["status"], "queued");
  assert_eq!(detail["attemptCount"], 0);
  assert_eq!(detail["maxAttempts"], 3);
}
```

- [ ] **Step 2: Run the new test to verify failure**

Run: `cargo test -p rust-integration created_tasks_start_with_runtime_fields -- --exact`
Expected: FAIL because the query-task endpoint and runtime fields do not exist.

- [ ] **Step 3: Add the runtime fields to `project_tasks`**

```sql
ALTER TABLE project_tasks ADD COLUMN payload JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE project_tasks ADD COLUMN result JSONB;
ALTER TABLE project_tasks ADD COLUMN error JSONB;
ALTER TABLE project_tasks ADD COLUMN attempt_count BIGINT NOT NULL DEFAULT 0;
ALTER TABLE project_tasks ADD COLUMN max_attempts BIGINT NOT NULL DEFAULT 3;
ALTER TABLE project_tasks ADD COLUMN started_at TEXT;
ALTER TABLE project_tasks ADD COLUMN finished_at TEXT;
ALTER TABLE project_tasks ADD COLUMN lease_owner TEXT;
ALTER TABLE project_tasks ADD COLUMN lease_expires_at TEXT;
ALTER TABLE project_tasks ADD COLUMN next_retry_at TEXT;
```

- [ ] **Step 4: Create a shared `TaskRecord` model with runtime metadata**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
  pub id: String,
  pub project_id: String,
  pub task_type: String,
  pub status: String,
  pub payload: Value,
  pub result: Option<Value>,
  pub error: Option<Value>,
  pub attempt_count: i64,
  pub max_attempts: i64,
  pub created_by: String,
  pub created_at: String,
  pub started_at: Option<String>,
  pub finished_at: Option<String>,
  pub lease_owner: Option<String>,
  pub lease_expires_at: Option<String>,
  pub next_retry_at: Option<String>,
}
```

- [ ] **Step 5: Re-run the schema test**

Run: `cargo test -p rust-integration created_tasks_start_with_runtime_fields -- --exact`
Expected: PASS for task defaults after route implementation stubs are in place.

- [ ] **Step 6: Commit the runtime task schema**

```bash
git add crates/knowledge-server/migrations/0001_init.sql crates/knowledge-server/src/tasks tests/rust-integration/tests/task_runtime_api.rs
git commit -m "feat: add durable runtime task fields"
```

### Task 2: Build Task Store Primitives For Queueing, Leasing, And Completion

**Files:**
- Create: `crates/knowledge-server/src/tasks/store.rs`
- Modify: `crates/knowledge-server/src/tasks/mod.rs`
- Test: `tests/rust-integration/tests/task_runtime_api.rs`

- [ ] **Step 1: Write the failing lease transition test**

```rust
#[tokio::test]
async fn scheduler_can_acquire_and_complete_a_queued_task() {
  let env = TestEnvironment::start("lease-store").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let task = create_test_task(&state, "query.answer").await;

  let leased = tasks::store::acquire_next_task(&state, "worker-1", 30).await.unwrap().unwrap();
  assert_eq!(leased.id, task.id);
  assert_eq!(leased.status, "running");

  let completed = tasks::store::complete_task(&state, &task.id, json!({ "answer": "done" })).await.unwrap();
  assert_eq!(completed.status, "succeeded");
}
```

- [ ] **Step 2: Run the lease transition test**

Run: `cargo test -p rust-integration scheduler_can_acquire_and_complete_a_queued_task -- --exact`
Expected: FAIL because the task store primitives do not exist.

- [ ] **Step 3: Implement task creation, acquisition, completion, failure, retry, and cancellation helpers**

```rust
pub async fn acquire_next_task(
  state: &AppState,
  lease_owner: &str,
  lease_seconds: i64,
) -> Result<Option<TaskRecord>, ApiError> {
  // single-statement CTE or UPDATE ... RETURNING based acquisition
}
```

- [ ] **Step 4: Ensure acquisition is atomic and ordered**

Rules:
- pick only `queued` tasks
- honor `next_retry_at`
- order by `created_at`
- set `started_at`, `lease_owner`, and `lease_expires_at`

- [ ] **Step 5: Re-run the lease transition test**

Run: `cargo test -p rust-integration scheduler_can_acquire_and_complete_a_queued_task -- --exact`
Expected: PASS with `queued -> running -> succeeded`.

- [ ] **Step 6: Commit the task store primitives**

```bash
git add crates/knowledge-server/src/tasks tests/rust-integration/tests/task_runtime_api.rs
git commit -m "feat: add task leasing and completion primitives"
```

### Task 3: Start The Embedded Scheduler And Restart Recovery

**Files:**
- Create: `crates/knowledge-server/src/tasks/scheduler.rs`
- Create: `crates/knowledge-server/src/tasks/recovery.rs`
- Modify: `crates/knowledge-server/src/app/state.rs`
- Modify: `crates/knowledge-server/src/cache/mod.rs`
- Modify: `crates/knowledge-server/src/config.rs`
- Modify: `crates/knowledge-server/src/lib.rs`
- Test: `tests/rust-integration/tests/task_runtime_api.rs`

- [ ] **Step 1: Write the failing restart recovery test**

```rust
#[tokio::test]
async fn startup_requeues_running_and_retry_waiting_tasks() {
  let env = TestEnvironment::start("task-recovery").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let running = seed_task_with_status(&state, "query.answer", "running").await;
  let waiting = seed_retry_waiting_task(&state, "query.answer").await;

  tasks::recovery::recover_tasks(&state).await.unwrap();

  let running_after = tasks::store::get_task_by_id(&state, &running.id).await.unwrap();
  let waiting_after = tasks::store::get_task_by_id(&state, &waiting.id).await.unwrap();
  assert_eq!(running_after.status, "queued");
  assert_eq!(waiting_after.status, "queued");
}
```

- [ ] **Step 2: Run the recovery test**

Run: `cargo test -p rust-integration startup_requeues_running_and_retry_waiting_tasks -- --exact`
Expected: FAIL because recovery behavior does not exist.

- [ ] **Step 3: Implement recovery and scheduler startup**

```rust
pub async fn recover_tasks(state: &AppState) -> Result<(), ApiError> {
  // reset queued/running/retry_waiting into executable queue state
}

pub fn spawn_scheduler(state: AppState) {
  tokio::spawn(async move {
    loop {
      run_scheduler_tick(&state).await;
      wait_for_wakeup_or_timeout(&state).await;
    }
  });
}
```

- [ ] **Step 4: Add Redis wake-up support in the cache module**

Requirements:
- publish a wake-up signal on task creation
- scheduler waits on pub/sub or falls back to periodic polling

- [ ] **Step 5: Re-run the recovery test and the full task runtime test file**

Run: `cargo test -p rust-integration --test task_runtime_api`
Expected: PASS for acquisition, completion, and startup recovery.

- [ ] **Step 6: Verify chunk 1**

Run:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all Rust verification passes.

- [ ] **Step 7: Commit chunk 1**

```bash
git add crates/knowledge-server tests/rust-integration/tests/task_runtime_api.rs
git commit -m "feat: add embedded scheduler and recovery"
```

## Chunk 2: Query Task API And Provider Abstraction

**Chunk goal:** Add async query-task endpoints, provider abstraction, one OpenAI-compatible adapter, and executor wiring for query tasks.

**Planned file structure:**

- Create: `crates/knowledge-server/src/providers/mod.rs`
- Create: `crates/knowledge-server/src/providers/types.rs`
- Create: `crates/knowledge-server/src/providers/openai_compatible.rs`
- Create: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `crates/knowledge-server/src/projects/tasks.rs`
- Modify: `crates/knowledge-server/src/settings/routes.rs`
- Modify: `crates/knowledge-server/src/settings/mod.rs`
- Modify: `crates/knowledge-server/src/config.rs`
- Test: `tests/rust-integration/tests/query_task_api.rs`

### Task 4: Add Async Query Task Creation And Polling APIs

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `crates/knowledge-server/src/projects/tasks.rs`
- Test: `tests/rust-integration/tests/query_task_api.rs`

- [ ] **Step 1: Write the failing query-task API test**

```rust
#[tokio::test]
async fn query_task_creation_returns_task_id_and_terminal_result() {
  let env = TestEnvironment::start("query-task-api").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("query-task-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
  seed_search_page(&project_root, "attention").await;

  let create = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query-tasks"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "query": "What is attention?", "topK": 3 }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(create.status(), StatusCode::ACCEPTED);
  let payload = read_json(create.into_body()).await;
  assert_eq!(payload["status"], "queued");
  assert!(payload["taskId"].as_str().is_some());
}
```

- [ ] **Step 2: Run the query-task API test**

Run: `cargo test -p rust-integration query_task_creation_returns_task_id_and_terminal_result -- --exact`
Expected: FAIL because the route does not exist.

- [ ] **Step 3: Add query-task creation and detail routes**

Routes to add:
- `POST /api/projects/{project_id}/query-tasks`
- `GET /api/projects/{project_id}/query-tasks/{task_id}`

- [ ] **Step 4: Persist `query.answer` payloads through the task store**

```rust
CreateTaskInput {
  task_type: "query.answer".to_string(),
  payload: json!({
    "query": payload.query,
    "topK": payload.top_k,
    "providerMode": settings.provider_mode,
    "language": settings.language,
    "requestedBy": session.user_id
  }),
  max_attempts: 3,
}
```

- [ ] **Step 5: Re-run the query-task API test**

Run: `cargo test -p rust-integration query_task_creation_returns_task_id_and_terminal_result -- --exact`
Expected: PASS for queued task creation and detail lookup.

- [ ] **Step 6: Commit the async query-task API**

```bash
git add crates/knowledge-server/src/projects crates/knowledge-server/src/tasks tests/rust-integration/tests/query_task_api.rs
git commit -m "feat: add async query task api"
```

### Task 5: Introduce Provider Abstraction And OpenAI-Compatible Adapter

**Files:**
- Create: `crates/knowledge-server/src/providers/mod.rs`
- Create: `crates/knowledge-server/src/providers/types.rs`
- Create: `crates/knowledge-server/src/providers/openai_compatible.rs`
- Modify: `crates/knowledge-server/src/settings/routes.rs`
- Modify: `crates/knowledge-server/src/settings/mod.rs`
- Test: `tests/rust-integration/tests/query_task_api.rs`

- [ ] **Step 1: Write the failing provider configuration test**

```rust
#[tokio::test]
async fn query_task_fails_when_provider_configuration_is_missing() {
  let env = TestEnvironment::start("provider-missing").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let task = seed_query_task(&state, json!({ "query": "attention", "topK": 3 })).await;

  let result = tasks::executors::run_query_executor(&state, &task).await;
  assert!(result.is_err());

  let detail = tasks::store::get_task_by_id(&state, &task.id).await.unwrap();
  assert_eq!(detail.status, "failed");
  assert_eq!(detail.error.as_ref().unwrap()["retryable"], false);
}
```

- [ ] **Step 2: Run the provider configuration test**

Run: `cargo test -p rust-integration query_task_fails_when_provider_configuration_is_missing -- --exact`
Expected: FAIL because there is no provider abstraction or query executor.

- [ ] **Step 3: Add provider settings fields and non-plaintext API key semantics**

Requirements:
- persist base URL, model, timeout, optional embedding model
- write-only API key update semantics
- settings read API returns configured/not configured indicator instead of raw key

- [ ] **Step 4: Implement provider interfaces and one OpenAI-compatible adapter**

```rust
pub trait ProviderClient {
  async fn answer_query(&self, request: ProviderQueryRequest) -> Result<ProviderAnswer, ProviderError>;
}
```

- [ ] **Step 5: Re-run the provider configuration test**

Run: `cargo test -p rust-integration query_task_fails_when_provider_configuration_is_missing -- --exact`
Expected: PASS for non-retryable configuration errors and adapter validation.

- [ ] **Step 6: Commit the provider abstraction**

```bash
git add crates/knowledge-server/src/providers crates/knowledge-server/src/settings tests/rust-integration/tests/query_task_api.rs
git commit -m "feat: add query provider abstraction"
```

### Task 6: Execute Query Tasks Through The Embedded Worker

**Files:**
- Create: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `crates/knowledge-server/src/tasks/scheduler.rs`
- Modify: `crates/knowledge-server/src/cache/mod.rs`
- Test: `tests/rust-integration/tests/query_task_api.rs`

- [ ] **Step 1: Write the failing async execution test**

```rust
#[tokio::test]
async fn query_task_runs_to_completion_and_persists_answer_and_citations() {
  let env = TestEnvironment::start("query-exec").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  configure_openai_compatible_provider(&state).await;
  let project_root = tempdir().unwrap().path().join("query-exec-project");
  let project_id = seed_project_with_attention_page(&state, &project_root).await;
  let task = create_query_task_via_api(&state, &project_id, "What is attention?").await;

  wait_for_task_terminal(&state, &task.id).await;

  let detail = tasks::store::get_task_by_id(&state, &task.id).await.unwrap();
  assert_eq!(detail.status, "succeeded");
  assert!(detail.result.as_ref().unwrap()["answer"].as_str().is_some());
  assert_eq!(detail.result.as_ref().unwrap()["citations"].as_array().unwrap().len(), 1);
}
```

- [ ] **Step 2: Run the async execution test**

Run: `cargo test -p rust-integration query_task_runs_to_completion_and_persists_answer_and_citations -- --exact`
Expected: FAIL because the scheduler does not dispatch a query executor.

- [ ] **Step 3: Implement the executor registry and `query.answer` executor**

Execution path:
- retrieve search results with existing `knowledge_core::search::search_project`
- build provider request from retrieved context
- normalize provider result into persisted task result shape
- write audit log for success and failure

- [ ] **Step 4: Add retry classification for provider failures**

Rules:
- 429/5xx/network timeout -> `retry_waiting`
- config/payload issues -> `failed`

- [ ] **Step 5: Re-run the async execution test and the full query task test file**

Run: `cargo test -p rust-integration --test query_task_api`
Expected: PASS for query task creation, execution, retry classification, and result persistence.

- [ ] **Step 6: Verify chunk 2**

Run:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all Rust verification passes.

- [ ] **Step 7: Commit chunk 2**

```bash
git add crates/knowledge-server tests/rust-integration/tests/query_task_api.rs
git commit -m "feat: execute async query tasks with provider adapter"
```

## Chunk 3: Project Operation Executors And Admin Query Workbench

**Chunk goal:** Move project operations into the durable executor model and extend the admin UI with a query workbench, richer tasks page, and provider settings support.

**Planned file structure:**

- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `crates/knowledge-server/src/projects/tasks.rs`
- Modify: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `apps/admin/src/app/router.tsx`
- Modify: `apps/admin/src/features/shared/api.ts`
- Modify: `apps/admin/src/features/projects/project-nav.tsx`
- Create: `apps/admin/src/features/query/page.tsx`
- Create: `apps/admin/src/features/query/queries.ts`
- Modify: `apps/admin/src/features/tasks/page.tsx`
- Modify: `apps/admin/src/features/tasks/queries.ts`
- Modify: `apps/admin/src/features/settings/page.tsx`
- Modify: `apps/admin/src/features/settings/queries.ts`
- Modify: `apps/admin/src/features/audit/page.tsx`
- Modify: `apps/admin/src/styles.css`
- Test: `apps/admin/src/features/query/page.test.tsx`
- Test: `tests/web/tests/query-workbench.spec.ts`

### Task 7: Move Project Operations Into Executor-Backed Durable Tasks

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `crates/knowledge-server/src/projects/tasks.rs`
- Test: `tests/rust-integration/tests/project_operation_runtime_api.rs`

- [ ] **Step 1: Write the failing project-operation runtime test**

```rust
#[tokio::test]
async fn ingest_is_enqueued_and_completed_by_worker() {
  let env = TestEnvironment::start("project-op-exec").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("project-op-exec");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
  import_source_via_api(state.clone(), &cookie, &csrf, &project_id, "attention.md", "IyBBdHRlbnRpb24K").await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/ingest"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "relativePath": "raw/sources/attention.md" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::ACCEPTED);
}
```

- [ ] **Step 2: Run the project-operation runtime test**

Run: `cargo test -p rust-integration ingest_is_enqueued_and_completed_by_worker -- --exact`
Expected: FAIL because the project operation endpoints still execute inline.

- [ ] **Step 3: Convert project operation endpoints to enqueue durable tasks**

Convert:
- import source
- rescan sources
- delete source
- ingest source
- review update

Rules:
- endpoints return `202 Accepted`
- actual work runs through task executors
- task result becomes the canonical operational response body for polling

- [ ] **Step 4: Re-run the project-operation runtime test file**

Run: `cargo test -p rust-integration --test project_operation_runtime_api`
Expected: PASS for async project operation execution.

- [ ] **Step 5: Commit executor-backed project operations**

```bash
git add crates/knowledge-server tests/rust-integration/tests/project_operation_runtime_api.rs
git commit -m "feat: execute project operations through durable tasks"
```

### Task 8: Add The Admin Query Workbench And Rich Task Views

**Files:**
- Create: `apps/admin/src/features/query/page.tsx`
- Create: `apps/admin/src/features/query/queries.ts`
- Modify: `apps/admin/src/app/router.tsx`
- Modify: `apps/admin/src/features/shared/api.ts`
- Modify: `apps/admin/src/features/projects/project-nav.tsx`
- Modify: `apps/admin/src/features/tasks/page.tsx`
- Modify: `apps/admin/src/features/tasks/queries.ts`
- Test: `apps/admin/src/features/query/page.test.tsx`
- Test: `apps/admin/src/features/projects/operations-actions.test.tsx`

- [ ] **Step 1: Write the failing query workbench unit test**

```tsx
it("submits a query task and renders the completed answer", async () => {
  const user = userEvent.setup();
  mockCreateQueryTask.mockResolvedValue({ taskId: "task-1", status: "queued" });
  mockTaskDetail
    .mockResolvedValueOnce({ id: "task-1", status: "queued" })
    .mockResolvedValueOnce({
      id: "task-1",
      status: "succeeded",
      result: {
        answer: "Attention focuses on relevant tokens.",
        citations: [{ path: "wiki/concepts/attention.md", title: "Attention", snippet: "relevant tokens", score: 1 }],
      },
    });

  render(<QueryPage />);
  await user.type(screen.getByLabelText("Query"), "What is attention?");
  await user.click(screen.getByRole("button", { name: "Run Query" }));

  expect(await screen.findByText("Attention focuses on relevant tokens.")).toBeInTheDocument();
  expect(screen.getByText("wiki/concepts/attention.md")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the query workbench test**

Run: `npm run test --workspace @knowledge/admin -- query/page.test.tsx`
Expected: FAIL because the page and query hooks do not exist.

- [ ] **Step 3: Add query-task client methods and hooks**

Add to shared API:
- `createQueryTask`
- `getQueryTaskDetail`
- `retryTask`
- `cancelTask`

- [ ] **Step 4: Build the query workbench and enhance the tasks page**

Requirements:
- route `/projects/:projectId/query`
- query submission form
- polling for active task
- answer and citations rendering
- task type/status filters
- attempt count and last error display

- [ ] **Step 5: Re-run the admin query tests**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS with query workbench and task enhancements.

- [ ] **Step 6: Commit the query workbench UI**

```bash
git add apps/admin
git commit -m "feat: add async query workbench"
```

### Task 9: Extend Settings, Audit, And Browser Coverage For Phase Two

**Files:**
- Modify: `apps/admin/src/features/settings/page.tsx`
- Modify: `apps/admin/src/features/settings/queries.ts`
- Modify: `apps/admin/src/features/audit/page.tsx`
- Modify: `tests/web/tests/project-operations.spec.ts`
- Create: `tests/web/tests/query-workbench.spec.ts`

- [ ] **Step 1: Write the failing provider settings and query Playwright test**

```ts
test("admin can configure provider settings and run an async query", async ({ page }) => {
  await page.goto("/login");
  await signInAsAdmin(page);
  await page.getByRole("link", { name: "Settings" }).click();
  await page.getByLabel("Provider Base URL").fill("http://127.0.0.1:18080/v1");
  await page.getByLabel("Provider API Key").fill("test-key");
  await page.getByLabel("Provider Model").fill("mock-model");
  await page.getByRole("button", { name: "Save Settings" }).click();

  await page.getByRole("link", { name: "Query" }).click();
  await page.getByLabel("Query").fill("What is attention?");
  await page.getByRole("button", { name: "Run Query" }).click();
  await expect(page.getByText("wiki/concepts/attention.md")).toBeVisible();
});
```

- [ ] **Step 2: Run the browser suite to confirm failure**

Run: `npm run test --workspace @knowledge/web`
Expected: FAIL because the query workbench and provider settings UI are incomplete.

- [ ] **Step 3: Add provider settings fields and audit visibility**

Requirements:
- provider base URL
- API key write-only field
- model
- timeout
- query task audit entries rendered in the audit page

- [ ] **Step 4: Add a deterministic OpenAI-compatible mock server to the Playwright environment if needed**

Approach:
- prefer a local test HTTP server started by Playwright config or test setup
- avoid test-only behavior in production routes

- [ ] **Step 5: Re-run the browser suite**

Run: `npm run test --workspace @knowledge/web`
Expected: PASS for project operations and query workbench flows.

- [ ] **Step 6: Verify chunk 3**

Run:

```powershell
npm test
npm run lint
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: full JS and Rust verification passes.

- [ ] **Step 7: Commit chunk 3**

```bash
git add apps/admin tests/web package.json
git commit -m "feat: add phase two query operations ui"
```

## Chunk 4: Recovery Hardening And Delivery Cleanup

**Chunk goal:** Harden retry and recovery behavior, cover critical restart semantics, and finalize delivery boundaries for the subproject.

**Planned file structure:**

- Modify: `crates/knowledge-server/src/tasks/scheduler.rs`
- Modify: `crates/knowledge-server/src/tasks/store.rs`
- Modify: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `tests/rust-integration/tests/query_task_api.rs`
- Modify: `tests/rust-integration/tests/project_operation_runtime_api.rs`

### Task 10: Harden Retry Windows, Cancellation, And Restart Semantics

**Files:**
- Modify: `crates/knowledge-server/src/tasks/scheduler.rs`
- Modify: `crates/knowledge-server/src/tasks/store.rs`
- Modify: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `tests/rust-integration/tests/query_task_api.rs`
- Modify: `tests/rust-integration/tests/project_operation_runtime_api.rs`

- [ ] **Step 1: Write the failing restart and retry regression test**

```rust
#[tokio::test]
async fn retryable_query_failures_reenter_queue_and_restart_recovery_preserves_execution() {
  let env = TestEnvironment::start("retry-recovery").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  configure_retryable_mock_provider(&state).await;
  let task = seed_query_task(&state, json!({ "query": "attention", "topK": 3 })).await;

  run_scheduler_tick(&state).await.unwrap();
  let waiting = tasks::store::get_task_by_id(&state, &task.id).await.unwrap();
  assert_eq!(waiting.status, "retry_waiting");

  tasks::recovery::recover_tasks(&state).await.unwrap();
  let recovered = tasks::store::get_task_by_id(&state, &task.id).await.unwrap();
  assert_eq!(recovered.status, "queued");
}
```

- [ ] **Step 2: Run the regression test**

Run: `cargo test -p rust-integration retryable_query_failures_reenter_queue_and_restart_recovery_preserves_execution -- --exact`
Expected: FAIL until retry windows and recovery edge cases are fully handled.

- [ ] **Step 3: Finalize retry, cancellation, and lease-expiry behavior**

Requirements:
- `retry_waiting` respects `next_retry_at`
- `cancelled` tasks are never reacquired
- expired `running` leases are re-queued exactly once per recovery pass

- [ ] **Step 4: Re-run the regression test and the focused runtime suites**

Run:

```powershell
cargo test -p rust-integration --test query_task_api
cargo test -p rust-integration --test project_operation_runtime_api
```

Expected: PASS for restart, retry, and cancellation semantics.

- [ ] **Step 5: Verify full subproject delivery**

Run:

```powershell
npm test
npm run lint
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all verification passes with no warnings.

- [ ] **Step 6: Commit the recovery hardening**

```bash
git add crates/knowledge-server tests/rust-integration
git commit -m "feat: harden task recovery and retry semantics"
```
