# Phase 4c — Web Search Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port upstream's pluggable web-search layer (`upstream_llm_wiki/src/lib/web-search.ts`) to the server so callers — primarily Phase 4d's deep-research executor, but also the admin UI "Test" button — can issue a query against one of four external providers (Tavily, SerpApi, SearXNG, Ollama) and receive a normalized `[{title, url, snippet, source}]` list. Configuration lives in `system_settings`; the active provider, its API key, and a few provider-specific fields are managed from the Settings page.

**Architecture:** Three layers. (1) A new `crates/knowledge-server/src/web_search/` module containing `provider.rs` (4 provider implementations behind a single `web_search(config, query, max_results)` function, ported verbatim from upstream), `config.rs` (load/validate config from `system_settings`), and `mod.rs` (re-exports + the `WebSearchResult` type). (2) A migration (`0010_web_search_settings.sql`) extends `system_settings` with 6 columns; `settings/routes.rs` GET/PATCH expose them. (3) A new POST `/api/web-search` route lets authenticated callers run a search; the admin Settings page gains a section for the config + a "Test Search" button. The deep-research executor (Phase 4d) will call the internal `web_search` function directly, not the HTTP route.

**Tech Stack:** Rust (axum 0.8, sqlx, reqwest, serde, serde_json, url). React 19 + TanStack Query + Zod, vitest. Playwright for the e2e settings→test flow (uses SearXNG against a localhost mock instance).

