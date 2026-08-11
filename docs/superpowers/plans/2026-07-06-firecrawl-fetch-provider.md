# Firecrawl Fetch Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the canvas URL node's built-in direct web fetch with a Firecrawl-only external scraping provider, configured via a new `system_settings` selector + per-provider JSONB map.

**Architecture:** Mirror the existing `search_provider` + `search_provider_configs` pattern exactly. A new `web_fetch/` module holds the provider enum, config resolver, and Firecrawl HTTP client. The URL-node handler loads the configured provider and scrapes through it; when no provider is set it returns a normal `status:"error"` with a "not configured" message. The built-in direct fetch (SSRF machinery, meta-refresh follower, HTML→markdown converter) and the `allow_private_fetch` chain are deleted end-to-end.

**Tech Stack:** Rust (axum, sqlx/Postgres, reqwest, tokio) backend; React 19 / Vite / TypeScript / zod / shadcn-ui admin SPA; vitest + cargo test.

**Spec:** `docs/superpowers/specs/2026-07-06-firecrawl-fetch-provider-design.md`

---

## Notes for the implementer

- **Working directory:** repo root is `E:\Projects\Js\knowledge`. Backend commands run from the repo root. Frontend commands run from `apps/admin`.
- **Migration numbering:** `0016_drop_legacy_provider_columns.sql` already exists, so the new migration is `0017`. `MIGRATOR = sqlx::migrate!("./migrations")` runs migrations numerically on boot; the test DB gets them applied when the suite starts.
- **The spec is the source of truth.** Where this plan and the spec differ, ask before proceeding.
- **Confirm names against the codebase as you go.** This plan quotes signatures from the read-through, but if a helper name or import path differs at implementation time, match the codebase, not the plan.
- **DRY / YAGNI / TDD / frequent commits.** Each task ends with a commit.

---

## Task 1: Migration 0017 — fetch provider columns

**Files:**
- Create: `crates/knowledge-server/migrations/0017_fetch_provider_settings.sql`

- [ ] **Step 1: Write the migration**

Create `crates/knowledge-server/migrations/0017_fetch_provider_settings.sql`:

```sql
-- Firecrawl-only web fetch provider config, mirroring search_provider +
-- search_provider_configs. Selector defaults to 'none' so a fresh install has no
-- fetch provider until one is set in the admin UI; the URL node returns a clear
-- "not configured" error until then.
ALTER TABLE system_settings ADD COLUMN fetch_provider TEXT NOT NULL DEFAULT 'none';
ALTER TABLE system_settings ADD COLUMN fetch_provider_configs JSONB NOT NULL DEFAULT '{}'::jsonb;
```

- [ ] **Step 2: Verify the migration applies**

Run (from repo root): `cargo test -p knowledge-server --lib migrate`
Expected: the migration suite loads without error (any migration-count assertion in `db/migrate.rs` that checks a specific highest version may need bumping — if a test asserts `MIGRATOR.iter().any(|m| m.version == N)` for the previous top version it still passes; only fix if a test explicitly pins the count). If a test fails asserting the latest version, update it to include version 17.

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/migrations/0017_fetch_provider_settings.sql
git commit -m "feat: add fetch_provider settings columns (migration 0017)"
```

---

## Task 2: `web_fetch` module scaffold + `FetchProvider` enum

**Files:**
- Create: `crates/knowledge-server/src/web_fetch/mod.rs`
- Modify: `crates/knowledge-server/src/lib.rs` (register `pub mod web_fetch;`)

- [ ] **Step 1: Create `web_fetch/mod.rs`**

```rust
pub mod config;
pub mod firecrawl;

/// A page reduced to readable markdown plus its title. Firecrawl is the only
/// producer now that the built-in direct fetch is gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedPage {
    pub title: String,
    pub markdown: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchProvider {
    Firecrawl,
}

impl FetchProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            FetchProvider::Firecrawl => "firecrawl",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_provider_as_str_is_firecrawl() {
        assert_eq!(FetchProvider::Firecrawl.as_str(), "firecrawl");
    }
}
```

> Note: `ExtractedPage` is defined here (moved from `canvas/service.rs`). Task 6 removes the old definition; until then two definitions may coexist — that is fine because `canvas/service.rs`'s copy is still referenced by the old `fetch_url` code that Task 6 deletes. If the compiler complains about an unused/duplicate before Task 6, proceed — Task 6 resolves it. To avoid a transient duplicate-name error, do NOT import `web_fetch::ExtractedPage` into `canvas` until Task 6.

- [ ] **Step 2: Register the module in `lib.rs`**

In `crates/knowledge-server/src/lib.rs`, add alongside the other `pub mod` declarations (keep alphabetical order if the file uses it; otherwise place it next to `pub mod web_search;`):

```rust
pub mod web_fetch;
```

- [ ] **Step 3: Verify it compiles**

`web_fetch/mod.rs` declares `pub mod config;` and `pub mod firecrawl;`, so those files must exist for the crate to compile. Create both now as stub files (use the Write tool, not a shell redirect) so this task compiles standalone; Tasks 3 and 4 fill them in:

- `crates/knowledge-server/src/web_fetch/config.rs` with a single line: `// filled in by Task 3`
- `crates/knowledge-server/src/web_fetch/firecrawl.rs` with a single line: `// filled in by Task 4`

