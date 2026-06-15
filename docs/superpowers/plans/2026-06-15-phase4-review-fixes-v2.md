# Phase 4 Review Fixes v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the seven real defects surfaced by the second-pass review of `phase4-review-fixes`. One P0 regression (settings endpoint opened to any Bearer principal — introduced by the previous batch's settings refactor), three P1 follow-throughs the previous plan promised but didn't deliver, and three P2 polish items.

**Why this is a separate plan:** the v1 review-fixes batch introduced a real P0 regression (Task 9 refactored settings auth from session-only to "any principal" without realising the consequences) and cut three promised items. Treating those as a follow-up plan rather than amendments to the merged batch keeps the audit trail honest.

**Architecture:** All changes are small and localized.

1. **P0 regression revert.** `settings/routes.rs` reverts to session-only auth. The "use `resolve_principal` everywhere" intuition was wrong for system administration endpoints; `resolve_principal` is the right helper for *consumer-facing* routes where Bearer tokens are explicitly part of the auth model. `/api/system/settings` is admin-only and stays session-only.
2. **Missing integration coverage.** Add Tavily / SerpApi / Ollama route-level integration tests that prove the persisted base URLs are actually used by the request, plus a no-sources-found deep-research test that exercises the executor's failure path.
3. **UI fixes.** Clear stale Test Search results when the form goes dirty; display project name (not UUID) in the API Tokens table; turn the deep-research savedPath into a clickable Files-tab link.
4. **Symmetric regression test.** Lock `searchApiKeyConfigured` the same way the v1 batch locked `providerApiKeyConfigured`.

**False positive surfaced in the review (not addressed here — no action required):** the "mojibake / encoding pollution" finding is incorrect. I verified the bytes at every cited line — they are valid UTF-8 (`e2 80 94 —`, `e2 80 a6 …`, `c2 b7 ·`). The reviewer's terminal is rendering UTF-8 source files through a Latin-1 / cp1252 codepage, which makes em-dash bytes look like `â€"`. The files render correctly in browsers and modern editors. Same conclusion as the v1 review.

**Tech Stack:** Rust (axum 0.8, sqlx), React 19 + TanStack Query, vitest. No new dependencies.

**Indentation conventions:** `crates/**` source = 2-space EXCEPT `crates/knowledge-server/src/projects/routes.rs` and `crates/knowledge-server/src/auth/routes.rs` = mixed (follow the file). `tests/rust-integration/**` = 4-space. All TS/TSX = 2-space.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `crates/knowledge-server/src/settings/routes.rs` | Modify | Revert to session-only auth |
| `tests/rust-integration/tests/web_search_api.rs` | Modify | (a) Symmetric `searchApiKeyConfigured` regression test, (b) Bearer rejection on `/api/system/settings`, (c) Tavily/SerpApi/Ollama route tests |
| `tests/rust-integration/tests/deep_research_api.rs` | Modify | No-sources-found integration test |
| `apps/admin/src/features/settings/page.tsx` | Modify | Reset `runWebSearch` when form becomes dirty |
| `apps/admin/src/features/api-tokens/page.tsx` | Modify | Show project name in table |
| `apps/admin/src/features/api-tokens/page.test.tsx` | Modify | Assert project name visible |
| `apps/admin/src/features/deep-research/page.tsx` | Modify | savedPath becomes a `<Link>` to the Files tab |
| `apps/admin/src/features/deep-research/page.test.tsx` | Modify | Assert link href contains the savedPath |

---

### Task 1: Revert settings handlers to session-only auth (P0)

**Files:**
- Modify: `crates/knowledge-server/src/settings/routes.rs`
- Modify: `tests/rust-integration/tests/web_search_api.rs`

The v1 batch's Task 9 "use `resolve_principal` everywhere" was a misjudgment for `/api/system/settings`. The helper accepts any Bearer token, so a project-scoped Bearer can now read and write global provider/search configuration — completely outside its scope, completely outside the CSRF check (CSRF is skipped for Bearer). System settings is an admin-only endpoint; the original `require_session` was correct.

- [ ] **Step 1: Write the failing tests**

Append two cases to `tests/rust-integration/tests/web_search_api.rs`:

```rust
#[tokio::test]
async fn settings_get_rejects_bearer_token() {
    let _env = TestEnvironment::start("settings-bearer-get").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(json!({ "name": "settings-test" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let token = read_json(mint.into_body()).await["token"]
        .as_str()
        .unwrap()
        .to_string();

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .uri("/api/system/settings")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "system settings must reject Bearer authentication"
    );
}

#[tokio::test]
async fn settings_patch_rejects_bearer_token() {
    let _env = TestEnvironment::start("settings-bearer-patch").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(json!({ "name": "settings-test" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let token = read_json(mint.into_body()).await["token"]
        .as_str()
        .unwrap()
        .to_string();

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25,
                      "searchProvider": "tavily",
                      "searchApiKey": "stolen-key"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "system settings must reject Bearer authentication"
    );
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p rust-integration --test web_search_api settings_get_rejects_bearer_token settings_patch_rejects_bearer_token -- --test-threads=1`
Expected: 2 FAIL — current code accepts Bearer.

- [ ] **Step 3: Revert to session-only**

In `crates/knowledge-server/src/settings/routes.rs`, replace the resolver block at the top of both handlers and reintroduce the session-only helper.

Replace the imports:

```rust
use axum::extract::State;
use axum::http::{header, HeaderMap};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
use crate::auth::session::find_session;
use crate::http::error::ApiError;
```

Replace `get_settings`:

```rust
async fn get_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
  require_session(&state, &headers).await?;
  Ok(Json(build_settings_response(&state).await?))
}
```

Replace `update_settings` opening:

```rust
async fn update_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
  let session = require_session(&state, &headers).await?;
  let supplied = headers
    .get("x-csrf-token")
    .and_then(|value| value.to_str().ok())
    .unwrap_or_default();
  if supplied.is_empty() || supplied != session.csrf_token {
    return Err(ApiError::unauthorized("invalid csrf token"));
  }
  // ... rest unchanged ...
}
```

Replace the trailing helper (the file ends with an empty function body after the v1 deletion) with:

```rust
async fn require_session(
  state: &AppState,
  headers: &HeaderMap,
) -> Result<crate::auth::session::SessionRecord, ApiError> {
  let session_id = headers
    .get(header::COOKIE)
    .and_then(|value| value.to_str().ok())
    .and_then(|cookie| {
      cookie
        .split(';')
        .map(str::trim)
        .find(|item| item.starts_with("knowledge_session="))
        .map(|item| item.trim_start_matches("knowledge_session=").to_string())
    })
    .ok_or_else(|| ApiError::unauthorized("missing session"))?;

  find_session(state, &session_id)
    .await?
    .ok_or_else(|| ApiError::unauthorized("missing session"))
}
```

(Returning the `SessionRecord` instead of `()` lets `update_settings` read `session.csrf_token` directly without re-parsing the cookie.)

- [ ] **Step 4: Verify both new tests pass**

Run: `cargo test -p rust-integration --test web_search_api -- --test-threads=1`
Expected: all PASS (existing tests + 2 new Bearer-rejection tests).

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/settings/routes.rs tests/rust-integration/tests/web_search_api.rs
git commit -m "fix(settings): require session authentication, rejecting bearer tokens"
```

---

### Task 2: Tavily / SerpApi / Ollama route-level integration tests (P1)

**Files:**
- Modify: `tests/rust-integration/tests/web_search_api.rs`

The previous plan's Task 5 promised "Tavily/SerpApi/Ollama using the new persisted base URLs" but delivered only the SearXNG-with-Bearer case. Close the gap with three small additions that each: (a) stand up a one-shot localhost HTTP mock for that provider, (b) PATCH `/api/system/settings` with the provider's specific fields including its persisted base URL, (c) POST `/api/web-search` and assert the mock was called and a normalized result came back.

The same `spawn_mock_searxng` pattern works for all three — only the URL paths and response JSON differ. Factor out a small `spawn_localhost_json_responder(method, path, body) -> (handle, base)` helper to avoid copy/paste, or just inline three short spawn helpers.

- [ ] **Step 1: Add a generic localhost responder helper**

Above the existing `spawn_mock_searxng`, add:

```rust
async fn spawn_localhost_responder(response_body: String) -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let body = response_body.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let payload = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(payload.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (handle, format!("http://127.0.0.1:{port}"))
}
```

- [ ] **Step 2: Add the three tests**

Append to `tests/rust-integration/tests/web_search_api.rs`:

```rust
#[tokio::test]
async fn web_search_tavily_uses_persisted_base_url() {
    let _env = TestEnvironment::start("web-search-tavily").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let (handle, base) = spawn_localhost_responder(
        json!({
          "results": [
            { "title": "Tav", "url": "https://example.com/tav", "content": "from tavily" }
          ]
        })
        .to_string(),
    )
    .await;

    let patch = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25,
                      "searchProvider": "tavily",
                      "searchApiKey": "test-key",
                      "tavilyBaseUrl": base
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web-search")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "query": "tavily query", "maxResults": 3 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response.into_body()).await;
    let results = payload.get("results").and_then(Value::as_array).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["title"].as_str(), Some("Tav"));
    assert_eq!(results[0]["url"].as_str(), Some("https://example.com/tav"));

    handle.abort();
}