**Upstream references (memory rule: port, don't invent):**
- `upstream_llm_wiki/src/lib/web-search.ts` — provider implementations, normalization, error messages. Port verbatim. Drop the `getHttpFetch()` indirection (it's a Tauri CORS shim — we use plain `reqwest`).
- `upstream_llm_wiki/src/lib/web-search.test.ts` — provider invocation + normalization expectations. Mirror semantically (port to `#[tokio::test]` with `wiremock` or a hand-rolled local HTTP mock).
- `upstream_llm_wiki/src/stores/wiki-store.ts:41-95` — `SearchProvider`, `SerpApiEngine`, `SearXngCategory`, `SearchProviderOverride`, `SearchProviderConfigs`, `SearchApiConfig` type definitions. Port the surface area that maps to backend storage; drop `providerConfigs` (UI-only multi-provider overlay) and `anyTxt`/`deepResearchSource` (Phase 4d).

**Documented divergences from upstream:**
1. **No `providerConfigs` overlay.** Upstream stores per-provider configs in one blob so the UI can remember settings across provider switches. We persist only the active provider's fields. Switching providers and back loses the inactive provider's settings — acceptable for a server admin who only configures one provider at a time.
2. **No Tauri HTTP plugin.** `reqwest::Client` handles all outbound HTTP. CORS isn't a concern because the server is the one making the calls, not the browser.
3. **No `anyTxt` / `deepResearchSource` fields.** Those belong to Phase 4d (deep research, which combines web + local source search). When 4d lands, it'll add them.
4. **`/api/web-search` is session OR Bearer authenticated (Phase 4a principal layer).** It does NOT carry a project ID — web search has no project scope. Bearer tokens that are project-scoped still work; the call doesn't touch any project state.
5. **Provider base URLs are configurable via `config.rs`** (defaults: `https://api.tavily.com`, `https://serpapi.com`, instance-supplied for SearXNG, `https://ollama.com`). Reason: integration tests need to point Tavily/SerpApi/Ollama at a localhost mock. Upstream hardcodes them; the divergence is small and test-driven.
6. **Provider name `"none"` is encoded as `Option<WebSearchProvider>::None`** in the Rust types — there is no `WebSearchProvider::None` variant. The DB column stores `'none'` (sentinel), the Rust load step converts it to `None`. Reason: idiomatic Rust, and it makes `Option::Some(provider)` the precondition for `web_search()`, so the type system prevents calling it on an unconfigured server.
7. **API key is stored in plaintext in Postgres** (same as the existing `provider_api_key` column). Reason: this is a server, not a multi-user shared secret store — the operator owns the DB. If we ever support per-user web search keys, we'd encrypt at rest then. Not in this phase.

**Indentation conventions:** `crates/**` source = 2-space EXCEPT `crates/knowledge-server/src/projects/routes.rs` and `crates/knowledge-server/src/auth/routes.rs` = mixed (follow the file). `tests/rust-integration/**` = 4-space. All TS/TSX = 2-space.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `crates/knowledge-server/migrations/0010_web_search_settings.sql` | Create | Add 6 columns to `system_settings` |
| `crates/knowledge-server/src/web_search/mod.rs` | Create | `pub mod provider; pub mod config;` + `WebSearchResult` + `WebSearchProvider` enum |
| `crates/knowledge-server/src/web_search/config.rs` | Create | `WebSearchConfig` + `load_web_search_config(state) -> Result<Option<WebSearchConfig>, ApiError>` |
| `crates/knowledge-server/src/web_search/provider.rs` | Create | `web_search(config, query, max_results)` + 4 provider fns + normalizers; unit tests |
| `crates/knowledge-server/src/lib.rs` | Modify | Register `pub mod web_search;` |
| `crates/knowledge-server/src/settings/routes.rs` | Modify | GET/PATCH new columns |
| `crates/knowledge-server/src/web_search/routes.rs` | Create | `POST /api/web-search` handler |
| `crates/knowledge-server/src/http/router.rs` | Modify | Merge `web_search::routes::router()` |
| `tests/rust-integration/tests/web_search_api.rs` | Create | end-to-end: PATCH settings (SearXNG mock) → POST /api/web-search → verify result |
| `apps/admin/src/features/shared/api.ts` | Modify | extend `SystemSettings` type + `runWebSearch` + zod schemas |
| `apps/admin/src/features/settings/page.tsx` | Modify | Web Search section + Test button |
| `apps/admin/src/features/settings/page.test.tsx` | Modify or Create | vitest for the new fields and Test action |
| `apps/admin/src/lib/route-meta.ts` | (no change) | Settings page already wired |
| `tests/web/mock-openai.mjs` | Modify | Add `/search` SearXNG-compatible endpoint |
| `tests/web/tests/web-search.spec.ts` | Create | e2e: configure SearXNG to mock URL → click Test → expect result visible |

---

### Task 1: Database migration

**Files:**
- Create: `crates/knowledge-server/migrations/0010_web_search_settings.sql`

- [ ] **Step 1: Create the migration**

```sql
ALTER TABLE system_settings ADD COLUMN search_provider TEXT NOT NULL DEFAULT 'none';
ALTER TABLE system_settings ADD COLUMN search_api_key TEXT;
ALTER TABLE system_settings ADD COLUMN serpapi_engine TEXT DEFAULT 'google';
ALTER TABLE system_settings ADD COLUMN searxng_url TEXT;
ALTER TABLE system_settings ADD COLUMN searxng_categories JSONB NOT NULL DEFAULT '["general"]'::jsonb;
ALTER TABLE system_settings ADD COLUMN ollama_search_url TEXT;
```

Notes:
- `search_provider` defaults to `'none'` (matches the upstream `"none"` sentinel; the Rust load step maps it to `Option::None`).
- `search_api_key` and `searxng_url` and `ollama_search_url` are nullable — only the active provider needs a value, and a freshly migrated DB has none configured.
- `searxng_categories` is a JSONB array (matches upstream `SearXngCategory[]`); default `["general"]` is upstream's default.
- `serpapi_engine` defaults to `google` (upstream's default).

- [ ] **Step 2: Verify the migration applies cleanly**

Postgres must be up. Run an integration test that boots a fresh DB:

```bash
cargo test -p rust-integration --test auth_api login_caches_session_in_redis -- --test-threads=1
```

Expected: PASS. The migration runs as part of `bootstrap_state`; failure would be a hard panic during bootstrap.

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/migrations/0010_web_search_settings.sql
git commit -m "feat: add web search columns to system_settings"
```

---

### Task 2: Config types + loader

**Files:**
- Create: `crates/knowledge-server/src/web_search/mod.rs`
- Create: `crates/knowledge-server/src/web_search/config.rs`
- Modify: `crates/knowledge-server/src/lib.rs`

- [ ] **Step 1: Register the module**

In `crates/knowledge-server/src/lib.rs`, add (alphabetical position):

```rust
pub mod web_search;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/knowledge-server/src/web_search/config.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_provider_maps_known_values() {
    assert_eq!(parse_provider("tavily"), Some(WebSearchProvider::Tavily));
    assert_eq!(parse_provider("serpapi"), Some(WebSearchProvider::SerpApi));
    assert_eq!(parse_provider("searxng"), Some(WebSearchProvider::SearXng));
    assert_eq!(parse_provider("ollama"), Some(WebSearchProvider::Ollama));
  }

  #[test]
  fn parse_provider_returns_none_for_sentinel_and_unknown() {
    assert!(parse_provider("none").is_none());
    assert!(parse_provider("").is_none());
    assert!(parse_provider("google").is_none());
  }

  #[test]
  fn parse_categories_handles_array_and_string_fallback() {
    use serde_json::json;
    let parsed = parse_categories(&json!(["general", "news"]));
    assert_eq!(parsed, vec!["general".to_string(), "news".to_string()]);

    let fallback = parse_categories(&json!("not an array"));
    assert_eq!(fallback, vec!["general".to_string()]);

    let empty = parse_categories(&json!([]));
    assert_eq!(empty, vec!["general".to_string()]);
  }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p knowledge-server web_search::config`
Expected: FAIL — `cannot find type WebSearchProvider`.

- [ ] **Step 4: Implement `mod.rs` and `config.rs`**

Create `crates/knowledge-server/src/web_search/mod.rs`:

```rust
pub mod config;
pub mod provider;
pub mod routes;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WebSearchResult {
  pub title: String,
  pub url: String,
  pub snippet: String,
  pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSearchProvider {
  Tavily,
  SerpApi,
  SearXng,
  Ollama,
}

impl WebSearchProvider {
  pub fn as_str(&self) -> &'static str {
    match self {
      WebSearchProvider::Tavily => "tavily",
      WebSearchProvider::SerpApi => "serpapi",
      WebSearchProvider::SearXng => "searxng",
      WebSearchProvider::Ollama => "ollama",
    }
  }
}
```

Prepend to `crates/knowledge-server/src/web_search/config.rs`:

```rust
use serde_json::Value;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::web_search::WebSearchProvider;

#[derive(Debug, Clone)]
pub struct WebSearchConfig {
  pub provider: WebSearchProvider,
  pub api_key: Option<String>,
  pub serpapi_engine: String,
  pub searxng_url: Option<String>,
  pub searxng_categories: Vec<String>,
  pub ollama_search_url: String,
  /// Overrideable base URL for tavily — defaults to the public endpoint.
  /// Tests inject a localhost mock via this field.
  pub tavily_base_url: String,
  /// Overrideable base URL for serpapi — defaults to the public endpoint.
  pub serpapi_base_url: String,
}

impl WebSearchConfig {
  pub fn requires_api_key(&self) -> bool {
    matches!(
      self.provider,
      WebSearchProvider::Tavily | WebSearchProvider::SerpApi | WebSearchProvider::Ollama,
    )
  }
}

pub(crate) fn parse_provider(value: &str) -> Option<WebSearchProvider> {
  match value {
    "tavily" => Some(WebSearchProvider::Tavily),
    "serpapi" => Some(WebSearchProvider::SerpApi),
    "searxng" => Some(WebSearchProvider::SearXng),
    "ollama" => Some(WebSearchProvider::Ollama),
    _ => None,
  }
}

pub(crate) fn parse_categories(value: &Value) -> Vec<String> {
  let parsed = value
    .as_array()
    .map(|items| {
      items
        .iter()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect::<Vec<_>>()
    })
    .unwrap_or_default();
  if parsed.is_empty() {
    vec!["general".to_string()]
  } else {
    parsed
  }
}

pub async fn load_web_search_config(state: &AppState) -> Result<Option<WebSearchConfig>, ApiError> {
  let (
    search_provider,
    search_api_key,
    serpapi_engine,
    searxng_url,
    searxng_categories,
    ollama_search_url,
  ) = sqlx::query_as::<
    _,
    (String, Option<String>, Option<String>, Option<String>, Value, Option<String>),
  >(
    "SELECT
       search_provider,
       search_api_key,
       serpapi_engine,
       searxng_url,
       searxng_categories,
       ollama_search_url
     FROM system_settings
     WHERE id = 1",
  )
  .fetch_one(&state.pool)
  .await
  .map_err(ApiError::from)?;

  let Some(provider) = parse_provider(&search_provider) else {
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
    tavily_base_url: "https://api.tavily.com".to_string(),
    serpapi_base_url: "https://serpapi.com".to_string(),
  }))
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p knowledge-server web_search::config`
Expected: 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/lib.rs crates/knowledge-server/src/web_search/mod.rs crates/knowledge-server/src/web_search/config.rs
git commit -m "feat: add WebSearchConfig loader and provider parsing"
```

(`mod.rs` declares `pub mod provider;` and `pub mod routes;` — those files come next. Until then, `cargo build` will fail. Apply Task 2 + Task 3 + Task 4 + Task 5 + Task 6 + Task 7 + Task 8 as a single connected batch; do not pause for `cargo build` between them. The unit tests in this task can still run because they don't reach into `provider`/`routes`.)

Actually, scratch that. To keep the TDD discipline of "each task ends with a green build", stub the two missing modules now:

In `crates/knowledge-server/src/web_search/provider.rs` create a one-line stub:

```rust
// Implemented in Task 3 onward.
```

In `crates/knowledge-server/src/web_search/routes.rs`:

```rust
use axum::Router;

use crate::app::state::AppState;

pub fn router() -> Router<AppState> {
  Router::new()
}
```

This keeps the workspace compiling between tasks.

---

### Task 3: Tavily provider

**Files:**
- Modify: `crates/knowledge-server/src/web_search/provider.rs`

Upstream reference: `upstream_llm_wiki/src/lib/web-search.ts:251-295` (`tavilySearch`). POST to `${tavily_base_url}/search`, JSON body `{api_key, query, max_results, search_depth: "advanced", include_answer: false}`. Response `{results: [{title, url, content}]}`.

- [ ] **Step 1: Write the failing test**

Replace the stub in `provider.rs`:

```rust
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::http::error::ApiError;
use crate::web_search::{WebSearchProvider, WebSearchResult};
use crate::web_search::config::WebSearchConfig;

const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

pub async fn web_search(
  config: &WebSearchConfig,
  query: &str,
  max_results: usize,
) -> Result<Vec<WebSearchResult>, ApiError> {
  if config.requires_api_key() && config.api_key.as_deref().unwrap_or("").is_empty() {
    return Err(ApiError::bad_request(format!(
      "web search provider \"{}\" requires an api key; configure one in Settings",
      config.provider.as_str()
    )));
  }
  match config.provider {
    WebSearchProvider::Tavily => tavily_search(config, query, max_results).await,
    WebSearchProvider::SerpApi => Err(ApiError::internal("serpapi not yet implemented")),
    WebSearchProvider::SearXng => Err(ApiError::internal("searxng not yet implemented")),
    WebSearchProvider::Ollama => Err(ApiError::internal("ollama not yet implemented")),
  }
}

fn build_client() -> Result<Client, ApiError> {
  Client::builder()
    .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
    .build()
    .map_err(|error| ApiError::internal(format!("web search client init failed: {error}")))
}

fn hostname_from_url(url: &str) -> String {
  match url::Url::parse(url) {
    Ok(parsed) => parsed
      .host_str()
      .map(|host| host.trim_start_matches("www.").to_string())
      .unwrap_or_default(),
    Err(_) => String::new(),
  }
}

#[derive(Debug, Deserialize)]
struct TavilyResponse {
  #[serde(default)]
  results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
  #[serde(default)]
  title: Option<String>,
  #[serde(default)]
  url: Option<String>,
  #[serde(default)]
  content: Option<String>,
}

async fn tavily_search(
  config: &WebSearchConfig,
  query: &str,
  max_results: usize,
) -> Result<Vec<WebSearchResult>, ApiError> {
  let api_key = config.api_key.as_deref().unwrap_or("");
  let client = build_client()?;
  let body = serde_json::json!({
    "api_key": api_key,
    "query": query,
    "max_results": max_results,
    "search_depth": "advanced",
    "include_answer": false,
  });
  let url = format!("{}/search", config.tavily_base_url.trim_end_matches('/'));
  let response = client
    .post(&url)
    .header("Content-Type", "application/json")
    .json(&body)
    .send()
    .await
    .map_err(|error| ApiError::bad_request(format!("tavily request failed: {error}")))?;
  if !response.status().is_success() {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    return Err(ApiError::bad_request(format!(
      "tavily search failed ({status}): {text}"
    )));
  }
  let parsed: TavilyResponse = response
    .json()
    .await
    .map_err(|error| ApiError::bad_request(format!("tavily response parse failed: {error}")))?;

  Ok(
    parsed
      .results
      .into_iter()
      .take(max_results)
      .map(|item| {
        let url = item.url.unwrap_or_default();
        WebSearchResult {
          title: item.title.unwrap_or_else(|| "Untitled".to_string()),
          url: url.clone(),
          snippet: item.content.unwrap_or_default(),
          source: hostname_from_url(&url),
        }
      })
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_config(base_url: &str) -> WebSearchConfig {
    WebSearchConfig {
      provider: WebSearchProvider::Tavily,
      api_key: Some("test-key".to_string()),
      serpapi_engine: "google".to_string(),
      searxng_url: None,
      searxng_categories: vec!["general".to_string()],
      ollama_search_url: "https://ollama.com".to_string(),
      tavily_base_url: base_url.to_string(),
      serpapi_base_url: base_url.to_string(),
    }
  }

  async fn spawn_mock_tavily() -> (tokio::task::JoinHandle<()>, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
      if let Ok((mut socket, _)) = listener.accept().await {
        let mut buf = vec![0u8; 4096];
        let _ = socket.read(&mut buf).await;
        let body = serde_json::json!({
          "results": [
            {"title": "Demo", "url": "https://example.com/demo", "content": "snippet text"},
            {"title": "Demo 2", "url": "https://other.example.com/page", "content": "second"}
          ]
        })
        .to_string();
        let payload = format!(
          "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
          body.len(),
          body
        );
        let _ = socket.write_all(payload.as_bytes()).await;
        let _ = socket.shutdown().await;
      }
    });
    (handle, format!("http://127.0.0.1:{port}"))
  }

  #[tokio::test]
  async fn tavily_normalizes_results_and_extracts_hostname() {
    let (handle, base) = spawn_mock_tavily().await;
    let config = test_config(&base);
    let results = web_search(&config, "query", 10).await.unwrap();
    handle.await.unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Demo");
    assert_eq!(results[0].url, "https://example.com/demo");
    assert_eq!(results[0].snippet, "snippet text");
    assert_eq!(results[0].source, "example.com");
    assert_eq!(results[1].source, "other.example.com");
  }

  #[tokio::test]
  async fn web_search_rejects_when_api_key_missing() {
    let mut config = test_config("http://127.0.0.1:1");
    config.api_key = None;
    let err = web_search(&config, "query", 10).await.unwrap_err();
    assert!(err.to_string().contains("requires an api key"));
  }
}
```

- [ ] **Step 2: Run to verify**

Run: `cargo test -p knowledge-server web_search::provider`
Expected: 2 tests PASS. (The first test uses a one-shot raw TCP HTTP responder so we don't pull `wiremock` as a dev-dep.)

If the raw-socket mock proves brittle on Windows (the `\r\n` line endings might trip something), swap to a one-handler `axum::Router` listening on `127.0.0.1:0` via `axum::serve` — same shape, less protocol risk.

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/src/web_search/provider.rs
git commit -m "feat: add tavily web search provider"
```

---

### Task 4: SerpApi provider

**Files:**
- Modify: `crates/knowledge-server/src/web_search/provider.rs`

Upstream reference: `upstream_llm_wiki/src/lib/web-search.ts:297-381`. GET to `${serpapi_base_url}/search?engine=${engine}&q=${query}&api_key=${key}&num=${max_results}`. Response keys vary by engine — `organic_results`, `news_results`, `images_results`, `video_results`/`videos_results`, `shopping_results`. The normalizer picks the first non-empty array. Each item has `title`, `link`/`url`/`original`/`thumbnail`, `snippet`/`summary`/`description`, optionally `source`/`displayed_link`.

- [ ] **Step 1: Add the test**

Append inside `mod tests` in `provider.rs`:

```rust
  async fn spawn_mock_serpapi() -> (tokio::task::JoinHandle<()>, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
      if let Ok((mut socket, _)) = listener.accept().await {
        let mut buf = vec![0u8; 4096];
        let _ = socket.read(&mut buf).await;
        let body = serde_json::json!({
          "organic_results": [
            {"title": "Org", "link": "https://example.com/org", "snippet": "from organic"}
          ]
        })
        .to_string();
        let payload = format!(
          "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
          body.len(),
          body
        );
        let _ = socket.write_all(payload.as_bytes()).await;
        let _ = socket.shutdown().await;
      }
    });
    (handle, format!("http://127.0.0.1:{port}"))
  }

  #[tokio::test]
  async fn serpapi_picks_organic_results_and_normalizes() {
    let (handle, base) = spawn_mock_serpapi().await;
    let mut config = test_config(&base);
    config.provider = WebSearchProvider::SerpApi;
    config.serpapi_base_url = base.clone();
    let results = web_search(&config, "query", 5).await.unwrap();
    handle.await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Org");
    assert_eq!(results[0].url, "https://example.com/org");
    assert_eq!(results[0].snippet, "from organic");
    assert_eq!(results[0].source, "example.com");
  }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p knowledge-server web_search::provider::tests::serpapi`
Expected: FAIL — `serpapi not yet implemented`.

- [ ] **Step 3: Implement SerpApi**

Replace `WebSearchProvider::SerpApi => Err(ApiError::internal("serpapi not yet implemented"))` with `WebSearchProvider::SerpApi => serpapi_search(config, query, max_results).await`.

Add (after `tavily_search`):

```rust
#[derive(Debug, Deserialize)]
struct SerpApiResponse {
  #[serde(default)]
  organic_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  news_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  images_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  video_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  videos_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  shopping_results: Option<Vec<SerpApiResult>>,
  #[serde(default)]
  error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerpApiResult {
  #[serde(default)]
  title: Option<String>,
  #[serde(default)]
  link: Option<String>,
  #[serde(default)]
  url: Option<String>,
  #[serde(default)]
  original: Option<String>,
  #[serde(default)]
  thumbnail: Option<String>,
  #[serde(default)]
  snippet: Option<String>,
  #[serde(default)]
  summary: Option<String>,
  #[serde(default)]
  description: Option<String>,
  #[serde(default)]
  source: Option<String>,
  #[serde(default)]
  displayed_link: Option<String>,
}

async fn serpapi_search(
  config: &WebSearchConfig,
  query: &str,
  max_results: usize,
) -> Result<Vec<WebSearchResult>, ApiError> {
  let api_key = config.api_key.as_deref().unwrap_or("");
  let client = build_client()?;
  let url = format!("{}/search", config.serpapi_base_url.trim_end_matches('/'));
  let response = client
    .get(&url)
    .query(&[
      ("engine", config.serpapi_engine.as_str()),
      ("q", query),
      ("api_key", api_key),
      ("num", &max_results.to_string()),
    ])
    .header("Accept", "application/json")
    .send()
    .await
    .map_err(|error| ApiError::bad_request(format!("serpapi request failed: {error}")))?;
  if !response.status().is_success() {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    return Err(ApiError::bad_request(format!(
      "serpapi search failed ({status}): {text}"
    )));
  }
  let parsed: SerpApiResponse = response
    .json()
    .await
    .map_err(|error| ApiError::bad_request(format!("serpapi response parse failed: {error}")))?;

  if let Some(message) = parsed.error.as_deref().filter(|value| !value.trim().is_empty()) {
    return Err(ApiError::bad_request(format!("serpapi search failed: {message}")));
  }

  let raw = parsed
    .organic_results
    .or(parsed.news_results)
    .or(parsed.images_results)
    .or(parsed.video_results)
    .or(parsed.videos_results)
    .or(parsed.shopping_results)
    .unwrap_or_default();

  Ok(
    raw
      .into_iter()
      .take(max_results)
      .map(|item| {
        let url = item
          .link
          .or(item.url)
          .or(item.original)
          .or(item.thumbnail)
          .unwrap_or_default();
        let source = hostname_from_url(&url);
        let source = if source.is_empty() {
          item.source.or(item.displayed_link).unwrap_or_default()
        } else {
          source
        };
        WebSearchResult {
          title: item.title.unwrap_or_else(|| "Untitled".to_string()),
          url,
          snippet: item.snippet.or(item.summary).or(item.description).unwrap_or_default(),
          source,
        }
      })
      .collect(),
  )
}
```

- [ ] **Step 2: Run all provider tests**

Run: `cargo test -p knowledge-server web_search::provider`
Expected: 3 tests PASS (2 from Task 3 + new SerpApi).

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/src/web_search/provider.rs
git commit -m "feat: add serpapi web search provider"
```

---

### Task 5: SearXNG provider

**Files:**
- Modify: `crates/knowledge-server/src/web_search/provider.rs`

Upstream reference: `upstream_llm_wiki/src/lib/web-search.ts:164-241`. GET to `${searxng_url}/search?q=${query}&format=json&categories=${comma_joined}`. Response `{results: [{title, url, content, engine?, category?}]}`. Source = hostname of url, or `engine`, or `category`.

- [ ] **Step 1: Add the test**

Append inside `mod tests`:

```rust
  async fn spawn_mock_searxng() -> (tokio::task::JoinHandle<()>, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
      if let Ok((mut socket, _)) = listener.accept().await {
        let mut buf = vec![0u8; 4096];
        let _ = socket.read(&mut buf).await;
        let body = serde_json::json!({
          "results": [
            {"title": "Sx", "url": "https://example.com/sx", "content": "from searxng", "engine": "duckduckgo"},
            {"title": "Sx2", "url": "", "content": "no url", "engine": "google"}
          ]
        })
        .to_string();
        let payload = format!(
          "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
          body.len(),
          body
        );
        let _ = socket.write_all(payload.as_bytes()).await;
        let _ = socket.shutdown().await;
      }
    });
    (handle, format!("http://127.0.0.1:{port}"))
  }

  #[tokio::test]
  async fn searxng_normalizes_results_and_drops_url_less_entries() {
    let (handle, base) = spawn_mock_searxng().await;
    let mut config = test_config(&base);
    config.provider = WebSearchProvider::SearXng;
    config.api_key = None;
    config.searxng_url = Some(base.clone());
    let results = web_search(&config, "query", 5).await.unwrap();
    handle.await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Sx");
    assert_eq!(results[0].url, "https://example.com/sx");
    assert_eq!(results[0].source, "example.com");
  }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p knowledge-server web_search::provider::tests::searxng`
Expected: FAIL — `searxng not yet implemented`.

- [ ] **Step 3: Implement SearXNG**

Replace `WebSearchProvider::SearXng => Err(...)` with `WebSearchProvider::SearXng => searxng_search(config, query, max_results).await`.

Add:

```rust
#[derive(Debug, Deserialize)]
struct SearXngResponse {
  #[serde(default)]
  results: Vec<SearXngResult>,
}

#[derive(Debug, Deserialize)]
struct SearXngResult {
  #[serde(default)]
  title: Option<String>,
  #[serde(default)]
  url: Option<String>,
  #[serde(default)]
  content: Option<String>,
  #[serde(default)]
  engine: Option<String>,
  #[serde(default)]
  category: Option<String>,
}

fn searxng_endpoint(instance_url: &str) -> Result<url::Url, ApiError> {
  let trimmed = instance_url.trim();
  let with_protocol = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
    trimmed.to_string()
  } else {
    format!("https://{trimmed}")
  };
  let mut parsed = url::Url::parse(&with_protocol)
    .map_err(|_| ApiError::bad_request("Invalid SearXNG instance URL"))?;
  let path = parsed.path().trim_end_matches('/').to_string();
  let new_path = if path == "/search" || path.ends_with("/search") {
    if path.is_empty() { "/search".to_string() } else { path }
  } else {
    format!("{path}/search")
  };
  parsed.set_path(&new_path);
  parsed.set_query(None);
  parsed.set_fragment(None);
  Ok(parsed)
}

async fn searxng_search(
  config: &WebSearchConfig,
  query: &str,
  max_results: usize,
) -> Result<Vec<WebSearchResult>, ApiError> {
  let instance = config
    .searxng_url
    .as_deref()
    .ok_or_else(|| ApiError::bad_request("SearXNG instance URL not configured"))?;
  let endpoint = searxng_endpoint(instance)?;
  let client = build_client()?;
  let categories = if config.searxng_categories.is_empty() {
    "general".to_string()
  } else {
    config.searxng_categories.join(",")
  };
  let response = client
    .get(endpoint)
    .query(&[("q", query), ("format", "json"), ("categories", categories.as_str())])
    .header("Accept", "application/json")
    .send()
    .await
    .map_err(|error| ApiError::bad_request(format!("searxng request failed: {error}")))?;
  if !response.status().is_success() {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    return Err(ApiError::bad_request(format!(
      "SearXNG search failed ({status}): {text}"
    )));
  }
  let parsed: SearXngResponse = response
    .json()
    .await
    .map_err(|error| ApiError::bad_request(format!("searxng response parse failed: {error}")))?;

  Ok(
    parsed
      .results
      .into_iter()
      .take(max_results)
      .map(|item| {
        let url = item.url.unwrap_or_default();
        let source = hostname_from_url(&url);
        let source = if source.is_empty() {
          item.engine.or(item.category).unwrap_or_default()
        } else {
          source
        };
        WebSearchResult {
          title: item.title.unwrap_or_else(|| "Untitled".to_string()),
          url: url.clone(),
          snippet: item.content.unwrap_or_default(),
          source,
        }
      })
      .filter(|result| !result.url.is_empty())
      .collect(),
  )
}
```

Add `url` to `crates/knowledge-server/Cargo.toml` if it isn't already there. Check first: `grep "^url" crates/knowledge-server/Cargo.toml`. If missing, add `url = "2"` to `[dependencies]`.

- [ ] **Step 4: Run all provider tests**

Run: `cargo test -p knowledge-server web_search::provider`
Expected: 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/web_search/provider.rs crates/knowledge-server/Cargo.toml
git commit -m "feat: add searxng web search provider"
```

---

### Task 6: Ollama provider

**Files:**
- Modify: `crates/knowledge-server/src/web_search/provider.rs`

Upstream reference: `upstream_llm_wiki/src/lib/web-search.ts:383-455`. POST `${ollama_search_url}/api/web_search` with `{query, max_results}`, `Authorization: Bearer ${apiKey}`. Response `{results: [{title, url, content}], error?}`.

- [ ] **Step 1: Add the test**

Append inside `mod tests`:

```rust
  async fn spawn_mock_ollama() -> (tokio::task::JoinHandle<()>, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
      if let Ok((mut socket, _)) = listener.accept().await {
        let mut buf = vec![0u8; 4096];
        let _ = socket.read(&mut buf).await;
        let body = serde_json::json!({
          "results": [
            {"title": "O", "url": "https://example.com/o", "content": "from ollama"}
          ]
        })
        .to_string();
        let payload = format!(
          "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
          body.len(),
          body
        );
        let _ = socket.write_all(payload.as_bytes()).await;
        let _ = socket.shutdown().await;
      }
    });
    (handle, format!("http://127.0.0.1:{port}"))
  }

  #[tokio::test]
  async fn ollama_normalizes_results() {
    let (handle, base) = spawn_mock_ollama().await;
    let mut config = test_config(&base);
    config.provider = WebSearchProvider::Ollama;
    config.ollama_search_url = base.clone();
    let results = web_search(&config, "query", 5).await.unwrap();
    handle.await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "O");
    assert_eq!(results[0].url, "https://example.com/o");
    assert_eq!(results[0].source, "example.com");
  }
