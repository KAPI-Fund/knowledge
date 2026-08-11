# Firecrawl Fetch Provider Design

**Date:** 2026-07-06
**Status:** Approved (design), ready for planning

## Problem

The canvas URL node fetches a page directly (`crates/knowledge-server/src/canvas/service.rs`),
converts the HTML to markdown with a hand-rolled `scraper`-based extractor, and guards against SSRF
with a custom resolver + IP screen. This works for simple static pages but fails on JS-rendered sites:
`https://baidu.com` returns a 227-byte JS/`<noscript>` meta-refresh stub, which we only partially
worked around by following meta-refresh. Many modern sites need a real browser to render content at
all.

The user's directive: replace the built-in direct fetch with a professional open-source scraping
service. After research and discussion, scope is locked to **Firecrawl only**, and the built-in fetch
path is to be **removed entirely** — the URL node fetches exclusively through the configured external
provider.

Firecrawl renders JS (solving the baidu-style stub problem), returns clean markdown + metadata, and
exposes a simple HTTP API (`POST /v1/scrape`). It is self-hostable (Docker) or usable via a hosted
endpoint, so the operator points a base URL + API key at whichever they run.

## Locked decisions

1. **Firecrawl is the only fetch provider.** No XCrawl (it is a Node.js/Puppeteer library, not an HTTP
   service — incompatible with the Rust backend and the browser SPA's CORS constraints), no generic
   reader endpoint.
2. **The built-in direct fetch is removed.** All the SSRF-guard machinery (`validate_public_url`,
   `screen_resolved_addrs`, `is_blocked_ip`, `PublicOnlyResolver`, `build_extractor_client`), the
   meta-refresh follower (`meta_refresh_target`, `attr_value`), the HTML→markdown converter
   (`html_to_markdown`, `inline_markdown`), and `MAX_EXTRACT_BYTES` are deleted, along with all their
   unit tests. When Firecrawl fetches the target, our process never opens a socket to the target URL,
   so target-URL SSRF screening becomes moot.
3. **`allow_private_fetch` is removed end-to-end.** It existed only to let the built-in fetch reach
   fake-IP-proxy addresses. With the built-in fetch gone, the whole chain is deleted:
   `AppConfig::allow_private_fetch`, its `from_env` read, `AppState::allow_private_fetch`, the handler
   plumbing, and the `KNOWLEDGE_ALLOW_PRIVATE_FETCH` env in `docker-compose.yml`.
4. **Deployment: configure a base URL, do not add a compose service.** `fetch_provider_configs.firecrawl.baseUrl`
   defaults to `https://api.firecrawl.dev`; the operator may point it at a self-hosted instance
   (e.g. `http://host:3002`). `docker-compose.yml` is not changed to add a Firecrawl service.
5. This is a **new build** — no upstream_llm_wiki logic to port for scraping providers.

## Architecture

Mirror the existing `search_provider` + `search_provider_configs` JSONB pattern exactly. That pattern
is already the template the codebase uses for a "selector string + per-provider config map + deep-merge
+ redaction" capability, so the fetch provider becomes structurally identical and consistent.

### Data model (system_settings singleton row, id = 1)

New migration `0017_fetch_provider_settings.sql`:

```sql
ALTER TABLE system_settings ADD COLUMN fetch_provider TEXT NOT NULL DEFAULT 'none';
ALTER TABLE system_settings ADD COLUMN fetch_provider_configs JSONB NOT NULL DEFAULT '{}'::jsonb;
```

- `fetch_provider`: selector, `'none'` | `'firecrawl'`. Default `'none'` — a fresh install has no
  fetch provider; the URL node returns a clear "not configured" error until one is set.
- `fetch_provider_configs`: `{ "firecrawl": { "baseUrl": "...", "apiKey": "..." } }`. A blank/absent
  `baseUrl` falls back to `https://api.firecrawl.dev` at resolve time.

### New module: `crates/knowledge-server/src/web_fetch/`

`web_fetch/mod.rs`:
```rust
pub mod config;
pub mod firecrawl;

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
```

`web_fetch/config.rs` (mirrors `web_search/config.rs`):
```rust
pub const DEFAULT_FIRECRAWL_BASE_URL: &str = "https://api.firecrawl.dev";

#[derive(Debug, Clone)]
pub struct FetchConfig {
    pub provider: FetchProvider,
    pub base_url: String,     // defaulted to DEFAULT_FIRECRAWL_BASE_URL when blank/absent
    pub api_key: Option<String>,
}

pub(crate) fn parse_fetch_provider(value: &str) -> Result<Option<FetchProvider>, ApiError>;
// "none" -> Ok(None); "firecrawl" -> Ok(Some(Firecrawl)); else internal error.

pub(crate) fn resolve_fetch_config(active: &str, configs: &Value) -> Result<Option<FetchConfig>, ApiError>;
// Pure. Reads configs["firecrawl"] { baseUrl, apiKey }, applies the base-url default.

pub async fn load_fetch_config(state: &AppState) -> Result<Option<FetchConfig>, ApiError>;
// SELECT fetch_provider, fetch_provider_configs FROM system_settings WHERE id = 1; then resolve.
```

`web_fetch/firecrawl.rs`:
```rust
/// Scrape a URL via Firecrawl's /v1/scrape endpoint and return readable markdown.
/// POST {base_url}/v1/scrape  (Bearer auth when api_key is set)
///   { "url": <target>, "formats": ["markdown"], "onlyMainContent": true }
/// -> data.markdown + data.metadata.title
pub async fn scrape(config: &FetchConfig, url: &str) -> Result<ExtractedPage, ApiError>;
```
- Reqwest client with a 60s timeout (browser rendering is slower than a raw GET).
- Endpoint: `format!("{}/v1/scrape", base_url.trim_end_matches('/'))`.
- Auth: `Authorization: Bearer {api_key}` header only when `api_key` is present and non-empty
  (self-hosted instances may not require a key).
- Response shape (only the fields we use):
  ```rust
  struct FirecrawlResponse { success: Option<bool>, data: Option<FirecrawlData>, error: Option<String> }
  struct FirecrawlData { markdown: Option<String>, metadata: Option<FirecrawlMetadata> }
  struct FirecrawlMetadata { title: Option<String> }
  ```
- Map to `ExtractedPage { title: metadata.title.unwrap_or("Untitled"), markdown: data.markdown.unwrap_or_default() }`.
- Non-2xx or `success == Some(false)`: return `ApiError::bad_request` including Firecrawl's `error`
  string / status body so the failure surfaces in the node.

`ExtractedPage` moves from `canvas/service.rs` to `web_fetch/mod.rs` (or `web_fetch/firecrawl.rs`) since
its only remaining producer is Firecrawl; `canvas/service.rs` re-exports or references it as needed for
`collect_reference_blocks` (which does not actually use `ExtractedPage` — it reads node data — so no
coupling remains).

### Wiring: `extract_url_handler` (`canvas/routes.rs`)

```rust
async fn extract_url_handler(...) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;

    let Some(config) = crate::web_fetch::config::load_fetch_config(&state).await? else {
        return Ok(Json(json!({
            "status": "error", "title": "", "markdown": "",
            "error": "web page fetch is not configured; set a fetch provider in Settings"
        })));
    };

    match crate::web_fetch::firecrawl::scrape(&config, &body.url).await {
        Ok(page) => Ok(Json(json!({
            "status": "ok", "title": page.title, "markdown": page.markdown, "error": null
        }))),
        Err(error) => Ok(Json(json!({
            "status": "error", "title": "", "markdown": "", "error": error.to_string()
        }))),
    }
}
```
The frontend `extractResultSchema` (`{ status, title, markdown, error }`) is unchanged — the "not
configured" case flows through as a normal `status: "error"` with a helpful message.

### Settings routes (`settings/routes.rs`)

Add a `fetch` block symmetric to `search`:

- `UpdateSettingsRequest` gains `#[serde(default)] fetch: Option<FetchSettingsBlock>`.
- `FetchSettingsBlock { provider: Option<String>, providers: Option<Value> }` (same shape as
  `SearchSettingsBlock`; `providers` is the per-provider map, here just `{ firecrawl: {...} }`).
- `validate_fetch_provider(provider)` — accepts `"none" | "firecrawl"`; called before the tx opens.
- In `update_settings`, add a `fetch` handler mirroring the `search` handler: `UPDATE ... SET
  fetch_provider = $1` when provider present; deep-merge `fetch_provider_configs` via a reused merge
  helper when `providers` present.
- `merge_fetch_provider_configs` — reuse the exact deep-merge semantics of
  `merge_search_provider_configs` (blank string = keep, null = clear, other = set). Factor the shared
  merge into one generic helper both call, to stay DRY.
- `redact_fetch_configs(configs)` — mirror `redact_search_configs` for the `firecrawl` block: replace
  `apiKey` with `apiKeyConfigured` boolean, pass `baseUrl` through.
- `build_settings_response` — select `fetch_provider` + `fetch_provider_configs`, add a `fetch` section:
  `{ "provider": fetch_provider, "providers": redact_fetch_configs(...) }`.

### Admin UI

New `apps/admin/src/features/settings/sections/fetch-section.tsx`, mirroring `web-search-section.tsx`
(shadcn `Card`/`Input`/`Select`/`Switch`/`Button`):
- Provider `Select`: `none` | `firecrawl`.
- When `firecrawl`: `Firecrawl API Key` (password, blank = keep stored; clear-key `Switch` when a key
  is configured) + `Firecrawl Base URL` (`placeholder="https://api.firecrawl.dev"`, blank = revert to
  default).
- `save()` sends `{ fetch: { provider, providers: { firecrawl: { apiKey, baseUrl } } } }` using the
  same `keyValue` / `urlOrClear` helpers as the search section.

Mount it: add `"fetch"` to `SettingsSectionId` + `ITEMS` in `settings-nav.tsx` (label `"Web Fetch"`),
and render `<FetchSection />` for `section === "fetch"` in `page.tsx`. (No test-search analogue — there
is no cheap "test fetch" without picking a URL; omit it. YAGNI.)

### `api.ts` (`apps/admin/src/features/shared`)

- Add a `fetchProviderConfigsSchema` = `{ firecrawl?: { apiKeyConfigured?: boolean, baseUrl?: string } }`.
- Add `fetch?: { provider?, providers? }` to `settingsSchema`.
- Add `fetch?` to `updateSystemSettings`'s input type and built payload, reusing the same
  `filter(([, v]) => v !== "")` per-provider cleanup as `search`.

## Removals (clean cutover, no legacy)

**Backend `canvas/service.rs`:** delete `html_to_markdown`, `inline_markdown`, `MAX_EXTRACT_BYTES`,
`validate_public_url`, `is_blocked_ip`, `screen_resolved_addrs`, `PublicOnlyResolver`, `box_dns_err`,
`build_extractor_client`, `attr_value`, `meta_refresh_target`, `fetch_url`, and every associated unit
test in `mod url_tests` (keep `mod context_tests` and its helpers — those cover
`collect_reference_blocks` / `search_results_to_markdown` / `build_analyze_prompt`, which stay). Remove
now-unused imports (`scraper`, `std::net::*`, `std::sync::Arc`, `std::time::Duration`, `HashSet`) as the
compiler flags them.

**`ExtractedPage`:** relocate to `web_fetch` (its only remaining producer).

**`allow_private_fetch` chain:** `config.rs` (field + `for_tests` + `from_env` read + initializer),
`app/state.rs` (field), `lib.rs` (`bootstrap_state` initializer), `canvas/routes.rs` (handler),
`tests/rust-integration/tests/support/mod.rs` (`bootstrap_state_without_scheduler` initializer),
`tests/rust-integration/tests/docker_stack_smoke.rs` + `provider_connections_race.rs` (the
`allow_private_fetch: false` lines in their inline `AppConfig` literals), and `docker-compose.yml`
(`KNOWLEDGE_ALLOW_PRIVATE_FETCH` env + its comment).

## Testing

**Backend unit tests:**
- `web_fetch/config.rs`: `parse_fetch_provider` known/none/invalid; `resolve_fetch_config` reads the
  firecrawl block, applies the base-url default when absent, returns `None` for `"none"`.
- `web_fetch/firecrawl.rs`: a `tokio` TCP mock (same pattern as `web_search/provider.rs` tests)
  returning a canned `{ success, data: { markdown, metadata: { title } } }` — assert
  `scrape` maps it to `ExtractedPage`; a mock returning `{ success: false, error }` / non-2xx maps to a
  bad_request error carrying the message.
- `settings/routes.rs`: extend the merge tests to cover `merge_fetch_provider_configs` (or the shared
  helper) for the firecrawl block; `validate_fetch_provider` accepts `none`/`firecrawl`, rejects others.

**Frontend tests:**
- `fetch-section.test.tsx` (mirror `web-search-section.test.tsx`): hydrates from a firecrawl-active
  settings mock; `save()` sends `{ fetch: { provider, providers: { firecrawl: { apiKey: "", baseUrl } } } }`;
  clear-key toggle sends `apiKey: null`; blanking base URL sends `baseUrl: null`.
- `api.test.tsx`: `updateSystemSettings` with a `fetch` block builds the expected payload.

**Integration / E2E:**
- Existing `docker_stack_smoke.rs` and `provider_connections_race.rs` compile after the
  `allow_private_fetch` field removal.
- Manual: fresh DB → URL node returns "not configured"; set firecrawl provider + base URL/key in
  Settings → URL node scrapes `https://baidu.com` and returns real title/markdown.

## File summary

**Backend:**
- New: `migrations/0017_fetch_provider_settings.sql`, `src/web_fetch/mod.rs`, `src/web_fetch/config.rs`,
  `src/web_fetch/firecrawl.rs`.
- Modified: `src/lib.rs` (register `web_fetch` module; drop `allow_private_fetch`), `src/config.rs`,
  `src/app/state.rs`, `src/canvas/service.rs` (delete fetch machinery), `src/canvas/routes.rs`
  (rewire handler), `src/settings/routes.rs` (fetch block + validators + redaction + response).
- Modified tests/config: `tests/rust-integration/tests/support/mod.rs`, `docker_stack_smoke.rs`,
  `provider_connections_race.rs`, `docker-compose.yml`.

**Frontend:**
- New: `src/features/settings/sections/fetch-section.tsx`, `fetch-section.test.tsx`.
- Modified: `src/features/shared/api.ts`, `src/features/settings/page.tsx`,
  `src/features/settings/settings-nav.tsx`, `src/features/settings/api.test.tsx`.
</content>
</invoke>
