# Phase 4 Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the gaps surfaced by the Phase 4 code review. One P0 security defect (token-management scope escalation), six P1 correctness/coverage gaps, and two P2 polish items. Each fix is small and self-contained; the plan is a single batch because the items share test infrastructure and conventions, not because they're entangled.

**Architecture:** Mostly small, targeted patches. The largest change (Task 1) refactors token-management handlers to honour the caller's principal scope. Task 3 adds one migration (`0011_web_search_base_urls.sql`) and threads two new columns through `WebSearchConfig` + settings GET/PATCH + admin UI. Other tasks are localized to a single file or a handler/test pair.

**Tech Stack:** Rust (axum 0.8, sqlx, reqwest), React 19 + TanStack Query + Zod, vitest, Playwright. No new dependencies.

**Reference materials (not "upstream port" this time — these are post-merge corrections):**
- `docs/superpowers/plans/2026-06-13-phase4a-api-tokens.md` — original Phase 4a plan. Task 1 below tightens behaviour the plan never specified (scope containment for Bearer-managed tokens).
- `docs/superpowers/plans/2026-06-15-phase4c-web-search.md` — divergence #5 claimed Tavily/SerpApi base URLs were configurable; Task 3 makes that claim real.
- `docs/superpowers/plans/2026-06-15-phase4d-deep-research.md` — Task 7 below routes the executor through `save_wiki_page` and surfaces the task result in the UI, completing the "outcome visible" goal that was partially met.

**Documented behaviour changes:**
1. A project-scoped API token can no longer mint unscoped tokens, mint tokens for a different project, or list/revoke tokens outside its own scope. Session callers and unscoped Bearer callers are unaffected. (Task 1.)
2. `PATCH /api/system/settings` response shape changes: `searchApiKeyConfigured` and `providerApiKeyConfigured` are now computed from DB-truth after the UPDATE, not from the request payload. Callers that relied on optimistic reflection now see the persisted state. (Task 2.)
3. `system_settings` gains two columns `tavily_base_url` and `serpapi_base_url` (Task 3). The defaults are unchanged (`https://api.tavily.com`, `https://serpapi.com`), so existing deployments behave identically.
4. `POST /api/web-search` and `POST /api/projects/{id}/deep-research` are now exercised under Bearer authentication in integration tests (Tasks 5 and 7).
5. Deep research executor goes through `save_wiki_page` for path validation parity with other executors. (Task 7.)
6. Admin Settings page disables the **Test Search** button while the form is dirty, so users no longer test a stale config without knowing. (Task 9.)
7. The settings handlers stop hand-rolling session cookie parsing and call `resolve_principal` instead. (Task 9.)

**Indentation conventions:** `crates/**` source = 2-space EXCEPT `crates/knowledge-server/src/projects/routes.rs` and `crates/knowledge-server/src/auth/routes.rs` = mixed (follow the file). `tests/rust-integration/**` = 4-space. All TS/TSX = 2-space.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `crates/knowledge-server/src/auth/routes.rs` | Modify | Honour principal scope in create/list/revoke |
| `tests/rust-integration/tests/api_tokens_api.rs` | Modify | Add three scope-containment tests |
| `crates/knowledge-server/src/settings/routes.rs` | Modify | Settings PATCH re-reads DB after UPDATE; use `resolve_principal` |
| `crates/knowledge-server/migrations/0011_web_search_base_urls.sql` | Create | Add `tavily_base_url`, `serpapi_base_url` columns |
| `crates/knowledge-server/src/web_search/config.rs` | Modify | Load new columns; `parse_provider` returns `Result` for invalid values |
| `crates/knowledge-server/src/settings/routes.rs` | Modify | GET/PATCH new columns |
| `apps/admin/src/features/shared/api.ts` | Modify | Schema + payload for new fields |
| `apps/admin/src/features/settings/page.tsx` | Modify | Advanced base-URL fields; disable Test Search while dirty |
| `apps/admin/src/features/api-tokens/page.tsx` | Modify | Project scope picker + table column |
| `apps/admin/src/features/api-tokens/page.test.tsx` | Modify | Cover scoped mint and project column |
| `apps/admin/src/features/shared/api.ts` | Modify | List user-visible projects for the scope picker |
| `tests/rust-integration/tests/web_search_api.rs` | Modify | Add Bearer-auth case + Tavily/SerpApi/Ollama route checks |
| `tests/rust-integration/tests/deep_research_api.rs` | Modify | Add Bearer-auth case + no-sources-found case |
| `crates/knowledge-server/src/tasks/executors.rs` | Modify | Deep research executor uses `save_wiki_page` |
| `apps/admin/src/features/deep-research/page.tsx` | Modify | Render `task.result` (savedPath/sourceCount/errors) + `task.error` |
| `apps/admin/src/features/deep-research/page.test.tsx` | Modify | Assert savedPath + error are visible |
| `packages/mcp-server/test/server.test.ts` | Modify | Exercise the remaining six tools end-to-end |

---

### Task 1: Token scope containment (P0)

**Files:**
- Modify: `crates/knowledge-server/src/auth/routes.rs` (2-space indent)
- Modify: `tests/rust-integration/tests/api_tokens_api.rs` (4-space indent)

Three handlers (`create_api_token_handler`, `list_api_tokens_handler`, `revoke_api_token_handler`) currently filter only by `principal.user_id` and ignore `principal.project_id`. A project-scoped Bearer therefore escalates to unscoped or sideways. Fix: derive an effective scope from the principal and enforce it.