#[tokio::test]
async fn web_search_serpapi_uses_persisted_base_url() {
    let _env = TestEnvironment::start("web-search-serpapi").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let (handle, base) = spawn_localhost_responder(
        json!({
          "organic_results": [
            { "title": "Serp", "link": "https://example.com/serp", "snippet": "from serpapi" }
          ]
        })
        .to_string(),
    )
    .await;

    let patch = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25,
                      "searchProvider": "serpapi",
                      "searchApiKey": "test-key",
                      "serpapiEngine": "google",
                      "serpapiBaseUrl": base
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web-search")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "query": "serpapi query", "maxResults": 3 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response.into_body()).await;
    let results = payload.get("results").and_then(Value::as_array).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["title"].as_str(), Some("Serp"));
    assert_eq!(results[0]["url"].as_str(), Some("https://example.com/serp"));

    handle.abort();
}

#[tokio::test]
async fn web_search_ollama_uses_persisted_url() {
    let _env = TestEnvironment::start("web-search-ollama").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let (handle, base) = spawn_localhost_responder(
        json!({
          "results": [
            { "title": "Olla", "url": "https://example.com/olla", "content": "from ollama" }
          ]
        })
        .to_string(),
    )
    .await;

    let patch = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25,
                      "searchProvider": "ollama",
                      "searchApiKey": "test-key",
                      "ollamaSearchUrl": base
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web-search")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "query": "ollama query", "maxResults": 3 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response.into_body()).await;
    let results = payload.get("results").and_then(Value::as_array).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["title"].as_str(), Some("Olla"));
    assert_eq!(results[0]["url"].as_str(), Some("https://example.com/olla"));

    handle.abort();
}
```

- [ ] **Step 3: Run all web_search_api tests**

Run: `cargo test -p rust-integration --test web_search_api -- --test-threads=1`
Expected: PASS (existing tests + 3 new provider tests).

- [ ] **Step 4: Commit**

```bash
git add tests/rust-integration/tests/web_search_api.rs
git commit -m "test(web-search): cover tavily, serpapi, ollama with persisted base urls"
```

---

### Task 3: Symmetric `searchApiKeyConfigured` regression test (P2)

**Files:**
- Modify: `tests/rust-integration/tests/web_search_api.rs`

The v1 batch locked `providerApiKeyConfigured` against drift. The symmetric `searchApiKeyConfigured` branch has the same `COALESCE(NULLIF($x, ''), search_api_key)` SQL and the same response shape, but no test. Add the mirror case.

- [ ] **Step 1: Append the test**

```rust
#[tokio::test]
async fn patch_settings_search_api_key_configured_reflects_db_state() {
    let _env = TestEnvironment::start("settings-search-api-key-truth").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    // First PATCH: set a search API key.
    let initial = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25,
                      "searchProvider": "tavily",
                      "searchApiKey": "first-search-secret"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial.status(), StatusCode::OK);
    let body = read_json(initial.into_body()).await;
    assert_eq!(body["searchApiKeyConfigured"], json!(true));

    // Second PATCH: omit the key.
    let preserve = build_app(state)
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/system/settings")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({
                      "providerMode": "openai-compatible",
                      "language": "en",
                      "defaultQueryLimit": 25,
                      "searchProvider": "tavily"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preserve.status(), StatusCode::OK);
    let body = read_json(preserve.into_body()).await;
    assert_eq!(
        body["searchApiKeyConfigured"],
        json!(true),
        "searchApiKeyConfigured must reflect DB state, not request payload"
    );
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p rust-integration --test web_search_api patch_settings_search_api_key_configured_reflects_db_state -- --test-threads=1`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/rust-integration/tests/web_search_api.rs
git commit -m "test(settings): lock searchApiKeyConfigured against db-truth regression"
```

---

### Task 4: Deep research no-sources-found integration test (P1)

**Files:**
- Modify: `tests/rust-integration/tests/deep_research_api.rs`