Then run: `cargo check -p knowledge-server`
Expected: compiles (empty modules are valid Rust).

- [ ] **Step 4: Run the enum test**

Run: `cargo test -p knowledge-server --lib web_fetch::tests::fetch_provider_as_str_is_firecrawl`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/web_fetch/mod.rs crates/knowledge-server/src/web_fetch/config.rs crates/knowledge-server/src/web_fetch/firecrawl.rs crates/knowledge-server/src/lib.rs
git commit -m "feat: scaffold web_fetch module with FetchProvider enum"
```

---

## Task 3: `web_fetch/config.rs` — parse + resolve + load (TDD)

**Files:**
- Modify: `crates/knowledge-server/src/web_fetch/config.rs`

Reference: mirror `crates/knowledge-server/src/web_search/config.rs`. Confirm the exact `ApiError` constructor names and the `AppState` import path against that file during implementation.

- [ ] **Step 1: Write the failing tests**

Replace the placeholder contents of `web_fetch/config.rs` with the full module below (tests included). Write it in one shot since the tests reference the very functions we're adding:

```rust
use serde_json::Value;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::web_fetch::FetchProvider;

pub const DEFAULT_FIRECRAWL_BASE_URL: &str = "https://api.firecrawl.dev";

#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub provider: FetchProvider,
    pub base_url: String,
    pub api_key: Option<String>,
}

/// "none" -> Ok(None); "firecrawl" -> Ok(Some(Firecrawl)); anything else is an
/// internal error (the selector column is validated on write).
pub(crate) fn parse_fetch_provider(value: &str) -> Result<Option<FetchProvider>, ApiError> {
    match value {
        "none" => Ok(None),
        "firecrawl" => Ok(Some(FetchProvider::Firecrawl)),
        other => Err(ApiError::internal(format!(
            "unknown fetch provider stored in system_settings: {other}"
        ))),
    }
}

/// Pure resolver: reads configs["firecrawl"] { baseUrl, apiKey } and applies the
/// base-url default when the stored value is blank/absent.
pub(crate) fn resolve_fetch_config(
    active: &str,
    configs: &Value,
) -> Result<Option<FetchConfig>, ApiError> {
    let Some(provider) = parse_fetch_provider(active)? else {
        return Ok(None);
    };

    let str_field = |field: &str| -> Option<String> {
        configs
            .get("firecrawl")
            .and_then(|block| block.get(field))
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|v| !v.trim().is_empty())
    };

    let base_url = str_field("baseUrl").unwrap_or_else(|| DEFAULT_FIRECRAWL_BASE_URL.to_string());
    let api_key = str_field("apiKey");

    Ok(Some(FetchConfig {
        provider,
        base_url,
        api_key,
    }))
}