The rule is straightforward:
- Session principal: unrestricted (admin UI manages the full user-owned set).
- Unscoped Bearer (`principal.project_id == None`): unrestricted across the user's tokens — matches current behaviour.
- Scoped Bearer (`principal.project_id == Some(scope)`):
  - `create`: payload's `project_id` MUST equal `scope`. Reject `null` or any other value with 403.
  - `list`: return only tokens whose `project_id == scope`.
  - `revoke`: reject if the target token's `project_id != scope` with 404 (same 404 as "token not found" so we don't leak the target's existence).

- [ ] **Step 1: Write the failing integration tests**

Append to `tests/rust-integration/tests/api_tokens_api.rs`:

```rust
#[tokio::test]
async fn project_scoped_token_cannot_mint_unscoped() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("api-tokens-no-escalate-mint").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("scope-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

    let scoped_token = mint_scoped_token(state.clone(), &cookie, &csrf, &project_id).await;

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {scoped_token}"))
                .body(Body::from(
                    json!({ "name": "escalation", "projectId": null }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn project_scoped_token_cannot_mint_for_other_project() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("api-tokens-no-escalate-cross").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_a_root = temp.path().join("project-a");
    let project_a = create_project(state.clone(), &cookie, &csrf, project_a_root).await;
    let project_b_root = temp.path().join("project-b");
    let project_b = create_project(state.clone(), &cookie, &csrf, project_b_root).await;

    let scoped_a = mint_scoped_token(state.clone(), &cookie, &csrf, &project_a).await;

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {scoped_a}"))
                .body(Body::from(
                    json!({ "name": "cross", "projectId": project_b }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn project_scoped_token_list_and_revoke_only_see_same_scope() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("api-tokens-scope-isolation").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("scope-iso");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root).await;

    let unscoped_id = mint_and_id(state.clone(), &cookie, &csrf, None).await;
    let scoped_id = mint_and_id(state.clone(), &cookie, &csrf, Some(&project_id)).await;
    let scoped_token = scoped_id.1.clone();

    let listed = build_app(state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/users/me/api-tokens")
                .header("authorization", format!("Bearer {}", scoped_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let payload = read_json(listed.into_body()).await;
    let ids = payload["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|token| token["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(ids.contains(&scoped_id.0), "scoped token should see itself");
    assert!(!ids.contains(&unscoped_id.0), "scoped token must not see unscoped tokens");

    let denied = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/users/me/api-tokens/{}/revoke", unscoped_id.0))
                .header("authorization", format!("Bearer {}", scoped_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);

    let allowed = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/users/me/api-tokens/{}/revoke", scoped_id.0))
                .header("authorization", format!("Bearer {}", scoped_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
}

async fn mint_scoped_token(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_id: &str,
) -> String {
    mint_and_id(state, cookie, csrf, Some(project_id)).await.1
}

async fn mint_and_id(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_id: Option<&str>,
) -> (String, String) {
    let body = match project_id {
        Some(value) => json!({ "name": "test", "projectId": value }),
        None => json!({ "name": "test", "projectId": null }),
    };
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let payload = read_json(response.into_body()).await;
    (
        payload["id"].as_str().unwrap().to_string(),
        payload["token"].as_str().unwrap().to_string(),
    )
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rust-integration --test api_tokens_api project_scoped -- --test-threads=1`
Expected: 3 tests FAIL — current code allows the escalation.

- [ ] **Step 3: Add the scope-containment helpers**

In `crates/knowledge-server/src/auth/api_token.rs`, extend `find_by_token` consumers by adding two list/revoke variants that take the principal's scope. Append:

```rust
/// Return all tokens for the user OR only those scoped to the given project.
pub async fn list_tokens_for_user_scoped(
  state: &AppState,
  user_id: &str,
  scope: Option<&str>,
) -> Result<Vec<ApiTokenRecord>, ApiError> {
  let rows = match scope {
    None => {
      sqlx::query_as::<
        _,
        (String, String, Option<String>, String, String, Option<String>, Option<String>, String),
      >(
        "SELECT id, user_id, project_id, name, token_prefix, last_used_at, revoked_at, created_at
         FROM api_tokens
         WHERE user_id = $1
         ORDER BY created_at DESC",
      )
      .bind(user_id)
      .fetch_all(&state.pool)
      .await
    }
    Some(project_id) => {
      sqlx::query_as::<
        _,
        (String, String, Option<String>, String, String, Option<String>, Option<String>, String),
      >(
        "SELECT id, user_id, project_id, name, token_prefix, last_used_at, revoked_at, created_at
         FROM api_tokens
         WHERE user_id = $1 AND project_id = $2
         ORDER BY created_at DESC",
      )
      .bind(user_id)
      .bind(project_id)
      .fetch_all(&state.pool)
      .await
    }
  }
  .map_err(ApiError::from)?;
  Ok(
    rows
      .into_iter()
      .map(|(id, user_id, project_id, name, token_prefix, last_used_at, revoked_at, created_at)| {
        ApiTokenRecord {
          id,
          user_id,
          project_id,
          name,
          token_prefix,
          last_used_at,
          revoked_at,
          created_at,
        }
      })
      .collect(),
  )
}

pub async fn revoke_token_scoped(
  state: &AppState,
  user_id: &str,
  scope: Option<&str>,
  token_id: &str,
) -> Result<bool, ApiError> {
  let now = OffsetDateTime::now_utc()
    .format(&Rfc3339)
    .map_err(|_| ApiError::internal("failed to format revoked_at"))?;
  let result = match scope {
    None => {
      sqlx::query(
        "UPDATE api_tokens
         SET revoked_at = $1
         WHERE id = $2 AND user_id = $3 AND revoked_at IS NULL",
      )
      .bind(&now)
      .bind(token_id)
      .bind(user_id)
      .execute(&state.pool)
      .await
    }
    Some(project_id) => {
      sqlx::query(
        "UPDATE api_tokens
         SET revoked_at = $1
         WHERE id = $2 AND user_id = $3 AND project_id = $4 AND revoked_at IS NULL",
      )
      .bind(&now)
      .bind(token_id)
      .bind(user_id)
      .bind(project_id)
      .execute(&state.pool)
      .await
    }
  }
  .map_err(ApiError::from)?;
  Ok(result.rows_affected() > 0)
}
```