```

- [ ] **Step 2: Implement Ollama**

Replace `WebSearchProvider::Ollama => Err(...)` with `WebSearchProvider::Ollama => ollama_search(config, query, max_results).await`.

Add:

```rust
#[derive(Debug, Deserialize)]
struct OllamaResponse {
  #[serde(default)]
  results: Vec<OllamaResult>,
  #[serde(default)]
  error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaResult {
  #[serde(default)]
  title: Option<String>,
  #[serde(default)]
  url: Option<String>,
  #[serde(default)]
  content: Option<String>,
}

async fn ollama_search(
  config: &WebSearchConfig,
  query: &str,
  max_results: usize,
) -> Result<Vec<WebSearchResult>, ApiError> {
  let api_key = config.api_key.as_deref().unwrap_or("");
  let client = build_client()?;
  let url = format!("{}/api/web_search", config.ollama_search_url.trim_end_matches('/'));
  let response = client
    .post(&url)
    .header("Content-Type", "application/json")
    .header("Authorization", format!("Bearer {api_key}"))
    .json(&serde_json::json!({"query": query, "max_results": max_results}))
    .send()
    .await
    .map_err(|error| ApiError::bad_request(format!("ollama request failed: {error}")))?;
  if !response.status().is_success() {
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
      return Err(ApiError::bad_request(
        "Ollama Web Search API authentication failed. Check your Ollama API key.",
      ));
    }
    let text = response.text().await.unwrap_or_default();
    return Err(ApiError::bad_request(format!(
      "Ollama web search failed ({status}): {text}"
    )));
  }
  let parsed: OllamaResponse = response
    .json()
    .await
    .map_err(|error| ApiError::bad_request(format!("ollama response parse failed: {error}")))?;