/// SELECT the selector + config map from the singleton row, then resolve.
pub async fn load_fetch_config(state: &AppState) -> Result<Option<FetchConfig>, ApiError> {
    let (provider, configs): (String, Value) = sqlx::query_as(
        "SELECT fetch_provider, fetch_provider_configs FROM system_settings WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|err| ApiError::internal(format!("failed to load fetch provider settings: {err}")))?;

    resolve_fetch_config(&provider, &configs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_fetch_provider_maps_known_values() {
        assert_eq!(parse_fetch_provider("none").unwrap(), None);
        assert_eq!(
            parse_fetch_provider("firecrawl").unwrap(),
            Some(FetchProvider::Firecrawl)
        );
    }

    #[test]
    fn parse_fetch_provider_rejects_unknown() {
        assert!(parse_fetch_provider("wget").is_err());
    }

    #[test]
    fn resolve_returns_none_for_none_selector() {
        let configs = json!({ "firecrawl": { "baseUrl": "http://x", "apiKey": "k" } });
        assert!(resolve_fetch_config("none", &configs).unwrap().is_none());
    }

    #[test]
    fn resolve_reads_firecrawl_block() {
        let configs = json!({ "firecrawl": { "baseUrl": "http://host:3002", "apiKey": "sk-1" } });
        let cfg = resolve_fetch_config("firecrawl", &configs).unwrap().unwrap();
        assert_eq!(cfg.provider, FetchProvider::Firecrawl);
        assert_eq!(cfg.base_url, "http://host:3002");
        assert_eq!(cfg.api_key.as_deref(), Some("sk-1"));
    }

    #[test]
    fn resolve_applies_base_url_default_when_absent() {
        let configs = json!({ "firecrawl": { "apiKey": "sk-1" } });
        let cfg = resolve_fetch_config("firecrawl", &configs).unwrap().unwrap();
        assert_eq!(cfg.base_url, DEFAULT_FIRECRAWL_BASE_URL);
        assert_eq!(cfg.api_key.as_deref(), Some("sk-1"));
    }

    #[test]
    fn resolve_applies_base_url_default_when_blank() {
        let configs = json!({ "firecrawl": { "baseUrl": "   ", "apiKey": "sk-1" } });
        let cfg = resolve_fetch_config("firecrawl", &configs).unwrap().unwrap();
        assert_eq!(cfg.base_url, DEFAULT_FIRECRAWL_BASE_URL);
    }

    #[test]
    fn resolve_api_key_absent_is_none() {
        let configs = json!({ "firecrawl": { "baseUrl": "http://host:3002" } });
        let cfg = resolve_fetch_config("firecrawl", &configs).unwrap().unwrap();
        assert_eq!(cfg.api_key, None);
    }
}
```

- [ ] **Step 2: Run the tests — expect fail first, then pass**

Because the module and tests are added together, run:
Run: `cargo test -p knowledge-server --lib web_fetch::config`
Expected: PASS (all 7 tests). If `ApiError::internal` or `AppState` import path differs, fix the import to match `web_search/config.rs` and re-run.

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/src/web_fetch/config.rs
git commit -m "feat: add web_fetch config parse/resolve/load with tests"
```

---

## Task 4: `web_fetch/firecrawl.rs` — scrape via /v1/scrape (TDD)

**Files:**
- Modify: `crates/knowledge-server/src/web_fetch/firecrawl.rs`

Reference: mirror the HTTP client shape and the `spawn_mock_tavily` TCP-mock test pattern from `crates/knowledge-server/src/web_search/provider.rs`.

- [ ] **Step 1: Write the module + failing tests**

Replace the placeholder contents of `web_fetch/firecrawl.rs`:

```rust
use std::time::Duration;

use serde::Deserialize;

use crate::http::error::ApiError;
use crate::web_fetch::config::FetchConfig;
use crate::web_fetch::ExtractedPage;

#[derive(Debug, Deserialize)]
struct FirecrawlResponse {
    success: Option<bool>,
    data: Option<FirecrawlData>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FirecrawlData {
    markdown: Option<String>,
    metadata: Option<FirecrawlMetadata>,
}

#[derive(Debug, Deserialize)]
struct FirecrawlMetadata {
    title: Option<String>,
}

/// Scrape a URL via Firecrawl's /v1/scrape endpoint and return readable markdown.
/// POST {base_url}/v1/scrape with { url, formats:["markdown"], onlyMainContent:true },
/// Bearer auth when an api_key is present. Renders JS, so a 60s timeout.
pub async fn scrape(config: &FetchConfig, url: &str) -> Result<ExtractedPage, ApiError> {
    let endpoint = format!("{}/v1/scrape", config.base_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| ApiError::internal(format!("failed to build fetch client: {err}")))?;

    let mut request = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "url": url,
            "formats": ["markdown"],
            "onlyMainContent": true,
        }));

    if let Some(key) = config.api_key.as_deref().filter(|k| !k.trim().is_empty()) {
        request = request.header("Authorization", format!("Bearer {key}"));
    }

    let response = request
        .send()
        .await
        .map_err(|err| ApiError::bad_request(format!("firecrawl request failed: {err}")))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| ApiError::bad_request(format!("firecrawl response read failed: {err}")))?;

    if !status.is_success() {
        return Err(ApiError::bad_request(format!(
            "firecrawl returned {status}: {body}"
        )));
    }

    let parsed: FirecrawlResponse = serde_json::from_str(&body).map_err(|err| {
        ApiError::bad_request(format!("firecrawl response was not valid JSON: {err}; body: {body}"))
    })?;

    if parsed.success == Some(false) {
        let message = parsed.error.unwrap_or_else(|| "firecrawl reported failure".to_string());
        return Err(ApiError::bad_request(format!("firecrawl error: {message}")));
    }

    let data = parsed
        .data
        .ok_or_else(|| ApiError::bad_request("firecrawl response had no data".to_string()))?;

    let title = data
        .metadata
        .and_then(|m| m.title)
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Untitled".to_string());

    Ok(ExtractedPage {
        title,
        markdown: data.markdown.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web_fetch::FetchProvider;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Spawn a one-shot TCP server that returns `body` as an HTTP response with the
    /// given status line, and return the base_url pointing at it. Mirrors the
    /// spawn_mock_tavily pattern in web_search/provider.rs.
    async fn spawn_mock(status_line: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            // Drain the request (best-effort; we don't parse it).
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let response = format!(
                "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        });
        format!("http://{addr}")
    }

    fn config_for(base_url: String) -> FetchConfig {
        FetchConfig {
            provider: FetchProvider::Firecrawl,
            base_url,
            api_key: Some("sk-test".to_string()),
        }
    }

    #[tokio::test]
    async fn scrape_maps_success_to_extracted_page() {
        let base = spawn_mock(
            "200 OK",
            r#"{"success":true,"data":{"markdown":"# Hello","metadata":{"title":"Example"}}}"#,
        )
        .await;
        let page = scrape(&config_for(base), "https://example.com").await.unwrap();
        assert_eq!(page.title, "Example");
        assert_eq!(page.markdown, "# Hello");
    }

    #[tokio::test]
    async fn scrape_defaults_title_when_absent() {
        let base = spawn_mock(
            "200 OK",
            r#"{"success":true,"data":{"markdown":"body only"}}"#,
        )
        .await;
        let page = scrape(&config_for(base), "https://example.com").await.unwrap();
        assert_eq!(page.title, "Untitled");
        assert_eq!(page.markdown, "body only");
    }

    #[tokio::test]
    async fn scrape_maps_success_false_to_bad_request() {
        let base = spawn_mock(
            "200 OK",
            r#"{"success":false,"error":"could not reach target"}"#,
        )
        .await;
        let err = scrape(&config_for(base), "https://example.com").await.unwrap_err();
        assert!(err.to_string().contains("could not reach target"));
    }

    #[tokio::test]
    async fn scrape_maps_non_2xx_to_bad_request() {
        let base = spawn_mock("500 Internal Server Error", r#"{"error":"boom"}"#).await;
        let err = scrape(&config_for(base), "https://example.com").await.unwrap_err();
        assert!(err.to_string().contains("500"));
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p knowledge-server --lib web_fetch::firecrawl`
Expected: PASS (4 tests). If `ApiError::bad_request` takes a `&str` vs `String`, or the mock's socket handling needs a tweak against the `web_search/provider.rs` pattern, adjust to match and re-run.

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/src/web_fetch/firecrawl.rs
git commit -m "feat: add Firecrawl scrape client with TCP-mock tests"
```

---

## Task 5: Rewire `extract_url_handler`

**Files:**
- Modify: `crates/knowledge-server/src/canvas/routes.rs`

- [ ] **Step 1: Rewrite the handler body**

Find `extract_url_handler` (around line 716). Keep its existing signature and the `resolve_principal` / `require_csrf` preamble. Replace the fetch body with:

```rust
    let Some(config) = crate::web_fetch::config::load_fetch_config(&state).await? else {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "title": "",
            "markdown": "",
            "error": "web page fetch is not configured; set a fetch provider in Settings"
        })));
    };

    match crate::web_fetch::firecrawl::scrape(&config, &body.url).await {
        Ok(page) => Ok(Json(serde_json::json!({
            "status": "ok",
            "title": page.title,
            "markdown": page.markdown,
            "error": null
        }))),
        Err(error) => Ok(Json(serde_json::json!({
            "status": "error",
            "title": "",
            "markdown": "",
            "error": error.to_string()
        }))),
    }