Previous plan promised this; the v1 batch shipped only the bearer happy-path. The executor's failure path (`ApiError::bad_request("no research sources found")`) needs an end-to-end test that proves the task lands in `failed` status with an error message visible in the task record.

- [ ] **Step 1: Add the test**

Append to `tests/rust-integration/tests/deep_research_api.rs`. Reuse the existing `configure_settings`, `login_and_csrf`, `create_project`, `read_json`, and `wait_for_task_terminal` helpers.

```rust
#[tokio::test]
async fn deep_research_marks_task_failed_when_no_sources_found() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("deep-research-no-sources").await.unwrap();
    let mock_openai = MockOpenAiServer::start(MockScenario::deep_research_success())
        .await
        .unwrap();
    let (searxng_handle, searxng_base) = spawn_mock_searxng_with_empty_results().await;
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("dr-no-sources-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

    configure_settings(&state, &mock_openai, &searxng_base).await;

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/deep-research"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(json!({ "topic": "Knowledge Graphs" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let task_id = read_json(response.into_body()).await["taskId"]
        .as_str()
        .unwrap()
        .to_string();
    wait_for_task_terminal(&state, &task_id).await;

    let task = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(task.status, "failed");
    let error = task.error.expect("failed task should record an error");
    let serialized = serde_json::to_string(&error).unwrap();
    assert!(
        serialized.contains("no research sources found"),
        "expected failure reason in task.error, got: {serialized}"
    );

    searxng_handle.abort();
}

async fn spawn_mock_searxng_with_empty_results() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let body = json!({ "results": [] }).to_string();
                let payload = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(payload.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (handle, format!("http://127.0.0.1:{port}"))
}
```

Verified `TaskRecord.error` is `Option<serde_json::Value>` at `crates/knowledge-server/src/tasks/model.rs:16`, so the `serde_json::to_string(&error).unwrap()` round-trip above is correct.

- [ ] **Step 2: Run**

Run: `cargo test -p rust-integration --test deep_research_api deep_research_marks_task_failed_when_no_sources_found -- --test-threads=1`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/rust-integration/tests/deep_research_api.rs
git commit -m "test(deep-research): cover no-sources-found failure path"
```

---

### Task 5: Reset stale Test Search results on dirty (P1)

**Files:**
- Modify: `apps/admin/src/features/settings/page.tsx`

The v1 batch disabled the Test Search button while `isDirty` and added a "save settings before testing" hint, but the previous result list is still rendered from `runWebSearch.data`. Users see "save before testing" alongside the old stale results — visually contradictory.

Fix: when any of the search-related fields change (or any settings field, more conservatively), call `runWebSearch.reset()`. The simplest place is the field `onChange` handlers that already call `setIsDirty(true)`. But that's repetitive. Cleaner: a `useEffect` that resets `runWebSearch` whenever `isDirty` becomes true.

- [ ] **Step 1: Add the effect**

In `apps/admin/src/features/settings/page.tsx`, add right after the existing `runWebSearch` declaration:

```tsx
  useEffect(() => {
    if (isDirty) {
      runWebSearch.reset();
      setTestError("");
    }
  }, [isDirty, runWebSearch]);
```

(`useEffect` is already imported at the top of the file.)

- [ ] **Step 2: Run the admin test suite**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS — including the existing settings tests. (No new test added because the existing dirty-disable behaviour is already covered; the reset is a UI-only refinement that doesn't change any test surface.)

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/settings/page.tsx
git commit -m "fix(settings): clear stale test search results when form becomes dirty"
```

---

### Task 6: API Tokens table shows project name (P2)

**Files:**
- Modify: `apps/admin/src/features/api-tokens/page.tsx`
- Modify: `apps/admin/src/features/api-tokens/page.test.tsx`

The picker uses project names; the table column still shows the raw UUID. Build a `Map<id, name>` from `projectsQuery.data` and render `projectsById.get(token.projectId)?.name ?? token.projectId ?? "—"`.

- [ ] **Step 1: Update the page**

In `apps/admin/src/features/api-tokens/page.tsx`, after the `projects` declaration, add:

```tsx
  const projectsById = new Map(projects.map((project) => [project.id, project.name] as const));
```

Replace the existing project cell:

```tsx
                    <TableCell>{token.projectId ?? "—"}</TableCell>
```

With:

```tsx
                    <TableCell>
                      {token.projectId
                        ? projectsById.get(token.projectId) ?? token.projectId
                        : "—"}
                    </TableCell>
```

- [ ] **Step 2: Update the project-column test**