  if let Some(message) = parsed.error.as_deref().filter(|value| !value.trim().is_empty()) {
    return Err(ApiError::bad_request(format!("Ollama web search error: {message}")));
  }

  Ok(
    parsed
      .results
      .into_iter()
      .take(max_results)
      .map(|item| {
        let url = item.url.unwrap_or_default();
        WebSearchResult {
          title: item.title.unwrap_or_else(|| "Untitled".to_string()),
          url: url.clone(),
          snippet: item.content.unwrap_or_default(),
          source: hostname_from_url(&url),
        }
      })
      .collect(),
  )
}
```

- [ ] **Step 3: Run all provider tests**

Run: `cargo test -p knowledge-server web_search::provider`
Expected: 5 tests PASS.

- [ ] **Step 4: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/web_search/provider.rs
git commit -m "feat: add ollama web search provider"
```

---

### Task 7: Settings GET/PATCH for web search columns

**Files:**
- Modify: `crates/knowledge-server/src/settings/routes.rs`

The existing handler tuples are already long. The cleanest patch: extend `UpdateSettingsRequest` with the new fields, extend the SELECT/UPDATE SQL accordingly, mirror the response JSON.

- [ ] **Step 1: Extend the request struct**

In `crates/knowledge-server/src/settings/routes.rs`, add after the existing fields in `UpdateSettingsRequest`:

```rust
  #[serde(default)]
  pub search_provider: Option<String>,
  #[serde(default)]
  pub search_api_key: Option<String>,
  #[serde(default)]
  pub serpapi_engine: Option<String>,
  #[serde(default)]
  pub searxng_url: Option<String>,
  #[serde(default)]
  pub searxng_categories: Option<Vec<String>>,
  #[serde(default)]
  pub ollama_search_url: Option<String>,
```

- [ ] **Step 2: Extend `get_settings`**

Add 6 columns to the SELECT and 6 fields to the JSON response. The expanded tuple is unwieldy; pull it into a named struct via `sqlx::FromRow` to keep readable. But the existing handler uses positional tuples — match that style for diff minimization. Final SELECT:

```rust
  let (
    provider_mode,
    language,
    default_query_limit,
    provider_base_url,
    provider_api_key,
    provider_model,
    provider_embedding_model,
    provider_timeout_seconds,
    search_provider,
    search_api_key,
    serpapi_engine,
    searxng_url,
    searxng_categories,
    ollama_search_url,
  ) = sqlx::query_as::<_, (
    String,
    String,
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    serde_json::Value,
    Option<String>,
  )>(
    "SELECT
      provider_mode,
      language,
      default_query_limit,
      provider_base_url,
      provider_api_key,
      provider_model,
      provider_embedding_model,
      provider_timeout_seconds,
      search_provider,
      search_api_key,
      serpapi_engine,
      searxng_url,
      searxng_categories,
      ollama_search_url
     FROM system_settings
     WHERE id = 1",
  )
```