```

Notes:
- The handler no longer references `state.allow_private_fetch`, the old `fetch_url`, `build_extractor_client`, or `ExtractedPage` from `canvas::service`. Remove any now-unused local variables/imports the compiler flags (do this after Task 6 deletes the service-side machinery, or now if the compiler complains).
- `ExtractUrlRequest { url: String }` (near line 659) is unchanged.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p knowledge-server`
Expected: compiles (the `allow_private_fetch` field on `AppState` still exists until Task 7 — this handler simply stops reading it). If a previously-used import like `use crate::canvas::service::fetch_url;` is now unused, remove it.

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs
git commit -m "feat: fetch URL node through configured web_fetch provider"
```

---

## Task 6: Remove built-in fetch machinery from `canvas/service.rs`

**Files:**
- Modify: `crates/knowledge-server/src/canvas/service.rs`

- [ ] **Step 1: Delete the fetch functions and their tests**

Delete these items in their entirety from `canvas/service.rs`:
- `html_to_markdown`, `inline_markdown`
- `const MAX_EXTRACT_BYTES`
- `validate_public_url`, `is_blocked_ip`, `screen_resolved_addrs`, `PublicOnlyResolver`, `box_dns_err`, `build_extractor_client`
- `attr_value`, `meta_refresh_target`
- `fetch_url`
- the old `struct ExtractedPage` definition (its replacement now lives in `web_fetch/mod.rs`)
- the entire `mod url_tests { ... }` block

KEEP: `collect_reference_blocks`, `referenced_kb_project_ids`, `search_results_to_markdown`, `build_analyze_prompt`, `SearchResultEntry`, and the entire `mod context_tests { ... }`.

- [ ] **Step 2: Remove now-unused imports**

Remove imports the compiler flags as unused after the deletion. Expected candidates: `scraper::*`, `std::net::*` (IpAddr/SocketAddr), `std::sync::Arc`, `std::time::Duration`, `std::collections::HashSet`, and any reqwest DNS-resolver types. Do NOT remove imports still used by the kept context functions.

- [ ] **Step 3: If any kept code referenced `ExtractedPage`, point it at `web_fetch`**

Per the spec, `collect_reference_blocks` reads node data and does NOT use `ExtractedPage`, so no re-import should be needed. If the compiler shows a residual reference, add `use crate::web_fetch::ExtractedPage;` rather than redefining it.

- [ ] **Step 4: Verify compile + kept tests**

Run: `cargo test -p knowledge-server --lib canvas::service`
Expected: compiles; `context_tests` pass; `url_tests` are gone.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/service.rs
git commit -m "refactor: remove built-in direct fetch + SSRF machinery from canvas service"
```