Leave the existing `list_tokens_for_user` and `revoke_token` in place — callers outside the auth route may still need them. They become thin wrappers:

```rust
// Existing wrappers — keep for any non-handler callers.
pub async fn list_tokens_for_user(
  state: &AppState,
  user_id: &str,
) -> Result<Vec<ApiTokenRecord>, ApiError> {
  list_tokens_for_user_scoped(state, user_id, None).await
}

pub async fn revoke_token(
  state: &AppState,
  user_id: &str,
  token_id: &str,
) -> Result<bool, ApiError> {
  revoke_token_scoped(state, user_id, None, token_id).await
}
```

- [ ] **Step 4: Apply scope check to the create handler**

In `crates/knowledge-server/src/auth/routes.rs`, replace the membership-check block inside `create_api_token_handler` (currently lines 213–225):

```rust
  // Scope containment: project-scoped Bearer principals can only mint
  // tokens for their own scope, never null and never another project.
  if principal.scope == crate::auth::principal::AuthScope::ApiToken {
    if let Some(scope) = principal.project_id.as_deref() {
      let requested = payload.project_id.as_deref();
      if requested != Some(scope) {
        return Err(ApiError::forbidden(
          "project-scoped api tokens can only mint tokens for the same project",
        ));
      }
    }
  }
  if let Some(project_id) = payload.project_id.as_deref() {
    let membership = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND user_id = $2",
    )
    .bind(project_id)
    .bind(&principal.user_id)
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;
    if membership == 0 {
      return Err(ApiError::forbidden("not a project member"));
    }
  }
```

You'll need to also import `AuthScope`. Add to existing imports:

```rust
use crate::auth::principal::AuthScope;
```

The plan keeps `AuthScope::Session` paths unrestricted because session callers manage the full user-owned set through the admin UI.

- [ ] **Step 5: Apply scope check to list/revoke**

In the same file, replace the `list_tokens_for_user(...)` call inside `list_api_tokens_handler`:

```rust
  let scope = if principal.scope == AuthScope::ApiToken {
    principal.project_id.as_deref()
  } else {
    None
  };
  let tokens = crate::auth::api_token::list_tokens_for_user_scoped(
    &state,
    &principal.user_id,
    scope,
  )
  .await?;
```

And replace `revoke_token(...)` inside `revoke_api_token_handler`:

```rust
  let scope = if principal.scope == AuthScope::ApiToken {
    principal.project_id.as_deref()
  } else {
    None
  };
  let revoked = crate::auth::api_token::revoke_token_scoped(
    &state,
    &principal.user_id,
    scope,
    &token_id,
  )
  .await?;
```

The existing 404 ("token not found") response now also covers "token exists but is outside your scope". That's deliberate — leaking existence to a scoped caller is itself a side-channel.

- [ ] **Step 6: Run the new tests to verify they pass**

Run: `cargo test -p rust-integration --test api_tokens_api -- --test-threads=1`
Expected: all 5 tests PASS (2 existing + 3 new).

- [ ] **Step 7: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/auth/routes.rs crates/knowledge-server/src/auth/api_token.rs tests/rust-integration/tests/api_tokens_api.rs
git commit -m "fix(auth): enforce scope containment on api-token management routes"
```

---

### Task 2: Settings PATCH returns DB-truth (P1)

**Files:**
- Modify: `crates/knowledge-server/src/settings/routes.rs`

The handler currently returns `searchApiKeyConfigured: payload.search_api_key.is_some_and(...)` and the equivalent for `providerApiKeyConfigured`. With the `COALESCE(NULLIF($x, ''), col)` UPDATE preserving the secret when the request omits the key, the response says "not configured" while the DB still has the key.

Fix: re-read the row after UPDATE and return the same JSON shape `get_settings` returns.

- [ ] **Step 1: Refactor the response generation**

In `crates/knowledge-server/src/settings/routes.rs`, extract the SELECT + JSON construction from `get_settings` into a private helper:

```rust
async fn build_settings_response(
  state: &AppState,
) -> Result<serde_json::Value, ApiError> {
  // Move the existing SELECT body here, returning the json!({...}) value.
}
```

Then make both `get_settings` and `update_settings` end with:

```rust
  Ok(Json(build_settings_response(&state).await?))
```

In `update_settings`, delete the entire "construct response from payload" block (currently the trailing `Ok(Json(json!({ ... })))`). The response now reflects DB state, including `searchApiKeyConfigured: true` when the COALESCE preserved an existing key.

- [ ] **Step 2: Add an integration test**

Append to `tests/rust-integration/tests/web_search_api.rs` (or a new `settings_api.rs` — pick whichever feels closer to existing patterns):

```rust
#[tokio::test]
async fn patch_settings_response_reflects_db_state() {
    let _env = TestEnvironment::start("settings-response-truth").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    // First PATCH: set an API key.
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
                      "providerApiKey": "first-secret"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial.status(), StatusCode::OK);
    let body = read_json(initial.into_body()).await;
    assert_eq!(body["providerApiKeyConfigured"], json!(true));

    // Second PATCH: omit the key. Old code returned false; fixed code returns true
    // because the DB still holds it via COALESCE.
    let preserve = build_app(state.clone())
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
                      "defaultQueryLimit": 25
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
        body["providerApiKeyConfigured"], json!(true),
        "providerApiKeyConfigured must reflect DB state, not request payload"
    );
}
```

- [ ] **Step 3: Verify**

Run: `cargo test -p rust-integration --test web_search_api patch_settings -- --test-threads=1`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/settings/routes.rs tests/rust-integration/tests/web_search_api.rs
git commit -m "fix(settings): return persisted state from PATCH /api/system/settings"
```

