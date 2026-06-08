# Provider-Backed Query And Wiki Filing Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the placeholder query executor with a real OpenAI-compatible provider path, harden provider-driven retry behavior, and let successful query answers be filed back into the wiki as durable tasks.

**Architecture:** Keep the current embedded scheduler and durable task model. Add a narrow provider layer inside `knowledge-server`, keep retrieval grounded in existing project-local wiki search, and introduce a second durable query operation that writes successful answers into `wiki/queries/` with index and log updates so knowledge compounds instead of staying transient.

**Tech Stack:** Rust 2024, axum, tokio, serde, sqlx/postgres, redis, reqwest, React, TypeScript, TanStack Query, React Router, Vitest, Playwright

---

Use `@superpowers:test-driven-development` for behavior-bearing tasks and `@superpowers:verification-before-completion` before each commit or chunk handoff.

## Chunk 1: Real Provider Execution

**Chunk goal:** Introduce a real OpenAI-compatible provider adapter and replace the current placeholder query execution path with provider-backed responses.

**Planned file structure:**

- Modify: `Cargo.toml`
- Modify: `crates/knowledge-server/Cargo.toml`
- Create: `crates/knowledge-server/src/providers/mod.rs`
- Create: `crates/knowledge-server/src/providers/types.rs`
- Create: `crates/knowledge-server/src/providers/openai_compatible.rs`
- Modify: `crates/knowledge-server/src/lib.rs`
- Modify: `crates/knowledge-server/src/tasks/executors.rs`
- Test: `tests/rust-integration/tests/query_task_api.rs`
- Create: `tests/rust-integration/tests/support/mock_openai.rs`

### Task 1: Add A Deterministic OpenAI-Compatible Test Harness

**Files:**
- Create: `tests/rust-integration/tests/support/mock_openai.rs`
- Modify: `tests/rust-integration/tests/query_task_api.rs`

- [ ] **Step 1: Write the failing provider-backed query test**

```rust
#[tokio::test]
async fn query_task_uses_openai_compatible_provider_response() {
  let env = TestEnvironment::start("query-provider").await.unwrap();
  let mock = MockOpenAiServer::start(MockScenario::success()).await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  configure_provider(&state, &mock.base_url(), "mock-model").await;
  let project_root = tempdir().unwrap().path().join("query-provider-project");
  let project_id = seed_project_with_attention_page(&state, &project_root).await;
  let task = create_query_task_via_api(&state, &project_id, "What is attention?").await;

  wait_for_task_terminal(&state, &task.id).await;

  let detail = tasks::store::get_task_by_id(&state, &task.id).await.unwrap();
  assert_eq!(detail.status, "succeeded");
  assert_eq!(detail.result.as_ref().unwrap()["answer"], "Attention focuses computation on relevant tokens.");
  assert_eq!(detail.result.as_ref().unwrap()["usage"]["totalTokens"], 42);
}
```

- [ ] **Step 2: Run the test to confirm failure**

Run: `cargo test -p rust-integration query_task_uses_openai_compatible_provider_response -- --exact`
Expected: FAIL because there is no provider module or HTTP execution path.

- [ ] **Step 3: Add a tiny mock server utility**

Requirements:
- bind to a random local port
- expose `POST /v1/chat/completions`
- support at least `success`, `retryable_error`, and `invalid_request` scenarios
- return deterministic JSON so assertions stay stable

- [ ] **Step 4: Re-run the focused test**

Run: `cargo test -p rust-integration query_task_uses_openai_compatible_provider_response -- --exact`
Expected: still FAIL, now because production provider code is not wired yet.

- [ ] **Step 5: Commit the test harness**

```bash
git add tests/rust-integration/tests/query_task_api.rs tests/rust-integration/tests/support/mock_openai.rs
git commit -m "test: add mock openai provider harness"
```