---

## Task 7: Remove the `allow_private_fetch` chain end-to-end

**Files:**
- Modify: `crates/knowledge-server/src/config.rs`
- Modify: `crates/knowledge-server/src/app/state.rs`
- Modify: `crates/knowledge-server/src/lib.rs`
- Modify: `crates/knowledge-server/src/canvas/routes.rs` (if any residual reference remains)
- Modify: `tests/rust-integration/tests/support/mod.rs`
- Modify: `tests/rust-integration/tests/docker_stack_smoke.rs`
- Modify: `tests/rust-integration/tests/provider_connections_race.rs`
- Modify: `docker-compose.yml`

- [ ] **Step 1: `config.rs` — remove the field + reads**

Remove `allow_private_fetch` from the `AppConfig` struct, from `for_tests` (if present), from the `from_env` read (the `KNOWLEDGE_ALLOW_PRIVATE_FETCH` parse), and from the struct initializer.

- [ ] **Step 2: `app/state.rs` — remove the field**

Remove `allow_private_fetch` from the `AppState` struct and from wherever it is populated.

- [ ] **Step 3: `lib.rs` — remove the initializer**

In `bootstrap_state` (and any `AppState { .. }` literal), remove the `allow_private_fetch: config.allow_private_fetch,` line.

- [ ] **Step 4: Integration test literals**