---

### Task 3: Web search base URLs persisted in `system_settings` (P1)

**Files:**
- Create: `crates/knowledge-server/migrations/0011_web_search_base_urls.sql`
- Modify: `crates/knowledge-server/src/web_search/config.rs`
- Modify: `crates/knowledge-server/src/settings/routes.rs`

The Phase 4c plan claimed Tavily/SerpApi base URLs were "overrideable via `config.rs`" and listed test injection as the reason. Production deployments behind enterprise proxies have no path. Persist them.

- [ ] **Step 1: Create the migration**

`crates/knowledge-server/migrations/0011_web_search_base_urls.sql`:

```sql
ALTER TABLE system_settings ADD COLUMN tavily_base_url TEXT NOT NULL DEFAULT 'https://api.tavily.com';
ALTER TABLE system_settings ADD COLUMN serpapi_base_url TEXT NOT NULL DEFAULT 'https://serpapi.com';
```

- [ ] **Step 2: Verify migration applies**

Run: `cargo test -p rust-integration --test auth_api login_caches_session_in_redis -- --test-threads=1`
Expected: PASS. The migration runs on `bootstrap_state` startup.

- [ ] **Step 3: Plumb new columns through `WebSearchConfig`**

In `crates/knowledge-server/src/web_search/config.rs`, change `load_web_search_config` to select the two new columns and use them instead of the hardcoded strings. Replace the SELECT + struct construction (currently lines 66–99):

```rust
  let (
    search_provider,
    search_api_key,
    serpapi_engine,
    searxng_url,
    searxng_categories,
    ollama_search_url,
    tavily_base_url,
    serpapi_base_url,
  ) = sqlx::query_as::<
    _,
    (
      String,
      Option<String>,
      Option<String>,
      Option<String>,
      Value,
      Option<String>,
      String,
      String,
    ),
  >(
    "SELECT
       search_provider,
       search_api_key,
       serpapi_engine,
       searxng_url,
       searxng_categories,
       ollama_search_url,
       tavily_base_url,
       serpapi_base_url
     FROM system_settings
     WHERE id = 1",
  )
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  let Some(provider) = parse_provider(&search_provider)? else {
    return Ok(None);
  };

  Ok(Some(WebSearchConfig {
    provider,
    api_key: search_api_key.filter(|value| !value.is_empty()),
    serpapi_engine: serpapi_engine.unwrap_or_else(|| "google".to_string()),
    searxng_url: searxng_url.filter(|value| !value.trim().is_empty()),
    searxng_categories: parse_categories(&searxng_categories),
    ollama_search_url: ollama_search_url
      .filter(|value| !value.trim().is_empty())
      .unwrap_or_else(|| "https://ollama.com".to_string()),
    tavily_base_url,
    serpapi_base_url,
  }))
```

- [ ] **Step 4: Change `parse_provider` to surface invalid values**

A corrupted `search_provider` column currently downgrades to "not configured" silently. Make invalid values an error so operators see misconfig as a 500 instead of confusing 400.

```rust
pub(crate) fn parse_provider(value: &str) -> Result<Option<WebSearchProvider>, ApiError> {
  match value {
    "none" => Ok(None),
    "tavily" => Ok(Some(WebSearchProvider::Tavily)),
    "serpapi" => Ok(Some(WebSearchProvider::SerpApi)),
    "searxng" => Ok(Some(WebSearchProvider::SearXng)),
    "ollama" => Ok(Some(WebSearchProvider::Ollama)),
    other => Err(ApiError::internal(format!(
      "invalid search_provider in system_settings: {other:?}"
    ))),
  }
}
```

Update the three unit tests in `config.rs` accordingly:

```rust
  #[test]
  fn parse_provider_maps_known_values() {
    assert_eq!(parse_provider("tavily").unwrap(), Some(WebSearchProvider::Tavily));
    assert_eq!(parse_provider("serpapi").unwrap(), Some(WebSearchProvider::SerpApi));
    assert_eq!(parse_provider("searxng").unwrap(), Some(WebSearchProvider::SearXng));
    assert_eq!(parse_provider("ollama").unwrap(), Some(WebSearchProvider::Ollama));
  }

  #[test]
  fn parse_provider_treats_none_sentinel_as_disabled() {
    assert!(parse_provider("none").unwrap().is_none());
  }

  #[test]
  fn parse_provider_errors_on_invalid_value() {
    assert!(parse_provider("google").is_err());
    assert!(parse_provider("").is_err());
  }
```

- [ ] **Step 5: Extend settings GET/PATCH to expose the new fields**

In `crates/knowledge-server/src/settings/routes.rs`, add to `UpdateSettingsRequest`:

```rust
  #[serde(default)]
  pub tavily_base_url: Option<String>,
  #[serde(default)]
  pub serpapi_base_url: Option<String>,
```

Extend the SELECT in `build_settings_response` (from Task 2) to include both columns, and add them to the JSON output:

```rust
    "tavilyBaseUrl": tavily_base_url,
    "serpapiBaseUrl": serpapi_base_url,
```

Extend the UPDATE in `update_settings`:

```sql
         tavily_base_url = COALESCE(NULLIF($N, ''), tavily_base_url),
         serpapi_base_url = COALESCE(NULLIF($M, ''), serpapi_base_url)
```

And bind:

```rust
  .bind(payload.tavily_base_url.as_deref())
  .bind(payload.serpapi_base_url.as_deref())
```

- [ ] **Step 6: Run unit + integration tests**

Run: `cargo test -p knowledge-server web_search::config`
Expected: 3 tests PASS.

Run: `cargo test -p rust-integration --test web_search_api -- --test-threads=1`
Expected: existing tests still PASS (defaults preserve current behaviour).

- [ ] **Step 7: Clippy + commit**

```bash
git add crates/knowledge-server/migrations/0011_web_search_base_urls.sql crates/knowledge-server/src/web_search/config.rs crates/knowledge-server/src/settings/routes.rs
git commit -m "feat(web-search): persist tavily and serpapi base urls in system_settings"
```

---

### Task 4: Admin advanced web-search fields (P1)

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts`
- Modify: `apps/admin/src/features/settings/page.tsx`

- [ ] **Step 1: Extend the settings schema**

In `apps/admin/src/features/shared/api.ts`, add to the existing `settingsSchema` zod object:

```ts
  tavilyBaseUrl: z.string().nullable().optional(),
  serpapiBaseUrl: z.string().nullable().optional(),
```

Extend the `updateSystemSettings` input type and the payload it sends with `tavilyBaseUrl?: string` and `serpapiBaseUrl?: string`.

- [ ] **Step 2: Add the fields to the page**

In `apps/admin/src/features/settings/page.tsx`, add two new state hooks beside the others:

```tsx
  const [tavilyBaseUrl, setTavilyBaseUrl] = useState("");
  const [serpapiBaseUrl, setSerpapiBaseUrl] = useState("");
```

Extend the hydration `useEffect`:

```tsx
    setTavilyBaseUrl(settings.data.tavilyBaseUrl ?? "");
    setSerpapiBaseUrl(settings.data.serpapiBaseUrl ?? "");
```

And the dependency list (`settings.data?.tavilyBaseUrl, settings.data?.serpapiBaseUrl`).

Add the fields to the Web Search Card body — only render when the selected provider needs them:

```tsx
          {searchProvider === "tavily" ? (
            <label className="grid gap-2 text-sm font-medium">
              Tavily Base URL (advanced)
              <Input
                onChange={(event) => {
                  setIsDirty(true);
                  setTavilyBaseUrl(event.target.value);
                }}
                placeholder="https://api.tavily.com"
                value={tavilyBaseUrl}
              />
            </label>
          ) : null}
          {searchProvider === "serpapi" ? (
            <label className="grid gap-2 text-sm font-medium">
              SerpApi Base URL (advanced)
              <Input
                onChange={(event) => {
                  setIsDirty(true);
                  setSerpapiBaseUrl(event.target.value);
                }}
                placeholder="https://serpapi.com"
                value={serpapiBaseUrl}
              />
            </label>
          ) : null}
```

Add to `handleSave`'s payload:

```tsx
      tavilyBaseUrl,
      serpapiBaseUrl,
```

- [ ] **Step 3: Verify admin suite still passes**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS (existing settings tests still pass; the new fields are additive).

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/features/shared/api.ts apps/admin/src/features/settings/page.tsx
git commit -m "feat(admin): expose tavily/serpapi base url overrides in settings"
```

---

### Task 5: Bearer auth + provider coverage for web search (P1)

**Files:**
- Modify: `tests/rust-integration/tests/web_search_api.rs`

Today the integration test only exercises SearXNG via session+CSRF. Add (a) the Bearer-auth path against the existing mock, and (b) Tavily/SerpApi/Ollama using the new persisted base URLs.

- [ ] **Step 1: Add the Bearer-auth case**

Append to `tests/rust-integration/tests/web_search_api.rs`:

```rust
#[tokio::test]
async fn web_search_via_bearer_token() {
    let _env = TestEnvironment::start("web-search-bearer").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let (searxng_handle, searxng_base) = spawn_mock_searxng().await;

    // Configure search provider via PATCH.
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
                      "searchProvider": "searxng",
                      "searxngUrl": searxng_base,
                      "searxngCategories": ["general"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);

    // Mint a Bearer token (session+CSRF for the mint itself).
    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(json!({ "name": "web-search-bearer" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint.status(), StatusCode::CREATED);
    let token = read_json(mint.into_body())["token"].as_str().unwrap().to_string();

    // Hit /api/web-search with Authorization: Bearer (no cookie, no CSRF).
    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web-search")
                .header(header::CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({ "query": "knowledge graphs", "maxResults": 3 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response.into_body()).await;
    let results = payload.get("results").and_then(Value::as_array).unwrap();
    assert_eq!(results.len(), 1);

    searxng_handle.abort();
}
```

(The existing `spawn_mock_searxng`, `login_and_csrf`, `read_json` helpers in the file are reusable as-is.)

- [ ] **Step 2: Run**

Run: `cargo test -p rust-integration --test web_search_api -- --test-threads=1`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/rust-integration/tests/web_search_api.rs
git commit -m "test(web-search): cover bearer auth path with mock searxng"
```

---

### Task 6: API Tokens UI — project scope picker + table column (P1)

**Files:**
- Modify: `apps/admin/src/features/api-tokens/page.tsx`
- Modify: `apps/admin/src/features/api-tokens/page.test.tsx`

- [ ] **Step 1: Add a project picker**

In `apps/admin/src/features/api-tokens/page.tsx`, add state next to `name`:

```tsx
  const [scopeProjectId, setScopeProjectId] = useState<string>("");
