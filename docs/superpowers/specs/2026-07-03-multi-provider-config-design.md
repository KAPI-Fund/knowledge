# Multi-Provider Configuration & Canvas Node Wiring — Design

Date: 2026-07-03

## Context

The admin app configures AI capabilities through a single global provider stored in the
`system_settings` singleton (flat `provider_*` columns). Every capability — chat completions, the
canvas `ai_analyze` node, the canvas `ai_image` node, and embedding/RAG retrieval — reuses the same
`provider_base_url` / `provider_api_key` / `provider_model`. Web search is already multi-provider
(Tavily / SerpApi / SearXNG / Ollama) but stores only one provider's credentials at a time.

Two concrete problems drive this work:

1. **Image generation is effectively broken.** `run_image_skill`
   (`crates/knowledge-server/src/canvas/routes.rs`) builds its provider from `load_query_settings`
   and calls `POST {base}/v1/images/generations` with `provider_model` — the *text/chat* model. When
   the configured chat model is a text model (the normal case), image generation fails. There is no
   separate image endpoint/model configuration.

2. **No multi-provider support.** An admin cannot register more than one LLM connection, and switching
   web-search providers discards the previous provider's API key.

The user asked to (a) support configuring **multiple providers**, (b) **redesign the configuration UI**
(including the image provider and the web-search provider, Tavily-primary), and (c) **wire up the real
functionality** of the canvas image and search nodes.

Investigation finding: the **search node is already wired end to end** to real APIs
(`web_search/provider.rs` → Tavily etc.) and reads `system_settings.search_*`. So "打通搜索节点" is
primarily a configuration-UI + verification task, not new plumbing. The **image node is also wired end
to end** but points at the wrong (text) model; the fix is a dedicated image configuration block.

### Reference: upstream_llm_wiki (required to copy)

Per project memory, key feature logic must be ported from `upstream_llm_wiki`, not invented. Its
model (`upstream_llm_wiki/src/stores/wiki-store.ts`, `src/components/settings/**`):

- **Per-capability configuration sections**, not one flat provider. A category rail (General, LLM,
  Embedding, Multimodal, Web Search, …) switches between independent config sections.
- **LLM = a list of presets** (`LlmConfig` + `ProviderOverride` in `ProviderConfigs`), one active at a
  time (`activePresetId`). Switching presets retains each preset's credentials.
- **Embedding = an independent config block** (`EmbeddingConfig`: enabled, endpoint, apiKey, model,
  timeout).
- **Web search = multiple providers pre-configured, one active** (`SearchApiConfig` +
  `SearchProviderConfigs`), so switching providers retains each provider's key. Tavily called at
  `src/lib/web-search.ts` (`POST https://api.tavily.com/search`, body
  `{api_key, query, max_results, search_depth:"advanced", include_answer:false}`).
- **Note:** upstream has **no image *generation*** config — its "multimodal" is image *captioning*
  (vision input) at ingest time. So the image-generation block below is net-new, modeled structurally
  on upstream's "dedicated endpoint" pattern but with generation fields (model, size).

## Decisions (confirmed with the user)

- **Structure:** a preset **connection list** for the LLM capability (upstream model). ✔
- **Scope:** redesign covers **all four** capabilities — LLM (chat/analyze), image, web search,
  embedding. ✔
- **Binding:** LLM (chat/analyze) uses the **preset connection list**; **embedding** and **image** are
  **their own independent config blocks**; **web search** stays its own provider selector. ✔
- **Web search:** keep all four providers, **Tavily default**, retain each provider's key on switch. ✔
- **Image:** **OpenAI-compatible** generation endpoint, **with a configurable size** (e.g.
  `1024x1024`); default params otherwise. ✔