In each of these, delete the `allow_private_fetch: false,` (or `: true`) line from the inline `AppConfig` literal:
- `tests/rust-integration/tests/support/mod.rs` (the `bootstrap_state_without_scheduler` helper's `AppConfig`, around line 105)
- `tests/rust-integration/tests/docker_stack_smoke.rs` (around line 20)
- `tests/rust-integration/tests/provider_connections_race.rs` (two occurrences: around lines 27 and 86)

- [ ] **Step 5: `docker-compose.yml` — remove the env + comment**

Delete the `KNOWLEDGE_ALLOW_PRIVATE_FETCH: "true"` line and the 4-line explanatory comment block directly above it (lines describing the fake-IP proxy / SSRF screening).

- [ ] **Step 6: Verify**

Run: `cargo check -p knowledge-server`
Expected: compiles.
Run: `cargo test -p knowledge-server --lib`
Expected: all lib tests pass.
(If the integration crate compiles standalone: `cargo check --tests` from repo root — expected clean.)

- [ ] **Step 7: Commit**

```bash
git add crates/knowledge-server/src/config.rs crates/knowledge-server/src/app/state.rs crates/knowledge-server/src/lib.rs crates/knowledge-server/src/canvas/routes.rs tests/rust-integration/tests/support/mod.rs tests/rust-integration/tests/docker_stack_smoke.rs tests/rust-integration/tests/provider_connections_race.rs docker-compose.yml
git commit -m "refactor: remove allow_private_fetch chain (built-in fetch gone)"
```

---

## Task 8: Settings `fetch` block — validate, merge, redact, response (TDD)

**Files:**
- Modify: `crates/knowledge-server/src/settings/routes.rs`

Reference the existing `search` handling in the same file: `SearchSettingsBlock`, `validate_search_provider`, `merge_search_provider_configs`, `redact_search_configs`, `build_settings_response`, and the `search` branch of `update_settings`.

- [ ] **Step 1: Factor the shared deep-merge helper + add fetch merge test (failing)**

The spec calls for one generic merge helper used by both search and fetch (DRY). Add a helper that both call. If `merge_search_provider_configs` currently hard-codes provider keys, extract its per-field merge semantics (blank string = keep stored, null = clear, other value = set) into a reusable function.

Add a test next to the existing search-merge tests:

```rust
#[test]
fn merge_fetch_provider_configs_applies_blank_keep_null_clear_set() {
    let stored = serde_json::json!({
        "firecrawl": { "baseUrl": "http://old", "apiKey": "old-key" }
    });
    let incoming = serde_json::json!({
        "firecrawl": { "baseUrl": "http://new", "apiKey": "" }
    });
    let merged = merge_fetch_provider_configs(&stored, &incoming);
    // blank apiKey keeps the stored key; non-blank baseUrl overwrites.
    assert_eq!(merged["firecrawl"]["baseUrl"], "http://new");
    assert_eq!(merged["firecrawl"]["apiKey"], "old-key");
}

#[test]
fn merge_fetch_provider_configs_null_clears() {
    let stored = serde_json::json!({ "firecrawl": { "apiKey": "old-key" } });
    let incoming = serde_json::json!({ "firecrawl": { "apiKey": null } });
    let merged = merge_fetch_provider_configs(&stored, &incoming);
    assert!(merged["firecrawl"].get("apiKey").map_or(true, |v| v.is_null()));
}
```

- [ ] **Step 2: Run — expect fail**

Run: `cargo test -p knowledge-server --lib settings::routes -- merge_fetch_provider_configs`
Expected: FAIL (`merge_fetch_provider_configs` not defined).

- [ ] **Step 3: Add `merge_fetch_provider_configs` + shared helper**

Implement `merge_fetch_provider_configs(stored, incoming)` by delegating to the shared per-field deep-merge helper (the same one `merge_search_provider_configs` now uses). Match the existing semantics exactly.

- [ ] **Step 4: Add `validate_fetch_provider` + its test**

```rust
fn validate_fetch_provider(provider: &str) -> Result<(), ApiError> {
    match provider {
        "none" | "firecrawl" => Ok(()),
        other => Err(ApiError::bad_request(format!("unknown fetch provider: {other}"))),
    }
}

#[test]
fn validate_fetch_provider_accepts_known_rejects_unknown() {
    assert!(validate_fetch_provider("none").is_ok());
    assert!(validate_fetch_provider("firecrawl").is_ok());
    assert!(validate_fetch_provider("wget").is_err());
}
```

- [ ] **Step 5: Add the request block type**

Mirror `SearchSettingsBlock`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchSettingsBlock {
    provider: Option<String>,
    providers: Option<serde_json::Value>,
}
```

And add to `UpdateSettingsRequest`:

```rust
    #[serde(default)]
    fetch: Option<FetchSettingsBlock>,
```

- [ ] **Step 6: Handle `fetch` in `update_settings`**

Mirror the `search` handler. Before the transaction opens, if `fetch.provider` is present, `validate_fetch_provider(&provider)?`. Inside the update:
- when `provider` present: `UPDATE system_settings SET fetch_provider = $1 WHERE id = 1`.
- when `providers` present: read current `fetch_provider_configs`, `merge_fetch_provider_configs`, write back the merged JSONB (same read-merge-write shape as search).

- [ ] **Step 7: Add `redact_fetch_configs` + wire into `build_settings_response`**

```rust
fn redact_fetch_configs(configs: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    if let Some(fc) = configs.get("firecrawl") {
        let mut block = serde_json::Map::new();
        let configured = fc
            .get("apiKey")
            .and_then(serde_json::Value::as_str)
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false);
        block.insert("apiKeyConfigured".into(), serde_json::Value::Bool(configured));
        if let Some(base) = fc.get("baseUrl") {
            block.insert("baseUrl".into(), base.clone());
        }
        out.insert("firecrawl".into(), serde_json::Value::Object(block));
    }
    serde_json::Value::Object(out)
}
```

In `build_settings_response`, extend the SELECT to include `fetch_provider` and `fetch_provider_configs`, and add a `fetch` section to the JSON:

```rust
"fetch": {
    "provider": fetch_provider,
    "providers": redact_fetch_configs(&fetch_provider_configs),
},
```

(Adjust the `query_as` tuple/struct to carry the two new columns, mirroring how `search_provider` + `search_provider_configs` are already selected.)

- [ ] **Step 8: Run the settings tests**

Run: `cargo test -p knowledge-server --lib settings::routes`
Expected: PASS (new merge + validate tests, plus existing tests still green).

- [ ] **Step 9: Commit**

```bash
git add crates/knowledge-server/src/settings/routes.rs
git commit -m "feat: settings fetch provider block (validate/merge/redact/response)"
```

---

## Task 9: Frontend `api.ts` — schema + update payload

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts`