In `apps/admin/src/features/api-tokens/page.test.tsx`, replace the existing project-column assertion:

```tsx
    expect(screen.getByText("project-1")).toBeInTheDocument();
```

With:

```tsx
    expect(screen.getByText("Demo Project")).toBeInTheDocument();
    expect(screen.queryByText("project-1")).not.toBeInTheDocument();
```

(The existing `useProjectsQuery` mock returns `{ id: "project-1", name: "Demo Project" }` — the page should display the name and not the UUID.)

- [ ] **Step 3: Run**

Run: `npm run test --workspace @knowledge/admin -- src/features/api-tokens/page.test.tsx`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/features/api-tokens/page.tsx apps/admin/src/features/api-tokens/page.test.tsx
git commit -m "feat(admin): show project name in api tokens table"
```

---

### Task 7: Deep research savedPath as a Files-tab link (P2)

**Files:**
- Modify: `apps/admin/src/features/deep-research/page.tsx`
- Modify: `apps/admin/src/features/deep-research/page.test.tsx`

`savedPath` currently renders as `<code>` — plain text. Wrap it in a `<Link>` to `/projects/:projectId/files?path=<savedPath>`. The Files tab reads this query parameter at `apps/admin/src/features/files/page.tsx:52` (`searchParams.get("path") ?? ""`) and uses it to seed `selectedPath`, so the deep link will open the file directly.

- [ ] **Step 1: Update the renderer**

In `apps/admin/src/features/deep-research/page.tsx`, change `renderTaskResult` to take `projectId`:

```tsx
function renderTaskResult(task: { result?: unknown; error?: unknown }, projectId: string) {
  // ... existing error branch unchanged ...
  if (
    typeof task.result === "object" &&
    task.result &&
    "savedPath" in task.result
  ) {
    const result = task.result as { savedPath?: string; sourceCount?: number; errors?: string[] };
    return (
      <div className="grid gap-1">
        {result.savedPath ? (
          <p>
            Saved:{" "}
            <Link
              className="underline"
              to={`/projects/${projectId}/files?path=${encodeURIComponent(result.savedPath)}`}
            >
              <code>{result.savedPath}</code>
            </Link>
          </p>
        ) : null}
        {/* ... rest unchanged ... */}
      </div>
    );
  }
  return null;
}
```

(Add `import { Link } from "react-router-dom";` at the top of the file.)

Update the call site:

```tsx
              <CardContent className="grid gap-2 text-sm">
                {renderTaskResult(task, projectId)}
              </CardContent>
```

- [ ] **Step 2: Update the page test**

In `apps/admin/src/features/deep-research/page.test.tsx`, the existing "renders the saved path and source count" test should assert the link:

```tsx
    expect(screen.getByText("wiki/queries/research-kg.md")).toBeInTheDocument();
    expect(screen.getByText("Sources used: 3")).toBeInTheDocument();
    const link = screen.getByRole("link", { name: /wiki\/queries\/research-kg\.md/ });
    expect(link).toHaveAttribute(
      "href",
      "/projects/project-1/files?path=wiki%2Fqueries%2Fresearch-kg.md",
    );
```

- [ ] **Step 3: Run**

Run: `npm run test --workspace @knowledge/admin -- src/features/deep-research/page.test.tsx`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/features/deep-research/page.tsx apps/admin/src/features/deep-research/page.test.tsx
git commit -m "feat(deep-research): link saved path to the Files tab"
```

---

### Task 8: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Rust lint + tests**

Run: `cargo fmt --all -- --check`
Expected: clean.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo test --workspace -- --test-threads=1`
Expected: PASS — including:
- 2 new settings-Bearer-rejection tests (Task 1)
- 3 new provider route-level tests (Task 2)
- 1 new `searchApiKeyConfigured` regression test (Task 3)
- 1 new no-sources-found test (Task 4)
- All pre-existing tests still pass.

- [ ] **Step 2: Frontend tests + lint**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS — including the updated api-tokens project-name assertion and the updated deep-research link assertion.

Run: `npm run test --workspace @knowledge/api-client`
Expected: PASS (regression check).

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: PASS (regression check).

Run: `npm run lint`
Expected: tsc + clippy clean.

- [ ] **Step 3: Playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: PASS. (No e2e is added in this fix plan; all changes are at unit/integration scope.)

- [ ] **Step 4: Final commit (only if fixes were needed)**

```bash
git status
```

If steps 1–3 required fixes, commit them. Otherwise the work is complete.