### Task 2: Implement The OpenAI-Compatible Provider Adapter

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/knowledge-server/Cargo.toml`
- Create: `crates/knowledge-server/src/providers/mod.rs`
- Create: `crates/knowledge-server/src/providers/types.rs`
- Create: `crates/knowledge-server/src/providers/openai_compatible.rs`
- Modify: `crates/knowledge-server/src/tasks/executors.rs`

- [ ] **Step 1: Add the failing unit-level provider mapping test**

```rust
#[tokio::test]
async fn provider_maps_openai_compatible_response_into_internal_answer() {
  let server = MockOpenAiServer::start(MockScenario::success()).await.unwrap();
  let client = OpenAiCompatibleProvider::new(server.base_url(), "test-key", "mock-model", 30);

  let answer = client.answer_query(ProviderQueryRequest {
    query: "What is attention?".to_string(),
    context_blocks: vec!["[1] wiki/concepts/attention.md\nAttention lets models focus on relevant tokens.".to_string()],
    language: "en".to_string(),
  }).await.unwrap();

  assert_eq!(answer.answer, "Attention focuses computation on relevant tokens.");
  assert_eq!(answer.usage.total_tokens, 42);
}
```

- [ ] **Step 2: Run the focused provider test**

Run: `cargo test -p rust-integration provider_maps_openai_compatible_response_into_internal_answer -- --exact`
Expected: FAIL because the provider module does not exist.

- [ ] **Step 3: Add the provider types**

Required shapes:
- `ProviderQueryRequest`
- `ProviderAnswer`
- `ProviderCitation`
- `ProviderUsage`
- `ProviderError`

- [ ] **Step 4: Implement the adapter with request/response normalization**

Requirements:
- use `reqwest`
- send bearer auth when API key is configured
- honor `provider_timeout_seconds`
- call `/v1/chat/completions`
- parse answer text and usage fields
- normalize transport and provider errors into stable internal error codes

- [ ] **Step 5: Replace placeholder query completion logic**

In `crates/knowledge-server/src/tasks/executors.rs`:
- keep existing retrieval via `knowledge_core::search::search_project`
- build structured context blocks from search results
- call the provider instead of `answer_from_results`
- persist provider answer, citations, model, provider, and usage

- [ ] **Step 6: Re-run the query provider tests**

Run: `cargo test -p rust-integration --test query_task_api`
Expected: PASS for provider-backed success path and missing-config failure path.

- [ ] **Step 7: Commit the provider implementation**

```bash
git add Cargo.toml crates/knowledge-server
git commit -m "feat: add openai-compatible provider execution"
```

## Chunk 2: Retry And Failure Semantics

**Chunk goal:** Make provider-driven query execution robust by classifying retryable failures correctly and honoring retry windows in the embedded worker.

**Planned file structure:**

- Modify: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `crates/knowledge-server/src/tasks/store.rs`
- Modify: `crates/knowledge-server/src/tasks/recovery.rs`
- Modify: `crates/knowledge-server/src/tasks/scheduler.rs`
- Modify: `tests/rust-integration/tests/query_task_api.rs`
- Modify: `tests/rust-integration/tests/task_runtime_api.rs`

### Task 3: Retry Retryable Provider Failures Instead Of Failing Immediately

**Files:**
- Modify: `tests/rust-integration/tests/query_task_api.rs`
- Modify: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `crates/knowledge-server/src/tasks/store.rs`

- [ ] **Step 1: Write the failing retry-classification test**

```rust
#[tokio::test]
async fn retryable_provider_failure_enters_retry_waiting_with_next_retry_at() {
  let env = TestEnvironment::start("query-retry").await.unwrap();
  let mock = MockOpenAiServer::start(MockScenario::http_429()).await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  configure_provider(&state, &mock.base_url(), "mock-model").await;
  let task = seed_query_task(&state, json!({ "query": "attention", "topK": 3 })).await;

  tasks::scheduler::run_scheduler_tick(&state).await.unwrap_err();

  let detail = tasks::store::get_task_by_id(&state, &task.id).await.unwrap();
  assert_eq!(detail.status, "retry_waiting");
  assert!(detail.next_retry_at.is_some());
  assert_eq!(detail.error.as_ref().unwrap()["retryable"], true);
}
```

- [ ] **Step 2: Run the retry test**

Run: `cargo test -p rust-integration retryable_provider_failure_enters_retry_waiting_with_next_retry_at -- --exact`
Expected: FAIL because query failures are not yet classified by provider status.

- [ ] **Step 3: Implement explicit retry classification**

Rules:
- `429`, `500..=599`, timeout, and connection failure -> retryable
- malformed payload, bad configuration, and unsupported response shape -> non-retryable
- write `next_retry_at`
- keep `attempt_count` monotonic

- [ ] **Step 4: Ensure due-time gating is respected**

In task acquisition:
- `retry_waiting` tasks must not be reacquired before `next_retry_at`
- once due, they must become eligible again

- [ ] **Step 5: Re-run the focused runtime suites**

Run:

```powershell
cargo test -p rust-integration --test query_task_api
cargo test -p rust-integration --test task_runtime_api
```

Expected: PASS for retry classification and recovery timing.

- [ ] **Step 6: Commit the retry semantics**

```bash
git add crates/knowledge-server tests/rust-integration/tests/query_task_api.rs tests/rust-integration/tests/task_runtime_api.rs
git commit -m "feat: harden provider retry semantics"
```

## Chunk 3: Save Successful Query Answers Back Into The Wiki

**Chunk goal:** Add the `llm_wiki` compounding-knowledge behavior by letting successful query answers become durable wiki pages under `wiki/queries/`.

**Planned file structure:**

- Create: `crates/knowledge-core/src/project/queries.rs`
- Modify: `crates/knowledge-core/src/project/mod.rs`
- Modify: `crates/knowledge-core/src/lib.rs`
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `crates/knowledge-server/src/tasks/model.rs`
- Modify: `apps/admin/src/features/shared/api.ts`
- Modify: `apps/admin/src/features/query/page.tsx`
- Modify: `apps/admin/src/features/query/page.test.tsx`
- Create: `tests/rust-integration/tests/query_save_api.rs`
- Create: `tests/web/tests/query-workbench.spec.ts`
- Create: `tests/web/mock-openai.mjs`
- Modify: `tests/web/playwright.config.ts`

### Task 4: Add A Durable `query.save_answer` Task

**Files:**
- Create: `tests/rust-integration/tests/query_save_api.rs`
- Modify: `crates/knowledge-server/src/projects/routes.rs`
- Modify: `crates/knowledge-server/src/tasks/executors.rs`

- [ ] **Step 1: Write the failing save-answer API test**

```rust
#[tokio::test]
async fn save_query_answer_task_writes_query_page_and_updates_index_and_log() {
  let env = TestEnvironment::start("query-save").await.unwrap();
  let config = AppConfig::for_tests(env.database_url.clone(), env.redis_url.clone());
  let state = bootstrap_state(&config).await.unwrap();
  let (cookie, csrf) = login_and_csrf(state.clone()).await;
  let project_root = tempdir().unwrap().path().join("query-save-project");
  let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;
  let query_task_id = seed_successful_query_task(&state, &project_id).await;

  let response = build_app(state.clone())
    .oneshot(
      Request::builder()
        .method("POST")
        .uri(format!("/api/projects/{project_id}/query-tasks/{query_task_id}/save"))
        .header(header::COOKIE, &cookie)
        .header("x-csrf-token", &csrf)
        .body(Body::from(json!({ "title": "Attention Notes" }).to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  assert_eq!(response.status(), StatusCode::ACCEPTED);
}
```

- [ ] **Step 2: Run the save-answer test**

Run: `cargo test -p rust-integration save_query_answer_task_writes_query_page_and_updates_index_and_log -- --exact`
Expected: FAIL because the route and task type do not exist.

- [ ] **Step 3: Add the new durable task type**

Task type:
- `query.save_answer`

Payload should include:
- `sourceTaskId`
- `title`
- `slug`
- `answer`
- `citations`
- `contextSummary`

- [ ] **Step 4: Add the HTTP route**

Route to add:
- `POST /api/projects/{project_id}/query-tasks/{task_id}/save`

Rules:
- only allow saving terminal successful `query.answer` tasks
- enqueue `query.save_answer`
- return `202 Accepted`

- [ ] **Step 5: Re-run the save-answer API test**

Run: `cargo test -p rust-integration save_query_answer_task_writes_query_page_and_updates_index_and_log -- --exact`
Expected: still FAIL until the wiki write path exists.

### Task 5: Write Query Pages Into `wiki/queries/`

**Files:**
- Create: `crates/knowledge-core/src/project/queries.rs`
- Modify: `crates/knowledge-core/src/project/mod.rs`
- Modify: `crates/knowledge-core/src/lib.rs`
- Modify: `crates/knowledge-server/src/tasks/executors.rs`

- [ ] **Step 1: Add the failing knowledge-core writer test**

```rust
#[test]
fn save_query_page_writes_frontmatter_body_index_and_log() {
  let root = initialize_project(tempdir().unwrap().path()).unwrap();
  let result = save_query_page(
    &root,
    SaveQueryPageInput {
      title: "Attention Notes".to_string(),
      slug: "attention-notes".to_string(),
      answer: "Attention focuses computation on relevant tokens.".to_string(),
      citations: vec![SavedQueryCitation {
        path: "wiki/concepts/attention.md".to_string(),
        title: "Attention".to_string(),
      }],
      context_summary: "wiki/concepts/attention.md (Attention)".to_string(),
    },
  ).unwrap();

  assert_eq!(result.relative_path, "wiki/queries/attention-notes.md");
}
```

- [ ] **Step 2: Run the focused knowledge-core test**

Run: `cargo test -p knowledge-core save_query_page_writes_frontmatter_body_index_and_log -- --exact`
Expected: FAIL because the query page writer does not exist.

- [ ] **Step 3: Implement `save_query_page`**

Requirements:
- write markdown under `wiki/queries/<slug>.md`
- add YAML frontmatter with `type: query`, `title`, and `sources`
- render citations as wiki-relative references
- update `wiki/index.md`
- append `wiki/log.md`
- prefer deterministic overwrite when slug already exists

- [ ] **Step 4: Wire `query.save_answer` executor**

Execution path:
- read task payload
- call `save_query_page`
- persist `relativePath` and save metadata into task result
- append audit event for saved query page

- [ ] **Step 5: Re-run the full save-answer suite**

Run:

```powershell
cargo test -p knowledge-core save_query_page_writes_frontmatter_body_index_and_log -- --exact
cargo test -p rust-integration --test query_save_api
```

Expected: PASS for wiki writes and task orchestration.

- [ ] **Step 6: Commit the wiki filing path**

```bash
git add crates/knowledge-core crates/knowledge-server tests/rust-integration/tests/query_save_api.rs
git commit -m "feat: save query answers back into wiki"
```

## Chunk 4: Admin Save Flow And Browser Coverage

**Chunk goal:** Expose the new provider-backed query behavior and wiki filing path in the admin UI with deterministic browser coverage.

**Planned file structure:**

- Modify: `apps/admin/src/features/shared/api.ts`
- Modify: `apps/admin/src/features/query/page.tsx`
- Modify: `apps/admin/src/features/query/page.test.tsx`
- Create: `tests/web/mock-openai.mjs`
- Modify: `tests/web/playwright.config.ts`
- Create: `tests/web/tests/query-workbench.spec.ts`

### Task 6: Add Save-To-Wiki Controls To The Query Workbench

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts`
- Modify: `apps/admin/src/features/query/page.tsx`
- Modify: `apps/admin/src/features/query/page.test.tsx`

- [ ] **Step 1: Write the failing query page UI test**

```tsx
it("saves a successful query answer back into the wiki", async () => {
  const user = userEvent.setup();
  mockCreateQueryTask.mockResolvedValue({ taskId: "task-1", status: "queued" });
  mockTaskDetail.mockReturnValue({
    id: "task-1",
    status: "succeeded",
    result: {
      answer: "Attention focuses computation on relevant tokens.",
      citations: [{ path: "wiki/concepts/attention.md", title: "Attention", snippet: "relevant tokens", score: 1 }],
    },
  });
  mockSaveQueryTask.mockResolvedValue({ taskId: "task-2", status: "queued" });

  render(<QueryPage />);
  await user.click(screen.getByRole("button", { name: "Save To Wiki" }));

  expect(mockSaveQueryTask).toHaveBeenCalledWith({
    projectId: "project-1",
    taskId: "task-1",
    title: "What is attention?",
  });
});
```

- [ ] **Step 2: Run the query UI tests**

Run: `npm run test --workspace @knowledge/admin -- query/page.test.tsx`
Expected: FAIL because the save action does not exist.

- [ ] **Step 3: Add the client method and UI action**

Add:
- `saveQueryTaskResult`
- success-state button `Save To Wiki`
- success-state save status message or follow-up task id display

- [ ] **Step 4: Re-run the admin query tests**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS for provider-backed query UI and save flow.

- [ ] **Step 5: Commit the query page save flow**

```bash
git add apps/admin
git commit -m "feat: add query save-to-wiki flow"
```

### Task 7: Add Deterministic Browser Coverage For Provider Query And Save Flow

**Files:**
- Create: `tests/web/mock-openai.mjs`
- Modify: `tests/web/playwright.config.ts`
- Create: `tests/web/tests/query-workbench.spec.ts`

- [ ] **Step 1: Write the failing Playwright test**

```ts
test("admin can run a provider-backed query and save it to the wiki", async ({ page }) => {
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
  await expect(page.getByText("Attention focuses computation on relevant tokens.")).toBeVisible();
  await page.getByRole("button", { name: "Save To Wiki" }).click();
  await page.getByRole("link", { name: "Search" }).click();
  await page.getByLabel("Search Query").fill("Attention focuses computation");
  await page.getByRole("button", { name: "Run Search" }).click();
  await expect(page.getByText("wiki/queries/what-is-attention.md")).toBeVisible();
});
```

- [ ] **Step 2: Run the browser suite**

Run: `npm run test --workspace @knowledge/web`
Expected: FAIL because the provider mock server and save flow are not wired.

- [ ] **Step 3: Add a deterministic mock provider web server**

Requirements:
- start from Playwright config
- listen on a fixed local port
- return stable response JSON for `/v1/chat/completions`
- avoid test-only branches inside production Rust routes

- [ ] **Step 4: Re-run the browser suite**

Run: `npm run test --workspace @knowledge/web`
Expected: PASS for login, project operations, provider-backed query, and save-to-wiki flow.

- [ ] **Step 5: Verify the full next-phase slice**

Run:

```powershell
npm test
npm run lint
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all JS and Rust verification passes.

- [ ] **Step 6: Commit the browser coverage**

```bash
git add tests/web package-lock.json
git commit -m "test: add provider-backed query browser coverage"
```