- [ ] **Step 1: Add the fetch config schema**

Near `searchProviderConfigsSchema` (or wherever the search schema lives), add:

```ts
const fetchProviderConfigsSchema = z
  .object({
    firecrawl: z
      .object({
        apiKeyConfigured: z.boolean().optional(),
        baseUrl: z.string().optional(),
      })
      .optional(),
  })
  .optional();
```

- [ ] **Step 2: Add `fetch` to `settingsSchema`**

Mirror the `search` shape:

```ts
  fetch: z
    .object({
      provider: z.string().optional(),
      providers: fetchProviderConfigsSchema,
    })
    .optional(),
```

- [ ] **Step 3: Add `fetch?` to `updateSystemSettings`**

Extend the input type with:

```ts
  fetch?: {
    provider?: string;
    providers?: { firecrawl?: { apiKey?: string | null; baseUrl?: string | null } };
  };
```

And, when building the payload, add a `fetch` branch that reuses the same per-provider cleanup as `search` (`.filter(([, v]) => v !== "")` to drop blank string fields so they mean "keep stored", while preserving `null` which means "clear"). Match the exact transform the `search` branch uses.

- [ ] **Step 4: Verify types**

Run (CWD `apps/admin`): `npx tsc --noEmit`
Expected: no new errors from `api.ts` (errors may remain in section/test files added in later tasks — that's fine).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/shared/api.ts
git commit -m "feat: add fetch provider to settings schema and update payload"
```

---

## Task 10: Admin `fetch-section.tsx` + mount + tests (TDD)

**Files:**
- Create: `apps/admin/src/features/settings/sections/fetch-section.tsx`
- Create: `apps/admin/src/features/settings/sections/fetch-section.test.tsx`
- Modify: `apps/admin/src/features/settings/settings-nav.tsx`
- Modify: `apps/admin/src/features/settings/page.tsx`

Reference: `apps/admin/src/features/settings/sections/web-search-section.tsx` and its test.

- [ ] **Step 1: Write the failing section test**

Create `fetch-section.test.tsx`, mirroring `web-search-section.test.tsx`. Mock `../queries` (or the correct relative path) so `useSystemSettingsQuery` returns a firecrawl-active settings object and capture the `useUpdateSystemSettingsMutation` payload:

```tsx
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { FetchSection } from "./fetch-section";

const mutate = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({
    data: {
      fetch: {
        provider: "firecrawl",
        providers: { firecrawl: { apiKeyConfigured: true, baseUrl: "https://api.firecrawl.dev" } },
      },
    },
    isLoading: false,
  }),
  useUpdateSystemSettingsMutation: () => ({ mutate, isPending: false }),
}));

describe("FetchSection", () => {
  it("saves a fetch block with provider + firecrawl config", async () => {
    render(<FetchSection />);
    fireEvent.click(screen.getByRole("button", { name: /save/i }));
    await waitFor(() => expect(mutate).toHaveBeenCalled());
    const payload = mutate.mock.calls.at(-1)?.[0];
    expect(payload).toMatchObject({
      fetch: { provider: "firecrawl", providers: { firecrawl: {} } },
    });
  });
});
```

(Match the exact query hook names, mutation call shape, and Save-button accessibility name to what `web-search-section.test.tsx` uses; adapt the assertion to the real payload keys.)

- [ ] **Step 2: Run — expect fail**

Run (CWD `apps/admin`): `npx vitest run src/features/settings/sections/fetch-section.test.tsx`
Expected: FAIL (module `./fetch-section` not found).

- [ ] **Step 3: Implement `fetch-section.tsx`**

Compose shadcn primitives (Card/CardHeader/CardTitle/CardDescription/CardContent, Select, Input, Switch, Button), mirroring `web-search-section.tsx`:
- Provider `Select`: options `none` | `firecrawl`.
- When `firecrawl` selected: a `Firecrawl API Key` password `Input` (blank = keep stored; show a clear-key `Switch` when `apiKeyConfigured` is true, which sends `apiKey: null`) and a `Firecrawl Base URL` `Input` with `placeholder="https://api.firecrawl.dev"` (blank = revert to default → send `baseUrl: null` or omit, matching the search-section convention for URL-or-clear).
- Hydrate local state from `useSystemSettingsQuery().data?.fetch` using the same `hydratedFrom` ref guard the search section uses.
- `save()` calls the update mutation with `{ fetch: { provider, providers: { firecrawl: { apiKey, baseUrl } } } }`, reusing the same `keyValue` / `urlOrClear` helpers as the search section (import or duplicate them consistently with how the codebase shares them).
- Export as a named export `FetchSection`.

- [ ] **Step 4: Run the section test — expect pass**

Run: `npx vitest run src/features/settings/sections/fetch-section.test.tsx`
Expected: PASS.

- [ ] **Step 5: Mount in nav + page**

In `settings-nav.tsx`: add `"fetch"` to the `SettingsSectionId` union and an entry to `ITEMS` with label `"Web Fetch"` (place it next to the `"search"` / Web Search item).
In `page.tsx`: import `FetchSection` and render `<FetchSection />` when `section === "fetch"`.

- [ ] **Step 6: Verify types + section tests**

Run: `npx tsc --noEmit`
Expected: clean (or only pre-existing unrelated warnings).
Run: `npx vitest run src/features/settings/sections`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add apps/admin/src/features/settings/sections/fetch-section.tsx apps/admin/src/features/settings/sections/fetch-section.test.tsx apps/admin/src/features/settings/settings-nav.tsx apps/admin/src/features/settings/page.tsx
git commit -m "feat: add Web Fetch settings section (Firecrawl) with test"
```