Extend the JSON response with:

```rust
    "searchProvider": search_provider,
    "searchApiKeyConfigured": search_api_key.as_deref().is_some_and(|value| !value.is_empty()),
    "serpapiEngine": serpapi_engine,
    "searxngUrl": searxng_url,
    "searxngCategories": searxng_categories,
    "ollamaSearchUrl": ollama_search_url,
```

- [ ] **Step 3: Extend `update_settings`**

Add 6 fields to the UPDATE SQL. Use the existing `COALESCE(NULLIF(...), search_api_key)` trick so empty-string-or-omitted keeps the previous value:

```rust
    "UPDATE system_settings
     SET provider_mode = $1,
         language = $2,
         default_query_limit = $3,
         provider_base_url = $4,
         provider_api_key = COALESCE(NULLIF($5, ''), provider_api_key),
         provider_model = $6,
         provider_embedding_model = $7,
         provider_timeout_seconds = $8,
         search_provider = COALESCE($9, search_provider),
         search_api_key = COALESCE(NULLIF($10, ''), search_api_key),
         serpapi_engine = COALESCE($11, serpapi_engine),
         searxng_url = $12,
         searxng_categories = COALESCE($13, searxng_categories),
         ollama_search_url = $14
     WHERE id = 1",
```

Bindings (after the existing 8):

```rust
  .bind(payload.search_provider.as_deref())
  .bind(payload.search_api_key.as_deref())
  .bind(payload.serpapi_engine.as_deref())
  .bind(payload.searxng_url.as_deref())
  .bind(
    payload
      .searxng_categories
      .as_ref()
      .map(|values| serde_json::to_value(values).unwrap_or(serde_json::Value::Null)),
  )
  .bind(payload.ollama_search_url.as_deref())
```

Extend the response JSON with the same 6 keys, mirroring `get_settings`.

- [ ] **Step 4: Build and run existing tests**

Run: `cargo test -p knowledge-server --lib`
Expected: PASS.

Run: `cargo test -p rust-integration -- --test-threads=1 settings`
Expected: existing settings tests PASS (if any). If none exist, just confirm the crate compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/settings/routes.rs
git commit -m "feat: extend settings GET/PATCH with web search fields"
```

---

### Task 8: `/api/web-search` route + integration test

**Files:**
- Modify: `crates/knowledge-server/src/web_search/routes.rs`
- Modify: `crates/knowledge-server/src/http/router.rs`
- Create: `tests/rust-integration/tests/web_search_api.rs`

- [ ] **Step 1: Write the integration test**

Create `tests/rust-integration/tests/web_search_api.rs`:

```rust
mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::{Value, json};
use support::TestEnvironment;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tower::util::ServiceExt;

#[tokio::test]
async fn web_search_against_mock_searxng_returns_results() {
    let _env = TestEnvironment::start("web-search-flow").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;

    let (searxng_handle, searxng_base) = spawn_mock_searxng().await;

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

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/web-search")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
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
    assert_eq!(results[0]["title"].as_str(), Some("Sx"));
    assert_eq!(results[0]["url"].as_str(), Some("https://example.com/sx"));

    searxng_handle.await.unwrap();
}

async fn spawn_mock_searxng() -> (tokio::task::JoinHandle<()>, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let body = json!({
              "results": [
                {"title": "Sx", "url": "https://example.com/sx", "content": "from searxng", "engine": "duckduckgo"}
              ]
            })
            .to_string();
            let payload = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(payload.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    (handle, format!("http://127.0.0.1:{port}"))
}

async fn login_and_csrf(state: knowledge_server::app::state::AppState) -> (String, String) {
    let login = build_app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                      "username": "admin",
                      "password": "secret-password"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let body = read_json(login.into_body()).await;
    let csrf = body.get("csrfToken").and_then(Value::as_str).unwrap().to_string();
    (cookie, csrf)
}

async fn read_json(body: Body) -> Value {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rust-integration --test web_search_api -- --test-threads=1`
Expected: FAIL — `/api/web-search` returns 404.

- [ ] **Step 3: Implement the route**

Replace `crates/knowledge-server/src/web_search/routes.rs` with:

```rust
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::state::AppState;
use crate::auth::principal::resolve_principal;
use crate::http::error::ApiError;
use crate::web_search::config::load_web_search_config;
use crate::web_search::provider::web_search;

pub fn router() -> Router<AppState> {
  Router::new().route("/api/web-search", post(web_search_handler))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchRequest {
  pub query: String,
  #[serde(default)]
  pub max_results: Option<usize>,
}

async fn web_search_handler(
  State(state): State<AppState>,
  headers: HeaderMap,
  Json(payload): Json<WebSearchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
  let principal = resolve_principal(&state, &headers).await?;
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
  let trimmed = payload.query.trim();
  if trimmed.is_empty() {
    return Err(ApiError::bad_request("query is required"));
  }
  let max_results = payload.max_results.unwrap_or(10).clamp(1, 50);

  let config = load_web_search_config(&state)
    .await?
    .ok_or_else(|| ApiError::bad_request("web search provider is not configured"))?;
  let results = web_search(&config, trimmed, max_results).await?;
  Ok(Json(json!({ "results": results })))
}
```

In `crates/knowledge-server/src/http/router.rs`, merge the new router:

```rust
use crate::web_search;
```

And inside `build_router`:

```rust
    .merge(web_search::routes::router())
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p rust-integration --test web_search_api -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/web_search/routes.rs crates/knowledge-server/src/http/router.rs tests/rust-integration/tests/web_search_api.rs
git commit -m "feat: add POST /api/web-search route"
```

---

### Task 9: Admin API client + queries hook

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts`

Patterns: `updateSystemSettings` (~line 916) for system settings; `sweepProjectReviews` for a POST that returns JSON.

- [ ] **Step 1: Extend the SystemSettings schema**

Find the existing `systemSettingsSchema` / `getSystemSettings`/`updateSystemSettings` in `apps/admin/src/features/shared/api.ts` (the get function lives near the existing `providerEmbeddingModel` logic). Add to the schema:

```ts
  searchProvider: z.string(),
  searchApiKeyConfigured: z.boolean(),
  serpapiEngine: z.string().nullable(),
  searxngUrl: z.string().nullable(),
  searxngCategories: z.array(z.string()),
  ollamaSearchUrl: z.string().nullable(),
```

Extend the `updateSystemSettings` input type:

```ts
  searchProvider?: string;
  searchApiKey?: string;
  serpapiEngine?: string;
  searxngUrl?: string;
  searxngCategories?: string[];
  ollamaSearchUrl?: string;
```

And the body it sends — include all the new keys (drop undefineds).

Add a new function:

```ts
const webSearchResultSchema = z.object({
  title: z.string(),
  url: z.string(),
  snippet: z.string(),
  source: z.string(),
});

const webSearchResponseSchema = z.object({
  results: z.array(webSearchResultSchema),
});

export type WebSearchResult = z.infer<typeof webSearchResultSchema>;

export async function runWebSearch(input: { query: string; maxResults?: number }) {
  return apiFetch(
    `/api/web-search`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({ query: input.query, maxResults: input.maxResults }),
    },
    webSearchResponseSchema,
  );
}
```

- [ ] **Step 2: Verify build**

Run: `npm run test --workspace @knowledge/admin`
Expected: existing tests PASS (zod schema accepts the new optional fields against the existing handler output even before Task 7 lands; once Task 7 is merged, fields are populated).

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/shared/api.ts
git commit -m "feat: add web search api client and extended settings schema"
```

---

### Task 10: Admin Settings page — Web Search section

**Files:**
- Modify: `apps/admin/src/features/settings/page.tsx`
- Modify or Create: `apps/admin/src/features/settings/page.test.tsx`

- [ ] **Step 1: Write the failing test**

Look at the existing settings page test (if any) for a template. If none, create `apps/admin/src/features/settings/page.test.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { SettingsPage } from "./page";

const mockSettings = vi.fn();
const mockSave = vi.fn();
const mockRunWebSearch = vi.fn();

vi.mock("./queries", () => ({
  useSystemSettingsQuery: () => ({ data: mockSettings(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: mockSave, isPending: false }),
  useRunWebSearchMutation: () => ({ mutateAsync: mockRunWebSearch, isPending: false, data: undefined }),
}));

function renderPage() {
  const queryClient = new QueryClient();
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/settings"]}>
        <SettingsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("settings page web search section", () => {
  it("renders the search provider fields and Test Search button", () => {
    mockSettings.mockReturnValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 25,
      providerBaseUrl: null,
      providerApiKeyConfigured: false,
      providerModel: null,
      providerEmbeddingModel: null,
      providerTimeoutSeconds: 30,
      searchProvider: "none",
      searchApiKeyConfigured: false,
      serpapiEngine: "google",
      searxngUrl: null,
      searxngCategories: ["general"],
      ollamaSearchUrl: null,
    });

    renderPage();

    expect(screen.getByLabelText(/Search Provider/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Search API Key/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/SearXNG Instance URL/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Test Search/i })).toBeInTheDocument();
  });

  it("calls runWebSearch with the typed query when Test Search is clicked", async () => {
    const user = userEvent.setup();
    mockSettings.mockReturnValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 25,
      providerBaseUrl: null,
      providerApiKeyConfigured: false,
      providerModel: null,
      providerEmbeddingModel: null,
      providerTimeoutSeconds: 30,
      searchProvider: "searxng",
      searchApiKeyConfigured: false,
      serpapiEngine: "google",
      searxngUrl: "http://127.0.0.1:18080",
      searxngCategories: ["general"],
      ollamaSearchUrl: null,
    });
    mockRunWebSearch.mockResolvedValue({ results: [] });

    renderPage();

    await user.type(screen.getByLabelText(/Test Query/i), "hello world");
    await user.click(screen.getByRole("button", { name: /Test Search/i }));

    expect(mockRunWebSearch).toHaveBeenCalledWith({ query: "hello world" });
  });
});
```

- [ ] **Step 2: Add the queries hook**

In `apps/admin/src/features/settings/queries.ts`, append:

```ts
import { useMutation } from "@tanstack/react-query";