```

Import `useProjectsQuery` (verified to exist at `apps/admin/src/features/projects/queries.ts:5`) and render a select between Token Name and the Mint button. Add to imports:

```tsx
import { useProjectsQuery } from "../projects/queries";
```

And initialise the query inside the component:

```tsx
const projectsQuery = useProjectsQuery();
```

Then render the select:

```tsx
          <label className="grid gap-2 text-sm font-medium">
            Scope
            <select
              aria-label="Token Scope"
              className="rounded border bg-background px-3 py-2"
              onChange={(event) => setScopeProjectId(event.target.value)}
              value={scopeProjectId}
            >
              <option value="">No scope (system-wide)</option>
              {(projectsQuery.data ?? []).map((project) => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          </label>
```

Update the mint call:

```tsx
              const result = await createMutation.mutateAsync({
                name: name.trim(),
                projectId: scopeProjectId.length > 0 ? scopeProjectId : null,
              });
```

- [ ] **Step 2: Add a Project column to the table**

Inside the existing `<TableHead>` row, add after Prefix:

```tsx
                  <TableHead>Project</TableHead>
```

Inside the `tokens.map(...)` body, add the matching cell:

```tsx
                    <TableCell>{token.projectId ?? "—"}</TableCell>
```

(The dash is the same em-dash used elsewhere in this page — `—`.)

- [ ] **Step 3: Update the page test**

In `apps/admin/src/features/api-tokens/page.test.tsx`, mock the projects query in the same `vi.mock(...)` block (or add a new one):

```tsx
vi.mock("../projects/queries", () => ({
  useProjectsQuery: () => ({ data: [{ id: "project-1", name: "Demo Project" }] }),
}));
```

Add a third test:

```tsx
  it("mints a project-scoped token when a scope is selected", async () => {
    const user = userEvent.setup();
    mockTokensList.mockReturnValue({ tokens: [] });
    mockCreateToken.mockResolvedValue({
      id: "token-2",
      name: "scoped",
      projectId: "project-1",
      prefix: "abcd1234",
      createdAt: "2026-06-15T00:00:00Z",
      token: "plaintext-scoped",
    });

    renderPage();

    await user.type(screen.getByLabelText("Token Name"), "scoped");
    await user.selectOptions(screen.getByLabelText("Token Scope"), "project-1");
    await user.click(screen.getByRole("button", { name: "Mint Token" }));

    expect(mockCreateToken).toHaveBeenCalledWith({
      name: "scoped",
      projectId: "project-1",
    });
  });
```

Verify the existing project-column assertion by extending the existing list test to include a token with `projectId: "project-1"` and asserting that "project-1" appears as a TableCell.

- [ ] **Step 4: Run**

Run: `npm run test --workspace @knowledge/admin -- src/features/api-tokens/page.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/api-tokens/page.tsx apps/admin/src/features/api-tokens/page.test.tsx
git commit -m "feat(admin): add scope picker and project column to api tokens page"
```

---

### Task 7: Deep research executor through `save_wiki_page` + surface outcome (P1)

**Files:**
- Modify: `crates/knowledge-server/src/tasks/executors.rs`
- Modify: `apps/admin/src/features/deep-research/page.tsx`
- Modify: `apps/admin/src/features/deep-research/page.test.tsx`
- Modify: `tests/rust-integration/tests/deep_research_api.rs`

- [ ] **Step 1: Route the page-write through `save_wiki_page`**

In `crates/knowledge-server/src/tasks/executors.rs`, replace the `std::fs::create_dir_all` + `std::fs::write` block at the end of `run_deep_research_executor` with:

```rust
  use knowledge_core::project::wiki_pages::save_wiki_page;
  save_wiki_page(&root, &relative_path, &page_content)
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
```

(`save_wiki_page` validates that the path is under `wiki/` and a `.md` file — both true for `wiki/queries/research-*.md` — and handles `create_dir_all` internally.)

- [ ] **Step 2: Add a Bearer-auth case to the integration test**

In `tests/rust-integration/tests/deep_research_api.rs`, append a Bearer test analogous to Task 5's web-search version:

```rust
#[tokio::test]
async fn deep_research_via_bearer_token() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("deep-research-bearer").await.unwrap();
    let mock_openai = MockOpenAiServer::start(MockScenario::deep_research_success())
        .await
        .unwrap();
    let (searxng_handle, searxng_base) = spawn_mock_searxng().await;
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("dr-bearer-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    configure_settings(&state, &mock_openai, &searxng_base).await;

    // Mint a project-scoped Bearer.
    let mint = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/me/api-tokens")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::from(
                    json!({ "name": "dr", "projectId": project_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let token = read_json(mint.into_body())["token"].as_str().unwrap().to_string();

    let response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/deep-research"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    json!({ "topic": "Knowledge Graphs" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let task_id = read_json(response.into_body())["taskId"].as_str().unwrap().to_string();
    wait_for_task_terminal(&state, &task_id).await;
    let task = store::get_task_by_id(&state, &task_id).await.unwrap();
    assert_eq!(task.status, "succeeded");

    searxng_handle.abort();
}
```

- [ ] **Step 3: Surface `task.result` and `task.error` in the admin page**

In `apps/admin/src/features/deep-research/page.tsx`, replace the inner content of each task `<Card>` with:

```tsx
              <CardHeader>
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="space-y-1">
                    <CardTitle>{task.title}</CardTitle>
                    <CardDescription>
                      Updated {task.updatedAt ?? "—"} · created {task.createdAt ?? "—"}
                    </CardDescription>
                  </div>
                  <StatusBadge value={task.status} />
                </div>
              </CardHeader>
              <CardContent className="grid gap-2 text-sm">
                {renderTaskResult(task)}
              </CardContent>
```

And add the helper above the `return`:

```tsx
function renderTaskResult(task: { result?: unknown; error?: unknown }) {
  if (task.error) {
    const message =
      typeof task.error === "object" && task.error && "message" in task.error
        ? String((task.error as { message?: unknown }).message ?? task.error)
        : String(task.error);
    return <p className="text-destructive">Error: {message}</p>;
  }
  if (
    typeof task.result === "object" &&
    task.result &&
    "savedPath" in task.result
  ) {
    const result = task.result as {
      savedPath?: string;
      sourceCount?: number;
      errors?: string[];
    };
    return (
      <div className="grid gap-1">
        {result.savedPath ? (
          <p>
            Saved: <code>{result.savedPath}</code>
          </p>
        ) : null}
        {typeof result.sourceCount === "number" ? (
          <p>Sources used: {result.sourceCount}</p>
        ) : null}
        {result.errors && result.errors.length > 0 ? (
          <p className="text-muted-foreground">
            Source errors: {result.errors.join("; ")}
          </p>
        ) : null}
      </div>
    );
  }
  return null;
}
```

- [ ] **Step 4: Extend the page test to assert the new output**

In `apps/admin/src/features/deep-research/page.test.tsx`, add a third test:

```tsx
  it("renders the saved path and source count for a succeeded task", () => {
    mockTasks.mockReturnValue([
      {
        id: "task-1",
        taskType: "project.deep_research",
        title: "Deep research: KG",
        status: "succeeded",
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:01:00Z",
        result: {
          savedPath: "wiki/queries/research-kg.md",
          sourceCount: 3,
          errors: [],
        },
      },
    ]);
    renderPage();
    expect(screen.getByText("wiki/queries/research-kg.md")).toBeInTheDocument();
    expect(screen.getByText("Sources used: 3")).toBeInTheDocument();
  });

  it("renders the error message for a failed task", () => {
    mockTasks.mockReturnValue([
      {
        id: "task-1",
        taskType: "project.deep_research",
        title: "Deep research: KG",
        status: "failed",
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:01:00Z",
        error: { message: "no sources found" },
      },
    ]);
    renderPage();
    expect(screen.getByText(/Error: no sources found/)).toBeInTheDocument();
  });
```

- [ ] **Step 5: Run all tests**

Run: `cargo test -p rust-integration --test deep_research_api -- --test-threads=1`
Expected: 2 tests PASS (existing + new Bearer case).

Run: `npm run test --workspace @knowledge/admin -- src/features/deep-research/page.test.tsx`
Expected: 4 tests PASS (existing 2 + new 2).

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/tasks/executors.rs tests/rust-integration/tests/deep_research_api.rs apps/admin/src/features/deep-research/page.tsx apps/admin/src/features/deep-research/page.test.tsx
git commit -m "feat(deep-research): route saves through save_wiki_page and surface outcome"
```

---

### Task 8: MCP stdio integration coverage (P1)

**Files:**
- Modify: `packages/mcp-server/test/server.test.ts`

The current integration test covers `tools/list` and one call to `knowledge_projects` plus one parameter-validation failure. Extend the in-process HTTP fake to handle the rest of the REST endpoints used by the 8 tools, then add round-trip tests for each.

- [ ] **Step 1: Extend the fake server**

In the existing `beforeAll`, extend the request handler to respond to:

- `GET /api/projects/p1/files?root=wiki&recursive=true` → `{"files":[{"name":"index.md","path":"wiki/index.md","isDir":false}],"truncated":false}`
- `GET /api/projects/p1/files/content?path=wiki/index.md` → `{"path":"wiki/index.md","content":"# Index"}`
- `GET /api/projects/p1/reviews?status=unresolved` → `{"reviews":[{"id":"r1","status":"unresolved","type":"missing-page","title":"Missing","options":[]}]}`
- `POST /api/projects/p1/search` → `{"mode":"keyword","tokenHits":1,"vectorHits":0,"results":[{"path":"wiki/index.md","title":"Index","snippet":"hit","score":0.5}]}`
- `GET /api/projects/p1/graph` → `{"nodes":[{"id":"a","label":"A","type":"concept","linkCount":3}],"edges":[]}`
- `POST /api/projects/p1/sources:rescan` → 202 `{"taskId":"task-1","status":"queued"}`

The existing handler uses a simple `if/else` chain; extend it linearly. Keep response sizes small.

- [ ] **Step 2: Add round-trip tests**

Append a sixth test that loops through the remaining tools:

```ts
it("round-trips files, read_file, reviews, search, graph, and rescan_sources", async () => {
  const transport = new StdioClientTransport({
    command: process.execPath,
    args: [entry],
    env: {
      ...process.env,
      KNOWLEDGE_API_BASE_URL: `http://127.0.0.1:${port}`,
      KNOWLEDGE_API_TOKEN: "test-token",
    },
  });
  const client = new Client({ name: "test-client", version: "0.0.0" }, { capabilities: {} });
  await client.connect(transport);
  try {
    const calls: Array<{ name: string; args: Record<string, unknown>; expect: string }> = [
      { name: "knowledge_files", args: { project_id: "p1" }, expect: "wiki/index.md" },
      { name: "knowledge_read_file", args: { project_id: "p1", path: "wiki/index.md" }, expect: "# Index" },
      { name: "knowledge_reviews", args: { project_id: "p1" }, expect: "missing-page" },
      { name: "knowledge_search", args: { project_id: "p1", query: "hit" }, expect: "Index" },
      { name: "knowledge_graph", args: { project_id: "p1" }, expect: "A (concept" },
      { name: "knowledge_rescan_sources", args: { project_id: "p1" }, expect: "task-1" },
    ];
    for (const call of calls) {
      const result = await client.callTool({ name: call.name, arguments: call.args });
      const text =
        Array.isArray(result.content) && result.content[0]?.type === "text"
          ? String(result.content[0].text)
          : "";
      expect(text).toContain(call.expect);
    }
  } finally {
    await client.close();
  }
}, 20_000);
```

Also add a coverage test for `knowledge_status` when the token is missing:

```ts
it("knowledge_status reports projects error when token is missing", async () => {
  const transport = new StdioClientTransport({
    command: process.execPath,
    args: [entry],
    env: {
      ...process.env,
      KNOWLEDGE_API_BASE_URL: `http://127.0.0.1:${port}`,
      KNOWLEDGE_API_TOKEN: "",
    },
  });
  const client = new Client({ name: "test-client", version: "0.0.0" }, { capabilities: {} });
  await client.connect(transport);
  try {
    const result = await client.callTool({ name: "knowledge_status", arguments: {} });
    const text =
      Array.isArray(result.content) && result.content[0]?.type === "text"
        ? String(result.content[0].text)
        : "";
    // health succeeds, projects fails — the rendered JSON contains "error".
    expect(text).toContain("\"error\"");
  } finally {
    await client.close();
  }
}, 15_000);
```

(For this to work, the fake server's `/api/projects` handler must reject requests without `Authorization: Bearer`. Add a check at the top of that endpoint: if `req.headers["authorization"]` is empty, respond 401 with `{"error":"missing session"}`.)

- [ ] **Step 3: Run**

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: 21 tests PASS (existing 19 + 2 new).

- [ ] **Step 4: Commit**

```bash
git add packages/mcp-server/test/server.test.ts
git commit -m "test(mcp): cover remaining 6 tools and unauthenticated status"
```

---

### Task 9: Settings polish — disable Test Search while dirty + use `resolve_principal` (P2)

**Files:**
- Modify: `apps/admin/src/features/settings/page.tsx`
- Modify: `crates/knowledge-server/src/settings/routes.rs`

- [ ] **Step 1: Disable Test Search while the form is dirty**

In `apps/admin/src/features/settings/page.tsx`, change the Test Search button's `disabled` predicate to include `isDirty`:

```tsx
              disabled={
                runWebSearch.isPending ||
                testQuery.trim().length === 0 ||
                isDirty
              }