---

## Task 11: `api.test.tsx` — updateSystemSettings fetch payload

**Files:**
- Modify: `apps/admin/src/features/settings/api.test.tsx`

- [ ] **Step 1: Add a fetch-block payload test**

Following the existing `updateSystemSettings` test pattern (`vi.spyOn(globalThis, "fetch")`, `okResponse()`), add a case asserting that calling `updateSystemSettings({ fetch: { provider: "firecrawl", providers: { firecrawl: { apiKey: "sk-1", baseUrl: "https://api.firecrawl.dev" } } } })` sends the expected JSON body (the built payload preserves `null` for clears and drops blank strings, matching the `search` branch behavior).

```tsx
it("sends a fetch block", async () => {
  const spy = vi.spyOn(globalThis, "fetch").mockResolvedValue(okResponse());
  await updateSystemSettings({
    fetch: {
      provider: "firecrawl",
      providers: { firecrawl: { apiKey: "sk-1", baseUrl: "https://api.firecrawl.dev" } },
    },
  });
  const body = JSON.parse((spy.mock.calls.at(-1)?.[1] as RequestInit).body as string);
  expect(body).toMatchObject({
    fetch: {
      provider: "firecrawl",
      providers: { firecrawl: { apiKey: "sk-1", baseUrl: "https://api.firecrawl.dev" } },
    },
  });
});
```

(Adapt `okResponse`, the import of `updateSystemSettings`, and the payload assertion to the real helpers/shape in the file.)

- [ ] **Step 2: Run**

Run (CWD `apps/admin`): `npx vitest run src/features/settings/api.test.tsx`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/settings/api.test.tsx
git commit -m "test: cover updateSystemSettings fetch block payload"
```

---

## Task 12: Full verification

- [ ] **Step 1: Backend**

Run (repo root):
- `cargo test -p knowledge-server --lib` — expected: all pass.
- `cargo check -p knowledge-server` — expected: clean.
- `cargo check --tests` — expected: integration crates compile after the `allow_private_fetch` removal.

- [ ] **Step 2: Frontend**

Run (CWD `apps/admin`):
- `npx vitest run` — expected: all pass.
- `npx tsc --noEmit` — expected: clean.

- [ ] **Step 3: Manual end-to-end (Docker)**

- `docker compose build backend admin && docker compose up -d`
- Fresh DB: open a canvas, add a URL node, run it → returns "web page fetch is not configured; set a fetch provider in Settings".
- Settings → Web Fetch → select `firecrawl`, set Base URL (hosted default or `http://host:3002`) + API key → Save.
- URL node scrapes `https://baidu.com` → returns real title + markdown (the JS-rendered content, not the 227-byte stub).
- Redaction check: reload Settings → the API key field shows configured (not the raw key); base URL persists.

- [ ] **Step 4: Finish the branch**

Use superpowers:finishing-a-development-branch.

---

## File summary

**Backend:**
- New: `migrations/0017_fetch_provider_settings.sql`, `src/web_fetch/mod.rs`, `src/web_fetch/config.rs`, `src/web_fetch/firecrawl.rs`.
- Modified: `src/lib.rs`, `src/config.rs`, `src/app/state.rs`, `src/canvas/service.rs`, `src/canvas/routes.rs`, `src/settings/routes.rs`.
- Modified tests/config: `tests/rust-integration/tests/support/mod.rs`, `tests/rust-integration/tests/docker_stack_smoke.rs`, `tests/rust-integration/tests/provider_connections_race.rs`, `docker-compose.yml`.

**Frontend:**
- New: `src/features/settings/sections/fetch-section.tsx`, `src/features/settings/sections/fetch-section.test.tsx`.
- Modified: `src/features/shared/api.ts`, `src/features/settings/page.tsx`, `src/features/settings/settings-nav.tsx`, `src/features/settings/api.test.tsx`.