import { runWebSearch } from "../shared/api";

export function useRunWebSearchMutation() {
  return useMutation({ mutationFn: runWebSearch });
}
```

(Keep the existing two hooks `useSystemSettingsQuery` and `useUpdateSystemSettingsMutation` intact — this just adds the third.)

- [ ] **Step 3: Extend the settings page state and hydration**

The existing `page.tsx` follows a pattern of one `useState` per editable field, a hydration `useEffect`, and a `handleSave` that builds the payload. Mirror that for the new fields.

Edit `apps/admin/src/features/settings/page.tsx`:

Replace the import block and `useSystemSettingsQuery`/`useUpdateSystemSettingsMutation` import with:

```tsx
import { useSystemSettingsQuery, useUpdateSystemSettingsMutation, useRunWebSearchMutation } from "./queries";
```

After the existing `useState` declarations (after `providerTimeoutSeconds`), add:

```tsx
  const [searchProvider, setSearchProvider] = useState("none");
  const [searchApiKey, setSearchApiKey] = useState("");
  const [serpapiEngine, setSerpapiEngine] = useState("google");
  const [searxngUrl, setSearxngUrl] = useState("");
  const [searxngCategories, setSearxngCategories] = useState("general");
  const [ollamaSearchUrl, setOllamaSearchUrl] = useState("");
  const [testQuery, setTestQuery] = useState("");
  const [testError, setTestError] = useState("");
  const runWebSearch = useRunWebSearchMutation();
```

In the existing hydration `useEffect`, add inside the body (after the existing `setProviderTimeoutSeconds(...)` line):

```tsx
    setSearchProvider(settings.data.searchProvider ?? "none");
    setSerpapiEngine(settings.data.serpapiEngine ?? "google");
    setSearxngUrl(settings.data.searxngUrl ?? "");
    setSearxngCategories((settings.data.searxngCategories ?? ["general"]).join(", "));
    setOllamaSearchUrl(settings.data.ollamaSearchUrl ?? "");
```

Extend the `useEffect` dependency array (append):

```tsx
    settings.data?.searchProvider,
    settings.data?.serpapiEngine,
    settings.data?.searxngUrl,
    settings.data?.searxngCategories,
    settings.data?.ollamaSearchUrl,
```

In `handleSave`, extend the `payload` object before the `try` block:

```tsx
    const categories = searxngCategories
      .split(",")
      .map((value) => value.trim())
      .filter((value) => value.length > 0);
    const webSearchPayload = {
      searchProvider,
      serpapiEngine,
      searxngUrl,
      searxngCategories: categories.length > 0 ? categories : ["general"],
      ollamaSearchUrl,
      ...(searchApiKey.trim() ? { searchApiKey: searchApiKey.trim() } : {}),
    };
    const fullPayload = { ...payload, ...webSearchPayload };