```

Add a one-line hint below the button when `isDirty`:

```tsx
            {isDirty ? (
              <p className="text-xs text-muted-foreground">
                Save settings before testing — current config in the form differs from the saved config.
              </p>
            ) : null}
```

- [ ] **Step 2: Replace hand-rolled session extraction with `resolve_principal`**

In `crates/knowledge-server/src/settings/routes.rs`, drop the `require_session` helper (lines ~181-198 in the current file) and the manual cookie parsing inside it. Change `get_settings` and `update_settings` to:

```rust
async fn get_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
  let principal = crate::auth::principal::resolve_principal(&state, &headers).await?;
  let _ = principal; // settings reads are admin-only; any authenticated user is allowed.
  Ok(Json(build_settings_response(&state).await?))
}

async fn update_settings(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
  let principal = crate::auth::principal::resolve_principal(&state, &headers).await?;
  if principal.requires_csrf() {
    let expected = principal
      .csrf_token
      .as_deref()
      .ok_or_else(|| ApiError::unauthorized("missing csrf token"))?;
    let supplied = headers
      .get("x-csrf-token")
      .and_then(|value| value.to_str().ok())
      .unwrap_or_default();
    if supplied.is_empty() || supplied != expected {
      return Err(ApiError::unauthorized("invalid csrf token"));
    }
  }
  // ... existing UPDATE body ...
  Ok(Json(build_settings_response(&state).await?))
}
```

Drop `find_session` and `header` imports if they become unused.

- [ ] **Step 3: Run**

Run: `cargo test -p rust-integration -- --test-threads=1`
Expected: all PASS (settings GET/PATCH still works via session or Bearer; Bearer was previously rejected because of hand-rolled cookie parsing — that's an unblock, not a regression).

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/features/settings/page.tsx crates/knowledge-server/src/settings/routes.rs
git commit -m "refactor(settings): use resolve_principal and warn on dirty test search"
```

---

### Task 10: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Rust lint + tests**

Run: `cargo fmt --all -- --check`
Expected: clean.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo test --workspace -- --test-threads=1`
Expected: PASS — including the new scope-containment tests, settings response-truth test, web-search bearer test, deep-research bearer test, and (unchanged) Phase 4 baseline.

- [ ] **Step 2: Frontend tests + lint**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS — including the new api-tokens scope test and deep-research result/error tests.

Run: `npm run test --workspace @knowledge/api-client`
Expected: PASS (regression).

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: PASS — including the 2 new integration tests.

Run: `npm run lint`
Expected: tsc + clippy clean.

- [ ] **Step 3: Playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: 10 specs PASS. (No e2e is added in this fix plan because every change is internal-correctness or covered by unit/integration tests at a closer scope.)

- [ ] **Step 4: Final commit (only if fixes were needed)**

```bash
git status
```

If steps 1–3 required fixes, commit them. Otherwise the work is complete.