- **Migration:** **migrate** existing single-provider config forward — no configured keys lost. ✔
- **Settings layout:** a **left sub-nav** within the Settings page (mirrors upstream's category rail). ✔

## Data model

### New table: `provider_connections` (LLM preset list)

The multi-connection list used by chat completions and the canvas `ai_analyze` node.

| Column | Type | Notes |
|---|---|---|
| `id` | UUID / TEXT PK | generated |
| `label` | TEXT NOT NULL | display name, e.g. "OpenAI", "Local vLLM" |
| `base_url` | TEXT NOT NULL | e.g. `https://api.openai.com` |
| `api_key` | TEXT | nullable; stored plaintext (consistent with current design) |
| `model` | TEXT NOT NULL | chat/analyze model, e.g. `gpt-4o` |
| `timeout_seconds` | BIGINT | nullable; default applied at load |
| `is_active` | BOOLEAN NOT NULL | exactly one row true (enforced in service layer) |
| `sort_order` | INT NOT NULL | list ordering |
| `created_at` / `updated_at` | TIMESTAMPTZ | |

"Exactly one active" is maintained transactionally: activating a connection clears `is_active` on all
others in the same statement/transaction.

### Extended `system_settings` (still `id = 1` singleton) — independent capability blocks

Embedding block (independent config):
- `embedding_enabled BOOLEAN NOT NULL DEFAULT false`
- `embedding_base_url TEXT`
- `embedding_api_key TEXT`
- `embedding_model TEXT`
- `embedding_timeout_seconds BIGINT`

Image block (net-new, OpenAI-compatible generation):
- `image_base_url TEXT`
- `image_api_key TEXT`
- `image_model TEXT` — e.g. `gpt-image-1`, `dall-e-3`
- `image_size TEXT NOT NULL DEFAULT '1024x1024'`
- `image_timeout_seconds BIGINT`

Web search (extend existing): add
- `search_provider_configs JSONB NOT NULL DEFAULT '{}'` — per-provider saved fields, shape mirrors
  upstream `SearchProviderConfigs`:
  ```json
  {
    "tavily":  { "apiKey": "...", "baseUrl": "https://api.tavily.com" },
    "serpapi": { "apiKey": "...", "engine": "google", "baseUrl": "https://serpapi.com" },
    "searxng": { "url": "https://...", "categories": ["general"] },
    "ollama":  { "apiKey": "...", "url": "https://ollama.com" }
  }
  ```
  `search_provider` remains the active-provider selector (`none|tavily|serpapi|searxng|ollama`).

Retained as-is: `language`, `default_query_limit`.

The legacy flat columns (`provider_*`, and the flat `search_*` fields) are **read during migration to
seed** the new structures, then left in place (removing them is out of scope; loaders stop reading
them). This keeps the migration reversible and low-risk.

### Migration `0015_multi_provider.sql`

1. Create `provider_connections`.
2. Insert one connection **"Default"** seeded from `system_settings.provider_base_url`,
   `provider_api_key`, `provider_model`, `provider_timeout_seconds`, with `is_active = true`,
   `sort_order = 0`. (Insert only when a base_url or model is present; otherwise no seed row and the
   admin configures from scratch.)
3. `ALTER TABLE system_settings ADD` the embedding block, image block, and `search_provider_configs`.
4. Seed embedding block from `provider_base_url` / `provider_api_key` / `provider_embedding_model`
   (+ `embedding_enabled = (provider_embedding_model IS NOT NULL)`).
5. Seed image block: `image_base_url`/`image_api_key` from `provider_*`, `image_model` left NULL (admin
   picks an image model), `image_size = '1024x1024'`.
6. Seed `search_provider_configs` from the existing flat `search_*` columns so the active provider's
   key/fields survive.

## Backend: capability loaders and usage sites

Introduce focused loaders (each returns a typed config or an explicit "not configured" result), and
repoint the five instantiation sites the exploration identified:

| Capability | New loader | Usage site(s) repointed |
|---|---|---|
| Chat | `load_active_connection` | `chat/routes.rs` |
| Analyze (`ai_analyze`) | `load_active_connection` | `canvas/routes.rs` `build_provider` |
| Image (`ai_image`) | `load_image_config` | `canvas/routes.rs` `run_image_skill` |
| Embedding | `load_embedding_config` (updated) | `retrieval/service.rs` |
| Web search | `load_web_search_config` (updated) | `web_search/config.rs` consumers |

- `load_active_connection` selects the `is_active` row from `provider_connections`; returns a
  `bad_request` ("no active provider connection") when none exists — same failure contract as today's
  `build_provider`.
- `load_image_config` reads the image block; validation requires `image_base_url` + `image_model`
  non-empty, else `bad_request("image provider is not configured")`.
- `load_embedding_config` reads the embedding block (was reading `provider_*`); returns `None` when
  `embedding_enabled = false`, matching its current Option contract.
- `load_web_search_config` resolves the active provider's fields from `search_provider_configs`
  (falling back to seeded values), preserving the existing `WebSearchConfig` shape so
  `web_search/provider.rs` is untouched.

### Image request gains `size`

`ProviderImageRequest { prompt }` → `ProviderImageRequest { prompt, size }`. `ImageGenerationRequest`
(the wire body) adds `size` (serialized only when the provider accepts it; OpenAI images API accepts
`size`). `generate_image` passes it through. `images_url()` and the b64 decode path are unchanged.
`run_image_skill` sources `prompt` from the node and `size` from `load_image_config`.

## Backend: settings API

`GET /api/system/settings` response gains:
- `connections`: array of `{ id, label, baseUrl, model, timeoutSeconds, isActive, apiKeyConfigured }`
  (raw `api_key` never returned — redacted to a boolean flag, matching the current
  `providerApiKeyConfigured` convention).
- `image`: `{ baseUrl, model, size, timeoutSeconds, apiKeyConfigured }`.
- `embedding`: `{ enabled, baseUrl, model, timeoutSeconds, apiKeyConfigured }`.
- `search`: `{ provider, providers: { tavily:{apiKeyConfigured,baseUrl}, serpapi:{…}, searxng:{…},
  ollama:{…} } }` (keys redacted to flags).
- `defaults`: `{ language, defaultQueryLimit }`.

New connection endpoints (operator auth + CSRF, same guards as current `update_settings`):
- `POST   /api/system/provider-connections` — create.
- `PATCH  /api/system/provider-connections/:id` — update (blank `apiKey` keeps the stored key; a
  `clearApiKey` flag clears it — mirrors current `clear_provider_api_key`).
- `DELETE /api/system/provider-connections/:id` — delete (refuse deleting the last active with a clear
  error, or auto-activate the next by `sort_order`; **decision: auto-activate next**).
- `POST   /api/system/provider-connections/:id/activate` — set active (clears others).

`PATCH /api/system/settings` extended to accept `image`, `embedding`, `search`, and `defaults` blocks,
each honoring blank-keeps-key / `clear*ApiKey` flags.

## Frontend: settings UI (shadcn, redesigned)

Single Settings page with a **left sub-nav** (category rail) mirroring upstream. Sections:

```
Settings
┌───────────────┬────────────────────────────────────────────┐
│ ▸ LLM         │  LLM Connections            [+ Add]         │
│   Embedding   │  ┌──────────────────────────────────────┐   │
│   Image       │  │ ● OpenAI      gpt-4o   [Active] [edit] │   │
│   Web search  │  │ ○ Local vLLM  qwen2   [Configured]     │   │
│   Defaults    │  └──────────────────────────────────────┘   │
│               │   (expand row → label / baseURL / key /     │
│               │    model / timeout;  Activate · Delete)     │
└───────────────┴────────────────────────────────────────────┘
```

- **LLM section:** the connection list — collapsible rows (upstream pattern). Each row: active radio,
  label + model summary, "Active"/"Configured" badge, expand → form (`label`, `baseUrl`, `apiKey`
  password with keep-on-blank, `model`, `timeoutSeconds`), Activate / Delete. `[+ Add]` appends a
  draft row.
- **Embedding section:** enable toggle + form (`baseUrl`, `apiKey`, `model`, `timeoutSeconds`).
- **Image section:** form (`baseUrl`, `apiKey`, `model`, `size` select `256|512|1024|1024x1792|…`,
  `timeoutSeconds`).
- **Web search section:** keep the existing four-provider selector, **Tavily default**; per-provider
  fields persist via `search_provider_configs` so switching retains keys. Retain the existing "Test
  search" button.
- **Defaults section:** `language`, `defaultQueryLimit`.

All sections compose shadcn primitives (Card, Input, Select, Button, Badge, Switch) — no hand-rolled
components (per memory). Data via a redesigned `features/settings/queries.ts` (query + mutations for
settings blocks and connection CRUD). The Zod schema in `features/shared/api.ts` is updated to the new
response shape.

## Frontend: canvas node wiring

- **Model tag / `__model` injection** in `canvas-board.tsx` becomes **capability-aware**: the
  `ai_analyze` adapter shows the **active connection's** model; the `ai_image` adapter shows the
  **image** model. Board reads both from settings (active connection model + image model) and injects
  the right one per node type.
- **Image node:** no frontend behavior change beyond the model tag; the real fix is server-side
  (`run_image_skill` now uses the image config). Verify generation produces an asset + preview.
- **Search node:** already functional; verify it resolves through the redesigned search config and
  renders markdown.

## Testing

Backend (Rust):
- Migration seeds a "Default" active connection and the embedding/image/search blocks from legacy
  columns (integration test against a migrated DB, following existing migration-test patterns).
- `load_active_connection`: returns the active row; errors when none active.
- `load_image_config`: validates base_url+model; feeds `size` into `ProviderImageRequest`.
- `provider_connections` service: activating clears others; deleting the active auto-activates next.
- `generate_image` includes `size` in the request body (extend `image_tests`).

Frontend (Vitest):
- Settings queries: GET maps the new shape; connection CRUD mutations call the right endpoints;
  blank-apiKey keeps key (no `clearApiKey`), explicit clear sends the flag.
- LLM section: lists connections, add appends a draft, activate calls activate endpoint, delete
  confirms then calls delete.
- Image/Embedding/Web-search/Defaults sections render and submit their blocks.
- Canvas board: `ai_image` model tag shows the **image** model, `ai_analyze` shows the **active
  connection** model.

## Files

Backend — create: `migrations/0015_multi_provider.sql`; a `provider_connections` store/service module.
Backend — modify: `settings/routes.rs` (GET shape + connection endpoints + extended PATCH),
`query.rs` / capability loaders, `canvas/routes.rs` (`build_provider` → active connection;
`run_image_skill` → image config + size), `chat/routes.rs`, `retrieval/service.rs`
(`load_embedding_config`), `web_search/config.rs` (`load_web_search_config`),
`providers/openai_compatible.rs` + `providers/types.rs` (`ProviderImageRequest.size`).

Frontend — modify: `features/settings/page.tsx` (sub-nav + sections), `features/settings/queries.ts`,
`features/shared/api.ts` (settings schema), `features/canvas/canvas-board.tsx` (capability-aware
`__model`). New per-section components under `features/settings/` as the page grows.

## Phasing (for the implementation plan)

1. **Schema + loaders + usage sites** — migration, `provider_connections` store, capability loaders,
   repoint the five instantiation sites, `ProviderImageRequest.size`. Backend tests green. (This alone
   fixes image generation.)
2. **Settings API** — GET shape, connection CRUD, extended PATCH.
3. **Settings UI** — sub-nav + five sections, redesigned queries/schema.
4. **Node wiring + verification** — capability-aware model tag; end-to-end verify image + search nodes.

## Out of scope

- Removing the legacy flat `provider_*` / flat `search_*` columns (kept for migration safety).
- Non-OpenAI-compatible image protocols (e.g. Stability's native API).
- Per-project / per-user provider selection (config stays global/system-wide).
- Encrypting API keys at rest (unchanged from current plaintext storage).

## Verification

1. `cargo test -p knowledge-server` — migration seed + loader + image-size tests pass.
2. `cd apps/admin && npx vitest run && npx tsc --noEmit` — settings + canvas tests pass, types clean.
3. `docker compose build admin backend && docker compose up -d`, hard-refresh `http://localhost:4173`:
   - Settings shows migrated "Default" LLM connection active; add a second connection, activate it,
     confirm chat/analyze use it; delete a connection.
   - Configure the Image section (image model + size); the `ai_image` node generates a real image and
     shows the preview; its model tag shows the image model.
   - Web search shows Tavily active with its key retained; the search node returns markdown results.
   - Configure the Embedding block; retrieval uses the embedding model.