```

And change `await updateSettings.mutateAsync(payload);` → `await updateSettings.mutateAsync(fullPayload);` Also clear `searchApiKey` in the success block: `setSearchApiKey("");`.

- [ ] **Step 4: Add the Web Search card to the JSX**

In `page.tsx`, after the existing `<Card>` block (just before the closing `</PageSection>`), add a second card:

```tsx
      <Card className="panel">
        <CardContent className="grid gap-4 p-6">
          <h2 className="text-lg font-semibold">Web Search</h2>
          <label className="grid gap-2 text-sm font-medium">
            Search Provider
            <select
              aria-label="Search Provider"
              className="rounded border bg-background px-3 py-2"
              onChange={(event) => {
                setIsDirty(true);
                setSearchProvider(event.target.value);
              }}
              value={searchProvider}
            >
              <option value="none">none</option>
              <option value="tavily">tavily</option>
              <option value="serpapi">serpapi</option>
              <option value="searxng">searxng</option>
              <option value="ollama">ollama</option>
            </select>
          </label>
          {searchProvider !== "none" && searchProvider !== "searxng" ? (
            <label className="grid gap-2 text-sm font-medium">
              Search API Key
              <Input
                onChange={(event) => {
                  setIsDirty(true);
                  setSearchApiKey(event.target.value);
                }}
                placeholder="Leave blank to keep the current key"
                type="password"
                value={searchApiKey}
              />
            </label>
          ) : null}
          {settings.data?.searchApiKeyConfigured && searchProvider !== "none" && searchProvider !== "searxng" ? (
            <p className="text-sm text-muted-foreground">Search API key configured</p>
          ) : null}
          {searchProvider === "serpapi" ? (
            <label className="grid gap-2 text-sm font-medium">
              SerpApi Engine
              <select
                aria-label="SerpApi Engine"
                className="rounded border bg-background px-3 py-2"
                onChange={(event) => {
                  setIsDirty(true);
                  setSerpapiEngine(event.target.value);
                }}
                value={serpapiEngine}
              >
                <option value="google">google</option>
                <option value="google_news">google_news</option>
                <option value="google_scholar">google_scholar</option>
                <option value="bing">bing</option>
                <option value="duckduckgo">duckduckgo</option>
              </select>
            </label>
          ) : null}
          {searchProvider === "searxng" ? (
            <>
              <label className="grid gap-2 text-sm font-medium">
                SearXNG Instance URL
                <Input
                  onChange={(event) => {
                    setIsDirty(true);
                    setSearxngUrl(event.target.value);
                  }}
                  placeholder="https://search.example.com"
                  value={searxngUrl}
                />
              </label>
              <label className="grid gap-2 text-sm font-medium">
                SearXNG Categories (comma-separated)
                <Input
                  onChange={(event) => {
                    setIsDirty(true);
                    setSearxngCategories(event.target.value);
                  }}
                  value={searxngCategories}
                />
              </label>
            </>
          ) : null}
          {searchProvider === "ollama" ? (
            <label className="grid gap-2 text-sm font-medium">
              Ollama Search URL
              <Input
                onChange={(event) => {
                  setIsDirty(true);
                  setOllamaSearchUrl(event.target.value);
                }}
                placeholder="https://ollama.com"
                value={ollamaSearchUrl}
              />
            </label>
          ) : null}
          <div className="grid gap-2 border-t pt-4">
            <label className="grid gap-2 text-sm font-medium">
              Test Query
              <Input
                onChange={(event) => setTestQuery(event.target.value)}
                placeholder="e.g. knowledge graphs"
                value={testQuery}
              />
            </label>
            <div className="flex gap-2">
              <Button
                disabled={runWebSearch.isPending || testQuery.trim().length === 0}
                onClick={async () => {
                  setTestError("");
                  try {
                    await runWebSearch.mutateAsync({ query: testQuery.trim() });
                  } catch (error) {
                    setTestError(error instanceof Error ? error.message : "Web search failed.");
                  }
                }}
                variant="outline"
              >
                Test Search
              </Button>
            </div>
            {testError ? (
              <Alert>
                <AlertTitle>Web search failed</AlertTitle>
                <AlertDescription>{testError}</AlertDescription>
              </Alert>
            ) : null}
            {runWebSearch.data?.results.length ? (
              <ul className="grid gap-3">
                {runWebSearch.data.results.map((result) => (
                  <li key={result.url} className="rounded border p-3">
                    <a className="font-medium" href={result.url} rel="noreferrer" target="_blank">
                      {result.title}
                    </a>
                    {result.source ? (
                      <p className="text-xs text-muted-foreground">{result.source}</p>
                    ) : null}
                    {result.snippet ? <p className="mt-1 text-sm">{result.snippet}</p> : null}
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </CardContent>
      </Card>
```

- [ ] **Step 3: Run the page test**

Run: `npm run test --workspace @knowledge/admin -- src/features/settings/page.test.tsx`
Expected: 2 tests PASS.

- [ ] **Step 4: Run the full admin suite**

Run: `npm run test --workspace @knowledge/admin`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/settings/page.tsx apps/admin/src/features/settings/page.test.tsx apps/admin/src/features/settings/queries.ts
git commit -m "feat: add web search section to settings page"
```

---

### Task 11: e2e — settings + mock SearXNG roundtrip

**Files:**
- Modify: `tests/web/mock-openai.mjs`
- Create: `tests/web/tests/web-search.spec.ts`

The existing mock server already handles `/v1/chat/completions` and `/v1/embeddings`. Add a SearXNG-compatible `/search` endpoint (responds with `{results: [...]}`). The e2e spec configures SearXNG to point at `http://127.0.0.1:18080`, then clicks Test Search.

- [ ] **Step 1: Extend the mock server**

In `tests/web/mock-openai.mjs`, add this block before the final 404 catch-all:

```js
  if (request.method === "GET" && request.url?.startsWith("/search")) {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(
      JSON.stringify({
        results: [
          {
            title: "Knowledge graphs explained",
            url: "https://example.com/knowledge-graphs",
            content: "An overview of knowledge graphs and their applications.",
            engine: "mock",
          },
        ],
      }),
    );
    return;
  }
```

- [ ] **Step 2: Write the spec**

Create `tests/web/tests/web-search.spec.ts`:

```ts
import { expect, test } from "@playwright/test";

test("admin configures searxng and runs a test search", async ({ page }) => {
  await signInAsAdmin(page);

  await page.getByRole("link", { name: "Settings" }).click();

  await page.getByLabel(/Search Provider/i).selectOption("searxng");
  await page.getByLabel(/SearXNG Instance URL/i).fill("http://127.0.0.1:18080");
  await page.getByRole("button", { name: /Save Settings/i }).click();

  await page.getByLabel(/Test Query/i).fill("knowledge graphs");
  await page.getByRole("button", { name: /Test Search/i }).click();

  await expect(page.getByText("Knowledge graphs explained")).toBeVisible({ timeout: 10_000 });
});

async function signInAsAdmin(page: import("@playwright/test").Page) {
  await page.goto("/login");
  await page.getByLabel("Username").fill("admin");
  await page.getByLabel("Password").fill("secret-password");
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.waitForURL("**/projects");
}
```

- [ ] **Step 3: Run the spec**

Run: `npm run test --workspace @knowledge/web -- tests/web-search.spec.ts`
Expected: PASS.

- [ ] **Step 4: Run the full playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: 9 tests PASS (8 existing + new web-search). `workers: 1` is already pinned in the playwright config, so the system_settings race that's affected previous specs won't bite.

- [ ] **Step 5: Commit**

```bash
git add tests/web/mock-openai.mjs tests/web/tests/web-search.spec.ts
git commit -m "test: add web search e2e flow with mock searxng"
```

---

### Task 12: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Rust lint + tests**

Run: `cargo fmt --all -- --check`
Expected: clean. If reports diffs, run `cargo fmt --all` and commit.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo test --workspace -- --test-threads=1`
Expected: PASS — includes 5 new provider unit tests + 1 new `web_search_api` integration test.

- [ ] **Step 2: Frontend tests + lint**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS (with the 2 new settings page tests).

Run: `npm run test --workspace @knowledge/api-client`
Expected: PASS (no changes here, regression check).

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: PASS (no changes here, regression check).

Run: `npm run lint`
Expected: tsc + clippy clean.

- [ ] **Step 3: Playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: 9 specs PASS, including the new web-search.

- [ ] **Step 4: Final commit (only if fixes were needed)**

```bash
git status
```

If steps 1–3 required fixes, commit them. Otherwise the work is complete.
