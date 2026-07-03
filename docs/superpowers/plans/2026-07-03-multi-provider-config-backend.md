# Multi-Provider Configuration — Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give each AI capability (chat/analyze, image, embedding, web search) its own configuration instead of one shared `provider_*` row, add a multi-connection LLM preset list, and fix canvas image generation by pointing it at a dedicated image model.

**Architecture:** A new `provider_connections` table holds the LLM preset list (exactly one active). `system_settings` gains independent embedding / image blocks and a `search_provider_configs` JSONB. Focused capability loaders (`load_active_connection`, `load_image_config`, updated `load_embedding_config` / `load_web_search_config`) replace the five sites that currently call `build_provider(&QuerySettings)`. Every DB loader is split into a thin SQL fetch plus a **pure resolution function** so behavior is unit-testable without a live Postgres (this repo has no DB test harness — all tests are pure unit tests). The settings API gains connection CRUD and an extended `GET`/`PATCH` shape.

**Tech Stack:** Rust, axum, sqlx (Postgres, `sqlx::migrate!`), serde/serde_json, tokio. Migrations in `crates/knowledge-server/migrations`, run via `MIGRATOR` (`db/migrate.rs`).

**Scope:** Spec phases 1–2 (schema + loaders + usage sites + image fix, then settings API). The frontend (settings UI + node model wiring) is a separate plan. This plan alone ships the image-generation fix.

**Spec:** `docs/superpowers/specs/2026-07-03-multi-provider-config-design.md`

---

## Conventions for every task

- Run backend tests with: `cargo test -p knowledge-server` (from repo root `E:\Projects\Js\knowledge`).
- Type-check quickly with: `cargo check -p knowledge-server`.
- `ApiError` constructors used below already exist: `ApiError::bad_request`, `::internal`, `::not_found`, `::unauthorized`, and `From<sqlx::Error>` (`ApiError::from`).
- `OpenAiCompatibleProvider::new(base_url: String, api_key: String, model: String, timeout_seconds: i64)` — confirmed signature.
- Timestamps: mirror `canvas/store.rs` — `OffsetDateTime::now_utc().format(&Rfc3339)`.
- IDs: TEXT primary keys holding a `Uuid::new_v4().to_string()`, consistent with `canvases`.
- Commit after each task with the message shown in its final step.

---

## Task 1: Migration `0015_multi_provider.sql`

Creates the `provider_connections` table, seeds a "Default" active connection from the legacy `provider_*` columns, and adds the embedding / image / search-JSONB columns to `system_settings` (seeded from legacy columns). Legacy columns are left in place (out of scope to drop).

**Files:**
- Create: `crates/knowledge-server/migrations/0015_multi_provider.sql`
- Test: `crates/knowledge-server/src/db/migrate.rs` (add an embed-presence test)

- [ ] **Step 1: Write the failing test** in `crates/knowledge-server/src/db/migrate.rs` (append after the existing content):

```rust
#[cfg(test)]
mod tests {
    use super::MIGRATOR;

    #[test]
    fn migrator_embeds_multi_provider_migration() {
        // 0015 must be compiled into the embedded migration set.
        assert!(
            MIGRATOR.iter().any(|m| m.version == 15),
            "migration 0015 not embedded"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server migrator_embeds_multi_provider_migration`
Expected: FAIL — no migration with version 15 exists yet.

- [ ] **Step 3: Create the migration** `crates/knowledge-server/migrations/0015_multi_provider.sql`:

```sql
-- Multi-provider configuration: a preset connection list for the LLM
-- capability, plus independent embedding / image blocks and a per-provider
-- web-search config map on the singleton system_settings row.

-- 1. LLM preset connection list. Exactly one row is active (enforced in the
--    service layer via a single UPDATE that sets is_active = (id = $target)).
CREATE TABLE provider_connections (
    id TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    base_url TEXT NOT NULL,
    api_key TEXT,
    model TEXT NOT NULL,
    timeout_seconds BIGINT,
    is_active BOOLEAN NOT NULL DEFAULT false,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_provider_connections_sort ON provider_connections (sort_order);

-- 2. Seed a "Default" active connection from the legacy flat provider_* columns,
--    but only when a base_url and model are actually configured. gen_random_uuid
--    is available via pgcrypto in this database; cast to text for the TEXT PK.
INSERT INTO provider_connections
    (id, label, base_url, api_key, model, timeout_seconds, is_active, sort_order, created_at, updated_at)
SELECT
    gen_random_uuid()::text,
    'Default',
    provider_base_url,
    provider_api_key,
    provider_model,
    provider_timeout_seconds,
    true,
    0,
    to_char(now() AT TIME ZONE 'utc', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    to_char(now() AT TIME ZONE 'utc', 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
FROM system_settings
WHERE id = 1
  AND provider_base_url IS NOT NULL AND btrim(provider_base_url) <> ''
  AND provider_model    IS NOT NULL AND btrim(provider_model)    <> '';

-- 3. Independent embedding block.
ALTER TABLE system_settings ADD COLUMN embedding_enabled BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE system_settings ADD COLUMN embedding_base_url TEXT;
ALTER TABLE system_settings ADD COLUMN embedding_api_key TEXT;
ALTER TABLE system_settings ADD COLUMN embedding_model TEXT;
ALTER TABLE system_settings ADD COLUMN embedding_timeout_seconds BIGINT;

-- 4. Seed embedding from the legacy provider_* columns; enabled iff an
--    embedding model was configured.
UPDATE system_settings
SET embedding_base_url = provider_base_url,
    embedding_api_key = provider_api_key,
    embedding_model = provider_embedding_model,
    embedding_timeout_seconds = provider_timeout_seconds,
    embedding_enabled = (provider_embedding_model IS NOT NULL AND btrim(provider_embedding_model) <> '')
WHERE id = 1;

-- 5. Net-new image-generation block (OpenAI-compatible /v1/images/generations).
--    image_model is intentionally left NULL: the admin must choose an image
--    model (the legacy provider_model is a text/chat model and is the bug we fix).
ALTER TABLE system_settings ADD COLUMN image_base_url TEXT;
ALTER TABLE system_settings ADD COLUMN image_api_key TEXT;
ALTER TABLE system_settings ADD COLUMN image_model TEXT;
ALTER TABLE system_settings ADD COLUMN image_size TEXT NOT NULL DEFAULT '1024x1024';
ALTER TABLE system_settings ADD COLUMN image_timeout_seconds BIGINT;

UPDATE system_settings
SET image_base_url = provider_base_url,
    image_api_key = provider_api_key,
    image_timeout_seconds = provider_timeout_seconds
WHERE id = 1;

-- 6. Per-provider web-search config map so switching the active provider keeps
--    each provider's key/fields. Seeded from the flat search_* columns; the
--    active provider's key is placed under its own block, other providers keep
--    only their non-secret fields. jsonb_strip_nulls drops absent keys.
ALTER TABLE system_settings ADD COLUMN search_provider_configs JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE system_settings
SET search_provider_configs = jsonb_strip_nulls(jsonb_build_object(
    'tavily',  jsonb_build_object(
        'apiKey',  CASE WHEN search_provider = 'tavily'  THEN search_api_key END,
        'baseUrl', tavily_base_url),
    'serpapi', jsonb_build_object(
        'apiKey',  CASE WHEN search_provider = 'serpapi' THEN search_api_key END,
        'engine',  serpapi_engine,
        'baseUrl', serpapi_base_url),
    'searxng', jsonb_build_object(
        'url',        searxng_url,
        'categories', searxng_categories),
    'ollama',  jsonb_build_object(
        'apiKey', CASE WHEN search_provider = 'ollama' THEN search_api_key END,
        'url',    ollama_search_url)
))
WHERE id = 1;
```

- [ ] **Step 4: Run the embed test to verify it passes**

Run: `cargo test -p knowledge-server migrator_embeds_multi_provider_migration`
Expected: PASS.

- [ ] **Step 5: Verify the migration applies against a real database.** With the docker Postgres running:

Run: `cd crates/knowledge-server && cargo run --bin knowledge-server` (boots, runs `MIGRATOR.run`, exits after Ctrl-C once "listening" appears) — OR `docker compose up -d backend` then check logs `docker compose logs backend | grep -i migrat`.
Expected: no migration error; `provider_connections` exists. If a legacy provider was configured, one active "Default" row is present.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/migrations/0015_multi_provider.sql crates/knowledge-server/src/db/migrate.rs
git commit -m "feat(db): add provider_connections + capability config columns (0015)"
```

---

## Task 2: Add `size` to the image request

`ProviderImageRequest` and the `ImageGenerationRequest` wire body gain a `size` field; `generate_image` forwards it. OpenAI's images API accepts `size` unconditionally, so it is always serialized.

**Files:**
- Modify: `crates/knowledge-server/src/providers/types.rs`
- Modify: `crates/knowledge-server/src/providers/openai_compatible.rs`

- [ ] **Step 1: Update the failing test** in `crates/knowledge-server/src/providers/types.rs` — replace `provider_image_request_holds_prompt`:

```rust
    #[test]
    fn provider_image_request_holds_prompt_and_size() {
        let req = ProviderImageRequest {
            prompt: "a red fox".to_string(),
            size: "512x512".to_string(),
        };
        assert_eq!(req.prompt, "a red fox");
        assert_eq!(req.size, "512x512");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server provider_image_request_holds_prompt_and_size`
Expected: FAIL — `ProviderImageRequest` has no `size` field (compile error).

- [ ] **Step 3: Add the field** in `crates/knowledge-server/src/providers/types.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ProviderImageRequest {
    pub prompt: String,
    pub size: String,
}
```

- [ ] **Step 4: Thread `size` through the wire body** in `crates/knowledge-server/src/providers/openai_compatible.rs`. Update the struct (near line 604):

```rust
#[derive(Debug, Serialize)]
struct ImageGenerationRequest {
    model: String,
    prompt: String,
    size: String,
    n: u8,
    response_format: String,
}
```

And update the body construction inside `generate_image` (near line 337):

```rust
        let body = ImageGenerationRequest {
            model: self.model.clone(),
            prompt: request.prompt,
            size: request.size,
            n: 1,
            response_format: "b64_json".to_string(),
        };
```

- [ ] **Step 5: Fix the existing image test call site** in `crates/knowledge-server/src/providers/openai_compatible.rs` (`generate_image_decodes_b64_payload`, near line 768):

```rust
    let result = provider
      .generate_image(ProviderImageRequest { prompt: "x".to_string(), size: "1024x1024".to_string() })
      .await
      .expect("image generated");
```

- [ ] **Step 6: Add a serialization test** proving `size` reaches the JSON body. Append inside the `image_tests` module in `crates/knowledge-server/src/providers/openai_compatible.rs`:

```rust
  #[test]
  fn image_generation_request_serializes_size() {
    let body = ImageGenerationRequest {
      model: "gpt-image-1".to_string(),
      prompt: "a fox".to_string(),
      size: "512x512".to_string(),
      n: 1,
      response_format: "b64_json".to_string(),
    };
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(json["size"], "512x512");
    assert_eq!(json["model"], "gpt-image-1");
  }
```

> Note: `ImageGenerationRequest` is a private struct in this module, so the test must live in the same file's test module (`image_tests`). Confirm the module name by searching for `mod image_tests` / the existing `generate_image_decodes_b64_payload`; add this test alongside it.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p knowledge-server image`
Expected: PASS — `provider_image_request_holds_prompt_and_size`, `image_generation_request_serializes_size`, `generate_image_decodes_b64_payload`.

- [ ] **Step 8: Commit**

```bash
git add crates/knowledge-server/src/providers/types.rs crates/knowledge-server/src/providers/openai_compatible.rs
git commit -m "feat(providers): add configurable size to image generation requests"
```

---

## Task 3: `provider_connections` store + `load_active_connection`

New module holding the connection row type, a **pure** `resolve_active`, CRUD/activate/delete-with-auto-activate against the pool, and `load_active_connection` (the chat/analyze loader). Also a helper to build an `OpenAiCompatibleProvider` from the active connection.

**Files:**
- Create: `crates/knowledge-server/src/providers/connections.rs`
- Modify: `crates/knowledge-server/src/providers/mod.rs`

- [ ] **Step 1: Register the module** in `crates/knowledge-server/src/providers/mod.rs` (add `mod connections;` and re-export). Result:

```rust
mod connections;
mod openai_compatible;
mod types;

pub use connections::{
    activate_connection, create_connection, delete_connection, list_connections,
    load_active_connection, resolve_active, update_connection, ActiveConnection, NewConnection,
    ProviderConnection, UpdateConnection,
};
pub use openai_compatible::OpenAiCompatibleProvider;
pub use types::{
    ProviderAnswer, ProviderChatMessage, ProviderChatStreamRequest, ProviderCitation,
    ProviderContentBlock, ProviderEmbeddingRequest, ProviderError, ProviderImageRequest,
    ProviderImageResult, ProviderMultimodalRequest, ProviderQueryRequest, ProviderTextRequest,
    ProviderTextResponse, ProviderUsage,
};
```

- [ ] **Step 2: Write the failing test** — create `crates/knowledge-server/src/providers/connections.rs` with only the types + pure fn + a test:

```rust
use serde::Serialize;
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::providers::OpenAiCompatibleProvider;

/// A row of the LLM preset connection list.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProviderConnection {
    pub id: String,
    pub label: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout_seconds: Option<i64>,
    pub is_active: bool,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// The resolved active connection used to build a chat/analyze provider.
#[derive(Debug, Clone)]
pub struct ActiveConnection {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_seconds: i64,
}

impl ActiveConnection {
    /// Build an OpenAI-compatible provider from this connection.
    pub fn provider(&self) -> OpenAiCompatibleProvider {
        OpenAiCompatibleProvider::new(
            self.base_url.clone(),
            self.api_key.clone(),
            self.model.clone(),
            self.timeout_seconds,
        )
    }
}

impl From<&ProviderConnection> for ActiveConnection {
    fn from(c: &ProviderConnection) -> Self {
        Self {
            base_url: c.base_url.clone(),
            api_key: c.api_key.clone().unwrap_or_default(),
            model: c.model.clone(),
            timeout_seconds: c.timeout_seconds.unwrap_or(30),
        }
    }
}

/// Pure: pick the active connection from a fetched list. The service layer keeps
/// exactly one active, but if the invariant is ever violated we deterministically
/// choose the lowest sort_order active row.
pub fn resolve_active(rows: &[ProviderConnection]) -> Option<&ProviderConnection> {
    rows.iter()
        .filter(|c| c.is_active)
        .min_by_key(|c| c.sort_order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(id: &str, active: bool, sort: i32) -> ProviderConnection {
        ProviderConnection {
            id: id.into(),
            label: id.into(),
            base_url: "https://api.example.com".into(),
            api_key: Some("k".into()),
            model: "gpt-4o".into(),
            timeout_seconds: None,
            is_active: active,
            sort_order: sort,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn resolve_active_returns_the_active_row() {
        let rows = vec![conn("a", false, 0), conn("b", true, 1)];
        assert_eq!(resolve_active(&rows).unwrap().id, "b");
    }

    #[test]
    fn resolve_active_none_when_no_active() {
        let rows = vec![conn("a", false, 0)];
        assert!(resolve_active(&rows).is_none());
    }

    #[test]
    fn active_connection_defaults_timeout_and_key() {
        let c = ProviderConnection { api_key: None, timeout_seconds: None, ..conn("a", true, 0) };
        let active = ActiveConnection::from(&c);
        assert_eq!(active.timeout_seconds, 30);
        assert_eq!(active.api_key, "");
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p knowledge-server -- providers::connections`
Expected: FAIL to compile (mod not yet complete / re-exports reference not-yet-defined store fns). This is expected; proceed to add the store fns so the module compiles.

- [ ] **Step 4: Add the store + loader functions** to `crates/knowledge-server/src/providers/connections.rs` (below the pure code, above `#[cfg(test)]`):

```rust
const SELECT_COLUMNS: &str = "id, label, base_url, api_key, model, timeout_seconds, is_active, sort_order, created_at, updated_at";

fn now() -> Result<String, ApiError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| ApiError::internal("failed to format timestamp"))
}

/// Fields accepted when creating a connection.
#[derive(Debug, Clone, Serialize)]
pub struct NewConnection {
    pub label: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout_seconds: Option<i64>,
}

/// Fields accepted when updating a connection. `api_key` = None leaves the stored
/// key; `clear_api_key = true` clears it (mirrors clear_provider_api_key).
#[derive(Debug, Clone)]
pub struct UpdateConnection {
    pub label: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub clear_api_key: bool,
    pub model: String,
    pub timeout_seconds: Option<i64>,
}

pub async fn list_connections(pool: &PgPool) -> Result<Vec<ProviderConnection>, ApiError> {
    sqlx::query_as::<_, ProviderConnection>(&format!(
        "SELECT {SELECT_COLUMNS} FROM provider_connections ORDER BY sort_order, created_at"
    ))
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)
}

/// Load the active connection, erroring the same way the old build_provider did
/// when nothing usable is configured.
pub async fn load_active_connection(state: &AppState) -> Result<ActiveConnection, ApiError> {
    let rows = list_connections(&state.pool).await?;
    resolve_active(&rows)
        .map(ActiveConnection::from)
        .ok_or_else(|| ApiError::bad_request("no active provider connection is configured"))
}

pub async fn create_connection(
    pool: &PgPool,
    input: &NewConnection,
) -> Result<ProviderConnection, ApiError> {
    let id = Uuid::new_v4().to_string();
    let ts = now()?;
    // First connection becomes active; new ones append after the current max.
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_connections")
        .fetch_one(pool)
        .await
        .map_err(ApiError::from)?;
    let is_active = existing == 0;
    let next_sort: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM provider_connections",
    )
    .fetch_one(pool)
    .await
    .map_err(ApiError::from)?;

    sqlx::query(&format!(
        "INSERT INTO provider_connections
           (id, label, base_url, api_key, model, timeout_seconds, is_active, sort_order, created_at, updated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$9)"
    ))
    .bind(&id)
    .bind(&input.label)
    .bind(&input.base_url)
    .bind(input.api_key.as_deref())
    .bind(&input.model)
    .bind(input.timeout_seconds)
    .bind(is_active)
    .bind(next_sort)
    .bind(&ts)
    .execute(pool)
    .await
    .map_err(ApiError::from)?;

    get_connection(pool, &id)
        .await?
        .ok_or_else(|| ApiError::internal("connection vanished after insert"))
}

async fn get_connection(pool: &PgPool, id: &str) -> Result<Option<ProviderConnection>, ApiError> {
    sqlx::query_as::<_, ProviderConnection>(&format!(
        "SELECT {SELECT_COLUMNS} FROM provider_connections WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::from)
}

pub async fn update_connection(
    pool: &PgPool,
    id: &str,
    input: &UpdateConnection,
) -> Result<ProviderConnection, ApiError> {
    let ts = now()?;
    let affected = sqlx::query(
        "UPDATE provider_connections
         SET label = $1,
             base_url = $2,
             api_key = COALESCE(NULLIF($3, ''), CASE WHEN $4 THEN NULL ELSE api_key END),
             model = $5,
             timeout_seconds = $6,
             updated_at = $7
         WHERE id = $8",
    )
    .bind(&input.label)
    .bind(&input.base_url)
    .bind(input.api_key.as_deref())
    .bind(input.clear_api_key)
    .bind(&input.model)
    .bind(input.timeout_seconds)
    .bind(&ts)
    .bind(id)
    .execute(pool)
    .await
    .map_err(ApiError::from)?
    .rows_affected();

    if affected == 0 {
        return Err(ApiError::not_found("connection not found"));
    }
    get_connection(pool, id)
        .await?
        .ok_or_else(|| ApiError::not_found("connection not found"))
}

/// Activate one connection, clearing is_active on all others in a single UPDATE
/// so exactly one row stays active.
pub async fn activate_connection(pool: &PgPool, id: &str) -> Result<(), ApiError> {
    let affected = sqlx::query("UPDATE provider_connections SET is_active = (id = $1)")
        .bind(id)
        .execute(pool)
        .await
        .map_err(ApiError::from)?
        .rows_affected();
    if affected == 0 {
        return Err(ApiError::not_found("connection not found"));
    }
    Ok(())
}

/// Delete a connection. If it was the active one, auto-activate the next by
/// sort_order so the list is never left without an active connection.
pub async fn delete_connection(pool: &PgPool, id: &str) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(ApiError::from)?;

    let was_active: Option<bool> =
        sqlx::query_scalar("SELECT is_active FROM provider_connections WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(ApiError::from)?;
    let Some(was_active) = was_active else {
        return Err(ApiError::not_found("connection not found"));
    };

    sqlx::query("DELETE FROM provider_connections WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;

    if was_active {
        // Promote the lowest-sort_order survivor, if any remain.
        sqlx::query(
            "UPDATE provider_connections SET is_active = true
             WHERE id = (SELECT id FROM provider_connections ORDER BY sort_order, created_at LIMIT 1)",
        )
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;
    }

    tx.commit().await.map_err(ApiError::from)?;
    Ok(())
}
```

- [ ] **Step 5: Run the module tests to verify they pass**

Run: `cargo test -p knowledge-server -- providers::connections`
Expected: PASS — `resolve_active_*`, `active_connection_defaults_*`. (The store fns compile; they are exercised end-to-end in the docker verification.)

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-server/src/providers/connections.rs crates/knowledge-server/src/providers/mod.rs
git commit -m "feat(providers): provider_connections store + load_active_connection loader"
```

---

## Task 4: `load_image_config` (dedicated image loader)

Reads the `image_*` block and validates it. A **pure** `image_config_from_row` does the validation so it is unit-testable.

**Files:**
- Create: `crates/knowledge-server/src/providers/image_config.rs`
- Modify: `crates/knowledge-server/src/providers/mod.rs`

- [ ] **Step 1: Register the module** in `crates/knowledge-server/src/providers/mod.rs` — add `mod image_config;` and extend the re-export:

```rust
mod connections;
mod image_config;
mod openai_compatible;
mod types;

pub use image_config::{image_config_from_row, load_image_config, ImageConfig};
```

(Keep the existing `pub use connections::…`, `pub use openai_compatible::…`, and `pub use types::…` lines.)

- [ ] **Step 2: Write the failing test** — create `crates/knowledge-server/src/providers/image_config.rs`:

```rust
use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::providers::OpenAiCompatibleProvider;

/// Resolved image-generation configuration (OpenAI-compatible endpoint).
#[derive(Debug, Clone)]
pub struct ImageConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub size: String,
    pub timeout_seconds: i64,
}

impl ImageConfig {
    pub fn provider(&self) -> OpenAiCompatibleProvider {
        OpenAiCompatibleProvider::new(
            self.base_url.clone(),
            self.api_key.clone(),
            self.model.clone(),
            self.timeout_seconds,
        )
    }
}

/// Pure: validate the raw image columns. Requires base_url + model; defaults the
/// size and timeout. Returns a bad_request error mirroring the old provider-
/// incomplete failure so callers surface a clear "configure the image provider".
pub fn image_config_from_row(
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    size: Option<String>,
    timeout_seconds: Option<i64>,
) -> Result<ImageConfig, ApiError> {
    let base_url = base_url.unwrap_or_default();
    let model = model.unwrap_or_default();
    if base_url.trim().is_empty() || model.trim().is_empty() {
        return Err(ApiError::bad_request("image provider is not configured"));
    }
    let size = size.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "1024x1024".to_string());
    Ok(ImageConfig {
        base_url,
        api_key: api_key.unwrap_or_default(),
        model,
        size,
        timeout_seconds: timeout_seconds.unwrap_or(60),
    })
}

pub async fn load_image_config(state: &AppState) -> Result<ImageConfig, ApiError> {
    let (base_url, api_key, model, size, timeout_seconds) =
        sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>)>(
            "SELECT image_base_url, image_api_key, image_model, image_size, image_timeout_seconds
             FROM system_settings WHERE id = 1",
        )
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::from)?;
    image_config_from_row(base_url, api_key, model, size, timeout_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_row_requires_base_url_and_model() {
        assert!(image_config_from_row(None, None, Some("m".into()), None, None).is_err());
        assert!(image_config_from_row(Some("u".into()), None, None, None, None).is_err());
        assert!(image_config_from_row(Some("".into()), None, Some("m".into()), None, None).is_err());
    }

    #[test]
    fn from_row_defaults_size_and_timeout() {
        let cfg = image_config_from_row(
            Some("https://api.example.com".into()),
            None,
            Some("gpt-image-1".into()),
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.size, "1024x1024");
        assert_eq!(cfg.timeout_seconds, 60);
        assert_eq!(cfg.api_key, "");
    }

    #[test]
    fn from_row_keeps_explicit_size() {
        let cfg = image_config_from_row(
            Some("https://api.example.com".into()),
            Some("k".into()),
            Some("gpt-image-1".into()),
            Some("512x512".into()),
            Some(90),
        )
        .unwrap();
        assert_eq!(cfg.size, "512x512");
        assert_eq!(cfg.timeout_seconds, 90);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail then pass.** The module is new, so first run is the compile+test:

Run: `cargo test -p knowledge-server -- providers::image_config`
Expected: PASS (`from_row_*` three tests). If it fails to compile, fix the `mod`/re-export from Step 1 before continuing.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/providers/image_config.rs crates/knowledge-server/src/providers/mod.rs
git commit -m "feat(providers): dedicated load_image_config with validation + size"
```

---

## Task 5: Repoint the LLM + image usage sites

Replace `build_provider(&QuerySettings)` in canvas and the inline provider construction in chat with `load_active_connection`; repoint `run_image_skill` (and the ai_image pre-stream validation) to `load_image_config` + `size`. `load_query_settings` is still used for `language` / `default_query_limit`.

**Files:**
- Modify: `crates/knowledge-server/src/canvas/routes.rs`
- Modify: `crates/knowledge-server/src/chat/routes.rs`

- [ ] **Step 1: Update canvas imports** in `crates/knowledge-server/src/canvas/routes.rs` (near lines 19-22):

```rust
use crate::providers::{
    load_active_connection, load_image_config, ProviderChatMessage, ProviderChatStreamRequest,
    ProviderImageRequest,
};
use crate::query::load_query_settings;
```

(Remove `OpenAiCompatibleProvider` and `QuerySettings` from the import list — they are no longer referenced once `build_provider` is deleted.)

- [ ] **Step 2: Delete the `build_provider` helper** in `crates/knowledge-server/src/canvas/routes.rs` (the whole `fn build_provider(...)` at ~228-241, plus its doc comment at ~226-227).

- [ ] **Step 3: Repoint the ai_image pre-stream validation** in `run_node_handler` (~296-298). Replace:

```rust
        let settings = load_query_settings(&state).await?;
        build_provider(&settings)?; // validate config before streaming
```

with:

```rust
        load_image_config(&state).await?; // validate image config before streaming
```

- [ ] **Step 4: Repoint the analyze provider** in `run_node_handler` (~363-364). Replace:

```rust
    let settings = load_query_settings(&state).await?;
    let provider = build_provider(&settings)?;
```

with:

```rust
    let settings = load_query_settings(&state).await?;
    let provider = load_active_connection(&state).await?.provider();
```

- [ ] **Step 5: Repoint `run_image_skill`** in `crates/knowledge-server/src/canvas/routes.rs` (~610-629). Replace its first three statements:

```rust
    let settings = load_query_settings(state).await.map_err(|error| error.to_string())?;
    let provider = build_provider(&settings).map_err(|error| error.to_string())?;
    let result = provider
        .generate_image(ProviderImageRequest { prompt: prompt.to_string() })
        .await
        .map_err(|error| error.message().to_string())?;
```

with:

```rust
    let config = load_image_config(state).await.map_err(|error| error.to_string())?;
    let result = config
        .provider()
        .generate_image(ProviderImageRequest { prompt: prompt.to_string(), size: config.size.clone() })
        .await
        .map_err(|error| error.message().to_string())?;
```

- [ ] **Step 6: Repoint the canvas chat Plain branch** in `chat_handler` (~530-544). Replace:

```rust
                let settings = match load_query_settings(&stream_state).await {
                    Ok(settings) => settings,
                    Err(error) => {
                        yield Ok(sse_error(&error.to_string()));
                        return;
                    }
                };
                let provider = match build_provider(&settings) {
                    Ok(provider) => provider,
                    Err(error) => {
                        yield Ok(sse_error(&error.to_string()));
                        return;
                    }
                };
```

with:

```rust
                let settings = match load_query_settings(&stream_state).await {
                    Ok(settings) => settings,
                    Err(error) => {
                        yield Ok(sse_error(&error.to_string()));
                        return;
                    }
                };
                let provider = match load_active_connection(&stream_state).await {
                    Ok(connection) => connection.provider(),
                    Err(error) => {
                        yield Ok(sse_error(&error.to_string()));
                        return;
                    }
                };
```

- [ ] **Step 7: Repoint chat/routes.rs** in `crates/knowledge-server/src/chat/routes.rs`. Update the import (line 22-23):

```rust
use crate::providers::{load_active_connection, ProviderChatMessage, ProviderChatStreamRequest};
use crate::query::load_query_settings;
```

(Drop `OpenAiCompatibleProvider` from that import.) Then replace the inline validation + construction (~157-212). Replace:

```rust
    let settings = load_query_settings(&state).await?;
    if settings.provider_mode != "openai-compatible"
        || settings
            .provider_base_url
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        || settings
            .provider_model
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return Err(ApiError::bad_request("provider configuration is incomplete"));
    }
```

with:

```rust
    let settings = load_query_settings(&state).await?;
    let connection = load_active_connection(&state).await?;
```

and replace the provider construction (~207-212):

```rust
    let provider = OpenAiCompatibleProvider::new(
        settings.provider_base_url.clone().unwrap_or_default(),
        settings.provider_api_key.clone().unwrap_or_default(),
        settings.provider_model.clone().unwrap_or_default(),
        settings.provider_timeout_seconds.unwrap_or(30),
    );
```

with:

```rust
    let provider = connection.provider();
```

- [ ] **Step 8: Verify everything compiles and existing tests pass**

Run: `cargo test -p knowledge-server`
Expected: PASS — all existing canvas/chat unit tests still green; no unused-import or missing-symbol errors. (Fix any leftover reference to `build_provider` / `QuerySettings` / `OpenAiCompatibleProvider` the compiler flags.)

- [ ] **Step 9: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs crates/knowledge-server/src/chat/routes.rs
git commit -m "feat(canvas,chat): route LLM through active connection, image through image config"
```

---

## Task 6: Repoint `load_embedding_config` to the embedding block

The embedding loader must read the new `embedding_*` columns and honor `embedding_enabled` instead of `provider_mode`. Extract a **pure** `embedding_config_from_row`.

**Files:**
- Modify: `crates/knowledge-server/src/retrieval/service.rs`

- [ ] **Step 1: Write the failing test** — append to the test module of `crates/knowledge-server/src/retrieval/service.rs` (if no `#[cfg(test)] mod tests` exists in this file, add one at the end):

```rust
#[cfg(test)]
mod embedding_config_tests {
    use super::embedding_config_from_row;

    #[test]
    fn disabled_returns_none() {
        assert!(embedding_config_from_row(false, Some("u".into()), None, Some("m".into()), None).is_none());
    }

    #[test]
    fn enabled_but_incomplete_returns_none() {
        assert!(embedding_config_from_row(true, None, None, Some("m".into()), None).is_none());
        assert!(embedding_config_from_row(true, Some("u".into()), None, None, None).is_none());
    }

    #[test]
    fn enabled_and_complete_returns_config() {
        let cfg = embedding_config_from_row(
            true,
            Some("https://api.example.com".into()),
            Some("k".into()),
            Some("text-embedding-3-small".into()),
            Some(45),
        )
        .expect("config");
        assert_eq!(cfg.model, "text-embedding-3-small");
        assert_eq!(cfg.timeout_seconds, 45);
        assert_eq!(cfg.api_key, "k");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server -- embedding_config_tests`
Expected: FAIL — `embedding_config_from_row` does not exist (compile error).

- [ ] **Step 3: Add the pure helper and rewrite the loader** in `crates/knowledge-server/src/retrieval/service.rs`. Replace the entire `load_embedding_config` fn (lines ~42-79) with:

```rust
/// Pure: build an embedding config from the raw embedding_* columns. Returns
/// None when disabled or incomplete (matching the loader's Option contract).
pub fn embedding_config_from_row(
    enabled: bool,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    timeout_seconds: Option<i64>,
) -> Option<EmbeddingConfig> {
    if !enabled {
        return None;
    }
    let base_url = base_url.unwrap_or_default();
    let model = model.unwrap_or_default();
    if base_url.trim().is_empty() || model.trim().is_empty() {
        return None;
    }
    Some(EmbeddingConfig {
        base_url,
        api_key: api_key.unwrap_or_default(),
        model,
        timeout_seconds: timeout_seconds.unwrap_or(30),
    })
}

pub async fn load_embedding_config(state: &AppState) -> Result<Option<EmbeddingConfig>, ApiError> {
    let (enabled, base_url, api_key, model, timeout_seconds) =
        sqlx::query_as::<_, (bool, Option<String>, Option<String>, Option<String>, Option<i64>)>(
            "SELECT
               embedding_enabled,
               embedding_base_url,
               embedding_api_key,
               embedding_model,
               embedding_timeout_seconds
             FROM system_settings
             WHERE id = 1",
        )
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::from)?;

    Ok(embedding_config_from_row(enabled, base_url, api_key, model, timeout_seconds))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p knowledge-server -- embedding_config_tests`
Expected: PASS — three `embedding_config_tests`.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/retrieval/service.rs
git commit -m "feat(retrieval): read embedding config from dedicated embedding block"
```

---

## Task 7: Web-search config from `search_provider_configs` JSONB

`load_web_search_config` resolves the active provider's fields from the JSONB map, falling back to the flat columns for backward-compat. A **pure** `resolve_web_search_config` does the mapping.

**Files:**
- Modify: `crates/knowledge-server/src/web_search/config.rs`

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `crates/knowledge-server/src/web_search/config.rs`:

```rust
  #[test]
  fn resolve_reads_active_provider_block_from_jsonb() {
    use serde_json::json;
    let configs = json!({
      "tavily": { "apiKey": "tav-key", "baseUrl": "https://tavily.local" }
    });
    let cfg = resolve_web_search_config("tavily", &configs)
      .unwrap()
      .expect("configured");
    assert!(matches!(cfg.provider, WebSearchProvider::Tavily));
    assert_eq!(cfg.api_key.as_deref(), Some("tav-key"));
    assert_eq!(cfg.tavily_base_url, "https://tavily.local");
  }

  #[test]
  fn resolve_none_provider_is_disabled() {
    use serde_json::json;
    assert!(resolve_web_search_config("none", &json!({})).unwrap().is_none());
  }

  #[test]
  fn resolve_defaults_when_block_absent() {
    use serde_json::json;
    let cfg = resolve_web_search_config("tavily", &json!({}))
      .unwrap()
      .expect("configured");
    assert_eq!(cfg.tavily_base_url, "https://api.tavily.com");
    assert!(cfg.api_key.is_none());
  }

  #[test]
  fn resolve_searxng_reads_url_and_categories() {
    use serde_json::json;
    let configs = json!({
      "searxng": { "url": "https://searx.local", "categories": ["news"] }
    });
    let cfg = resolve_web_search_config("searxng", &configs)
      .unwrap()
      .expect("configured");
    assert_eq!(cfg.searxng_url.as_deref(), Some("https://searx.local"));
    assert_eq!(cfg.searxng_categories, vec!["news".to_string()]);
  }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server -- web_search::config`
Expected: FAIL — `resolve_web_search_config` does not exist (compile error).

- [ ] **Step 3: Add the pure resolver and rewrite the loader.** In `crates/knowledge-server/src/web_search/config.rs`, add default-URL constants near the top (after the imports):

```rust
const DEFAULT_TAVILY_BASE_URL: &str = "https://api.tavily.com";
const DEFAULT_SERPAPI_BASE_URL: &str = "https://serpapi.com";
const DEFAULT_OLLAMA_SEARCH_URL: &str = "https://ollama.com";
```

Add the pure resolver (below `parse_categories`):

```rust
/// Pure: resolve the active provider's WebSearchConfig from the per-provider
/// JSONB map. `active` is the search_provider selector; `configs` is
/// search_provider_configs. Returns Ok(None) when the selector is "none".
pub(crate) fn resolve_web_search_config(
  active: &str,
  configs: &Value,
) -> Result<Option<WebSearchConfig>, ApiError> {
  let Some(provider) = parse_provider(active)? else {
    return Ok(None);
  };

  let block = |key: &str| configs.get(key);
  let str_field = |key: &str, field: &str| -> Option<String> {
    block(key)
      .and_then(|b| b.get(field))
      .and_then(Value::as_str)
      .map(str::to_string)
      .filter(|value| !value.trim().is_empty())
  };

  let api_key = match provider {
    WebSearchProvider::Tavily => str_field("tavily", "apiKey"),
    WebSearchProvider::SerpApi => str_field("serpapi", "apiKey"),
    WebSearchProvider::Ollama => str_field("ollama", "apiKey"),
    WebSearchProvider::SearXng => None,
  };

  let searxng_categories = block("searxng")
    .and_then(|b| b.get("categories"))
    .map(parse_categories)
    .unwrap_or_else(|| vec!["general".to_string()]);

  Ok(Some(WebSearchConfig {
    provider,
    api_key,
    serpapi_engine: str_field("serpapi", "engine").unwrap_or_else(|| "google".to_string()),
    searxng_url: str_field("searxng", "url"),
    searxng_categories,
    ollama_search_url: str_field("ollama", "url")
      .unwrap_or_else(|| DEFAULT_OLLAMA_SEARCH_URL.to_string()),
    tavily_base_url: str_field("tavily", "baseUrl")
      .unwrap_or_else(|| DEFAULT_TAVILY_BASE_URL.to_string()),
    serpapi_base_url: str_field("serpapi", "baseUrl")
      .unwrap_or_else(|| DEFAULT_SERPAPI_BASE_URL.to_string()),
  }))
}
```

Then rewrite `load_web_search_config` to fetch the selector + JSONB and delegate:

```rust
pub async fn load_web_search_config(state: &AppState) -> Result<Option<WebSearchConfig>, ApiError> {
  let (search_provider, search_provider_configs) =
    sqlx::query_as::<_, (String, Value)>(
      "SELECT search_provider, search_provider_configs
       FROM system_settings
       WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

  resolve_web_search_config(&search_provider, &search_provider_configs)
}
```

> Note: `parse_categories` currently takes `&Value`; the resolver passes `b.get("categories")` which is `&Value`, so the signature is unchanged. The old flat-column selects (`serpapi_engine`, `searxng_url`, `tavily_base_url`, …) are no longer read by the loader — that is intentional; the JSONB is now the source of truth.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p knowledge-server -- web_search::config`
Expected: PASS — the four new `resolve_*` tests plus the existing `parse_provider_*` / `parse_categories_*` tests.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/web_search/config.rs
git commit -m "feat(web-search): resolve active provider from search_provider_configs JSONB"
```

---

## Task 8: Extend `GET /api/system/settings` response

The response gains `connections` (redacted), `image`, `embedding`, `search.providers`, and `defaults`. Extract a **pure** `connection_to_json` (redacts the raw key) so redaction is unit-testable.

**Files:**
- Modify: `crates/knowledge-server/src/settings/routes.rs`

- [ ] **Step 1: Write the failing test** — add a `tests` module to `crates/knowledge-server/src/settings/routes.rs` (this file currently has none):

```rust
#[cfg(test)]
mod tests {
    use super::connection_to_json;
    use crate::providers::ProviderConnection;

    fn sample() -> ProviderConnection {
        ProviderConnection {
            id: "c1".into(),
            label: "OpenAI".into(),
            base_url: "https://api.openai.com".into(),
            api_key: Some("sk-secret".into()),
            model: "gpt-4o".into(),
            timeout_seconds: Some(30),
            is_active: true,
            sort_order: 0,
            created_at: "t".into(),
            updated_at: "t".into(),
        }
    }

    #[test]
    fn connection_json_redacts_api_key() {
        let json = connection_to_json(&sample());
        assert_eq!(json["id"], "c1");
        assert_eq!(json["label"], "OpenAI");
        assert_eq!(json["model"], "gpt-4o");
        assert_eq!(json["isActive"], true);
        assert_eq!(json["apiKeyConfigured"], true);
        assert!(json.get("apiKey").is_none(), "raw api_key must never be serialized");
    }

    #[test]
    fn connection_json_reports_unconfigured_key() {
        let mut c = sample();
        c.api_key = None;
        assert_eq!(connection_to_json(&c)["apiKeyConfigured"], false);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server -- settings::routes`
Expected: FAIL — `connection_to_json` does not exist (compile error).

- [ ] **Step 3: Add the pure helper** in `crates/knowledge-server/src/settings/routes.rs` (add the import `use crate::providers::{...}` as needed and the helper above `build_settings_response`):

```rust
fn configured(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty())
}

/// Serialize a connection for the settings API, redacting the raw api_key to a
/// boolean flag (matching the providerApiKeyConfigured convention).
pub(crate) fn connection_to_json(c: &crate::providers::ProviderConnection) -> serde_json::Value {
    json!({
        "id": c.id,
        "label": c.label,
        "baseUrl": c.base_url,
        "model": c.model,
        "timeoutSeconds": c.timeout_seconds,
        "isActive": c.is_active,
        "apiKeyConfigured": configured(c.api_key.as_deref()),
    })
}
```

- [ ] **Step 4: Extend `build_settings_response`** to also fetch and include the new blocks. Replace the whole `build_settings_response` fn with a version that (a) keeps the existing single-row select, (b) adds a select for the new columns, (c) lists connections, and (d) merges everything into the JSON:

```rust
async fn build_settings_response(state: &AppState) -> Result<serde_json::Value, ApiError> {
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
    ) = sqlx::query_as::<_, (
        String, String, i64,
        Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>,
        String,
    )>(
        "SELECT provider_mode, language, default_query_limit,
                provider_base_url, provider_api_key, provider_model,
                provider_embedding_model, provider_timeout_seconds, search_provider
         FROM system_settings WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let (
        embedding_enabled,
        embedding_base_url,
        embedding_api_key,
        embedding_model,
        embedding_timeout_seconds,
        image_base_url,
        image_api_key,
        image_model,
        image_size,
        image_timeout_seconds,
        search_provider_configs,
    ) = sqlx::query_as::<_, (
        bool, Option<String>, Option<String>, Option<String>, Option<i64>,
        Option<String>, Option<String>, Option<String>, String, Option<i64>,
        serde_json::Value,
    )>(
        "SELECT embedding_enabled, embedding_base_url, embedding_api_key,
                embedding_model, embedding_timeout_seconds,
                image_base_url, image_api_key, image_model, image_size, image_timeout_seconds,
                search_provider_configs
         FROM system_settings WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let connections: Vec<serde_json::Value> = crate::providers::list_connections(&state.pool)
        .await?
        .iter()
        .map(connection_to_json)
        .collect();

    // Redact api keys inside the search JSONB before returning it.
    let search_providers = redact_search_configs(&search_provider_configs);

    Ok(json!({
        // Legacy flat fields retained for backward compatibility during migration.
        "providerMode": provider_mode,
        "providerBaseUrl": provider_base_url,
        "providerApiKeyConfigured": configured(provider_api_key.as_deref()),
        "providerModel": provider_model,
        "providerEmbeddingModel": provider_embedding_model,
        "providerTimeoutSeconds": provider_timeout_seconds,
        // New structured blocks.
        "connections": connections,
        "embedding": {
            "enabled": embedding_enabled,
            "baseUrl": embedding_base_url,
            "model": embedding_model,
            "timeoutSeconds": embedding_timeout_seconds,
            "apiKeyConfigured": configured(embedding_api_key.as_deref()),
        },
        "image": {
            "baseUrl": image_base_url,
            "model": image_model,
            "size": image_size,
            "timeoutSeconds": image_timeout_seconds,
            "apiKeyConfigured": configured(image_api_key.as_deref()),
        },
        "search": {
            "provider": search_provider,
            "providers": search_providers,
        },
        "defaults": {
            "language": language,
            "defaultQueryLimit": default_query_limit,
        },
    }))
}
```

- [ ] **Step 5: Add the search-redaction helper** in `crates/knowledge-server/src/settings/routes.rs` (above `build_settings_response`):

```rust
/// Return the per-provider search config with any apiKey replaced by an
/// apiKeyConfigured boolean, so raw keys never leave the server.
fn redact_search_configs(configs: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for provider in ["tavily", "serpapi", "searxng", "ollama"] {
        let block = configs.get(provider);
        let mut fields = serde_json::Map::new();
        if let Some(obj) = block.and_then(|b| b.as_object()) {
            for (k, v) in obj {
                if k == "apiKey" {
                    let configured = v.as_str().is_some_and(|s| !s.is_empty());
                    fields.insert("apiKeyConfigured".to_string(), json!(configured));
                } else {
                    fields.insert(k.clone(), v.clone());
                }
            }
        }
        if !fields.contains_key("apiKeyConfigured") && provider != "searxng" {
            fields.insert("apiKeyConfigured".to_string(), json!(false));
        }
        out.insert(provider.to_string(), serde_json::Value::Object(fields));
    }
    serde_json::Value::Object(out)
}
```

- [ ] **Step 6: Run tests + type-check**

Run: `cargo test -p knowledge-server -- settings::routes` then `cargo check -p knowledge-server`
Expected: PASS — `connection_json_redacts_api_key`, `connection_json_reports_unconfigured_key`; clean check.

- [ ] **Step 7: Commit**

```bash
git add crates/knowledge-server/src/settings/routes.rs
git commit -m "feat(settings): GET returns connections + image/embedding/search/defaults blocks (redacted)"
```

---

## Task 9: Provider-connection CRUD endpoints

Four new routes under operator auth + CSRF, mirroring `update_settings`'s guards. They wrap the Task 3 store fns and return the redacted connection JSON (or the full settings response, for consistency with the frontend refetch).

**Files:**
- Modify: `crates/knowledge-server/src/settings/routes.rs`

- [ ] **Step 1: Write the failing deserialization test** — append to the `tests` module in `crates/knowledge-server/src/settings/routes.rs`:

```rust
    use super::{CreateConnectionRequest, UpdateConnectionRequest};

    #[test]
    fn create_connection_request_parses_camel_case() {
        let req: CreateConnectionRequest = serde_json::from_str(
            r#"{"label":"OpenAI","baseUrl":"https://api.openai.com","apiKey":"sk","model":"gpt-4o","timeoutSeconds":30}"#,
        )
        .unwrap();
        assert_eq!(req.label, "OpenAI");
        assert_eq!(req.base_url, "https://api.openai.com");
        assert_eq!(req.model, "gpt-4o");
        assert_eq!(req.timeout_seconds, Some(30));
    }

    #[test]
    fn update_connection_request_defaults_clear_flag_false() {
        let req: UpdateConnectionRequest = serde_json::from_str(
            r#"{"label":"L","baseUrl":"u","model":"m"}"#,
        )
        .unwrap();
        assert_eq!(req.clear_api_key, false);
        assert!(req.api_key.is_none());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server -- settings::routes`
Expected: FAIL — request types not defined (compile error).

- [ ] **Step 3: Add a shared CSRF/operator guard helper** in `crates/knowledge-server/src/settings/routes.rs` (the current `update_settings` inlines this; factor it out so all handlers share it):

```rust
async fn require_operator_csrf(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::auth::session::SessionRecord, ApiError> {
    let session = crate::auth::operator::require_operator(state, headers).await?;
    let supplied = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if supplied.is_empty() || supplied != session.csrf_token {
        return Err(ApiError::unauthorized("invalid csrf token"));
    }
    Ok(session)
}
```

> `require_operator` returns `crate::auth::session::SessionRecord` (confirmed), which carries the `csrf_token` field the current `update_settings` already reads. After adding this helper, update `update_settings` to call `require_operator_csrf(&state, &headers).await?;` in place of its inline `require_operator` + CSRF block (lines ~141-148).

- [ ] **Step 4: Add the request types + handlers** in `crates/knowledge-server/src/settings/routes.rs`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConnectionRequest {
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub model: String,
    #[serde(default)]
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConnectionRequest {
    pub label: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    pub model: String,
    #[serde(default)]
    pub timeout_seconds: Option<i64>,
}

async fn create_connection_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateConnectionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_operator_csrf(&state, &headers).await?;
    crate::providers::create_connection(
        &state.pool,
        &crate::providers::NewConnection {
            label: payload.label,
            base_url: payload.base_url,
            api_key: payload.api_key,
            model: payload.model,
            timeout_seconds: payload.timeout_seconds,
        },
    )
    .await?;
    Ok(Json(build_settings_response(&state).await?))
}

async fn update_connection_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(payload): Json<UpdateConnectionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_operator_csrf(&state, &headers).await?;
    crate::providers::update_connection(
        &state.pool,
        &id,
        &crate::providers::UpdateConnection {
            label: payload.label,
            base_url: payload.base_url,
            api_key: payload.api_key,
            clear_api_key: payload.clear_api_key,
            model: payload.model,
            timeout_seconds: payload.timeout_seconds,
        },
    )
    .await?;
    Ok(Json(build_settings_response(&state).await?))
}

async fn delete_connection_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_operator_csrf(&state, &headers).await?;
    crate::providers::delete_connection(&state.pool, &id).await?;
    Ok(Json(build_settings_response(&state).await?))
}

async fn activate_connection_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_operator_csrf(&state, &headers).await?;
    crate::providers::activate_connection(&state.pool, &id).await?;
    Ok(Json(build_settings_response(&state).await?))
}
```

- [ ] **Step 5: Register the routes** in the `router()` fn of `crates/knowledge-server/src/settings/routes.rs`:

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/system/settings", get(get_settings).patch(update_settings))
        .route(
            "/api/system/provider-connections",
            axum::routing::post(create_connection_handler),
        )
        .route(
            "/api/system/provider-connections/{id}",
            axum::routing::patch(update_connection_handler).delete(delete_connection_handler),
        )
        .route(
            "/api/system/provider-connections/{id}/activate",
            axum::routing::post(activate_connection_handler),
        )
}
```

> Note: this codebase's axum version uses `{id}` path syntax (see `canvas/routes.rs` `/{id}`). Match that style.

- [ ] **Step 6: Run tests + type-check**

Run: `cargo test -p knowledge-server -- settings::routes` then `cargo check -p knowledge-server`
Expected: PASS — the two request-parsing tests; clean check.

- [ ] **Step 7: Commit**

```bash
git add crates/knowledge-server/src/settings/routes.rs
git commit -m "feat(settings): provider-connection CRUD + activate endpoints"
```

---

## Task 10: Extend `PATCH /api/system/settings` with capability blocks

`update_settings` accepts optional `image`, `embedding`, `search`, and `defaults` blocks (each blank-key-keeps, explicit `clear*ApiKey` clears). The existing flat provider/search fields stay accepted for backward-compat, but the new blocks are what the redesigned UI sends.

**Files:**
- Modify: `crates/knowledge-server/src/settings/routes.rs`

- [ ] **Step 1: Write the failing test** — append to the `tests` module:

```rust
    use super::UpdateSettingsRequest;

    #[test]
    fn update_settings_request_parses_capability_blocks() {
        let req: UpdateSettingsRequest = serde_json::from_str(
            r#"{
              "providerMode":"openai-compatible","language":"en","defaultQueryLimit":8,
              "image":{"baseUrl":"https://img.example.com","model":"gpt-image-1","size":"512x512","timeoutSeconds":60},
              "embedding":{"enabled":true,"baseUrl":"https://emb.example.com","model":"text-embedding-3-small"},
              "search":{"provider":"tavily","providers":{"tavily":{"apiKey":"tav","baseUrl":"https://api.tavily.com"}}}
            }"#,
        )
        .unwrap();
        let image = req.image.expect("image block");
        assert_eq!(image.model.as_deref(), Some("gpt-image-1"));
        assert_eq!(image.size.as_deref(), Some("512x512"));
        let embedding = req.embedding.expect("embedding block");
        assert_eq!(embedding.enabled, Some(true));
        let search = req.search.expect("search block");
        assert_eq!(search.provider.as_deref(), Some("tavily"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p knowledge-server -- update_settings_request_parses_capability_blocks`
Expected: FAIL — `UpdateSettingsRequest` has no `image`/`embedding`/`search` fields (compile error).

- [ ] **Step 3: Add the block types and extend the request** in `crates/knowledge-server/src/settings/routes.rs`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSettingsBlock {
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSettingsBlock {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSettingsBlock {
    #[serde(default)]
    pub provider: Option<String>,
    /// Full per-provider config map (already merged client-side so switching
    /// providers retains each key). Stored verbatim into search_provider_configs.
    #[serde(default)]
    pub providers: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultsBlock {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub default_query_limit: Option<i64>,
}
```

Add these fields to `UpdateSettingsRequest`:

```rust
    #[serde(default)]
    pub image: Option<ImageSettingsBlock>,
    #[serde(default)]
    pub embedding: Option<EmbeddingSettingsBlock>,
    #[serde(default)]
    pub search: Option<SearchSettingsBlock>,
    #[serde(default)]
    pub defaults: Option<DefaultsBlock>,
```

- [ ] **Step 4: Run the parse test to verify it passes**

Run: `cargo test -p knowledge-server -- update_settings_request_parses_capability_blocks`
Expected: PASS.

- [ ] **Step 5: Persist the new blocks** in `update_settings`. After the existing legacy `UPDATE system_settings ... WHERE id = 1` statement (keep it for backward-compat), add block-specific updates guarded by `if let Some(...)`. Insert before the final `Ok(Json(build_settings_response(&state).await?))`:

```rust
    if let Some(image) = &payload.image {
        sqlx::query(
            "UPDATE system_settings
             SET image_base_url = COALESCE($1, image_base_url),
                 image_api_key = COALESCE(NULLIF($2, ''), CASE WHEN $3 THEN NULL ELSE image_api_key END),
                 image_model = COALESCE($4, image_model),
                 image_size = COALESCE($5, image_size),
                 image_timeout_seconds = COALESCE($6, image_timeout_seconds)
             WHERE id = 1",
        )
        .bind(image.base_url.as_deref())
        .bind(image.api_key.as_deref())
        .bind(image.clear_api_key)
        .bind(image.model.as_deref())
        .bind(image.size.as_deref())
        .bind(image.timeout_seconds)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    }

    if let Some(embedding) = &payload.embedding {
        sqlx::query(
            "UPDATE system_settings
             SET embedding_enabled = COALESCE($1, embedding_enabled),
                 embedding_base_url = COALESCE($2, embedding_base_url),
                 embedding_api_key = COALESCE(NULLIF($3, ''), CASE WHEN $4 THEN NULL ELSE embedding_api_key END),
                 embedding_model = COALESCE($5, embedding_model),
                 embedding_timeout_seconds = COALESCE($6, embedding_timeout_seconds)
             WHERE id = 1",
        )
        .bind(embedding.enabled)
        .bind(embedding.base_url.as_deref())
        .bind(embedding.api_key.as_deref())
        .bind(embedding.clear_api_key)
        .bind(embedding.model.as_deref())
        .bind(embedding.timeout_seconds)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    }

    if let Some(search) = &payload.search {
        if let Some(provider) = &search.provider {
            sqlx::query("UPDATE system_settings SET search_provider = $1 WHERE id = 1")
                .bind(provider)
                .execute(&state.pool)
                .await
                .map_err(ApiError::from)?;
        }
        // Deep-merge the incoming per-provider map into the stored one so fields
        // the client omits (notably a configured apiKey it never sees, because
        // GET redacts it) are preserved. Whole-object replacement would destroy
        // stored keys whenever any field is edited or the provider is switched.
        if let Some(incoming) = &search.providers {
            let existing: serde_json::Value = sqlx::query_scalar(
                "SELECT search_provider_configs FROM system_settings WHERE id = 1",
            )
            .fetch_one(&state.pool)
            .await
            .map_err(ApiError::from)?;
            let merged = merge_search_provider_configs(&existing, incoming);
            sqlx::query("UPDATE system_settings SET search_provider_configs = $1 WHERE id = 1")
                .bind(merged)
                .execute(&state.pool)
                .await
                .map_err(ApiError::from)?;
        }
    }

    if let Some(defaults) = &payload.defaults {
        sqlx::query(
            "UPDATE system_settings
             SET language = COALESCE($1, language),
                 default_query_limit = COALESCE($2, default_query_limit)
             WHERE id = 1",
        )
        .bind(defaults.language.as_deref())
        .bind(defaults.default_query_limit)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;
    }
```

> Note on the search block: `search_provider_configs` is **deep-merged**, not
> replaced. `merge_search_provider_configs` (added in Step 6 below) takes the
> stored map and the incoming per-provider map and, for each provider, overlays
> the incoming fields onto the stored fields — but drops any incoming field whose
> value is an empty string (`""`), so a blank apiKey box keeps the stored key.
> This is what lets the redesigned UI switch providers and edit non-secret fields
> without ever seeing or clobbering the stored keys.

- [ ] **Step 6: Add + test the pure merge helper.** First append the failing test to the `tests` module in `crates/knowledge-server/src/settings/routes.rs`:

```rust
    use super::merge_search_provider_configs;

    #[test]
    fn merge_overlays_incoming_fields_and_preserves_stored_key() {
        use serde_json::json;
        let stored = json!({
            "tavily": { "apiKey": "stored-key", "baseUrl": "https://api.tavily.com" }
        });
        // The client resends the redacted-then-edited block: no apiKey (it never
        // saw it) but a changed baseUrl.
        let incoming = json!({
            "tavily": { "baseUrl": "https://tavily.local" }
        });
        let merged = merge_search_provider_configs(&stored, &incoming);
        assert_eq!(merged["tavily"]["apiKey"], "stored-key");
        assert_eq!(merged["tavily"]["baseUrl"], "https://tavily.local");
    }

    #[test]
    fn merge_blank_apikey_keeps_stored_and_nonblank_replaces() {
        use serde_json::json;
        let stored = json!({ "tavily": { "apiKey": "old" } });
        // Blank string means "keep": stored key survives.
        let kept = merge_search_provider_configs(&stored, &json!({ "tavily": { "apiKey": "" } }));
        assert_eq!(kept["tavily"]["apiKey"], "old");
        // Non-blank string replaces.
        let replaced = merge_search_provider_configs(&stored, &json!({ "tavily": { "apiKey": "new" } }));
        assert_eq!(replaced["tavily"]["apiKey"], "new");
    }

    #[test]
    fn merge_adds_provider_absent_from_stored() {
        use serde_json::json;
        let merged = merge_search_provider_configs(
            &json!({}),
            &json!({ "searxng": { "url": "https://searx.local" } }),
        );
        assert_eq!(merged["searxng"]["url"], "https://searx.local");
    }
```

Run: `cargo test -p knowledge-server -- merge_` — Expected: FAIL (helper not defined).

Then add the pure helper above `update_settings` in `crates/knowledge-server/src/settings/routes.rs`:

```rust
/// Deep-merge an incoming per-provider search-config map onto the stored one.
/// For each provider present in `incoming`, overlay its fields onto the stored
/// provider block, but SKIP any incoming field whose value is an empty string so
/// a blank apiKey box keeps the stored key (the client never sees stored keys —
/// GET redacts them). Providers absent from `incoming` are left untouched.
pub(crate) fn merge_search_provider_configs(
    stored: &serde_json::Value,
    incoming: &serde_json::Value,
) -> serde_json::Value {
    let mut out = stored.as_object().cloned().unwrap_or_default();
    if let Some(incoming_obj) = incoming.as_object() {
        for (provider, fields) in incoming_obj {
            let mut block = out
                .get(provider)
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();
            if let Some(field_obj) = fields.as_object() {
                for (key, value) in field_obj {
                    // Blank string = "keep whatever is stored" (do not overwrite).
                    if value.as_str() == Some("") {
                        continue;
                    }
                    block.insert(key.clone(), value.clone());
                }
            }
            out.insert(provider.clone(), serde_json::Value::Object(block));
        }
    }
    serde_json::Value::Object(out)
}
```

Run: `cargo test -p knowledge-server -- merge_` — Expected: PASS (three `merge_*` tests).

- [ ] **Step 7: Run tests + type-check**

Run: `cargo test -p knowledge-server` then `cargo check -p knowledge-server`
Expected: PASS — all unit tests; clean check.

- [ ] **Step 8: Commit**

```bash
git add crates/knowledge-server/src/settings/routes.rs
git commit -m "feat(settings): PATCH accepts image/embedding/search/defaults capability blocks"
```

---

## Task 11: Full backend verification

**Files:** none (verification only).

- [ ] **Step 1: Run the whole backend suite**

Run: `cargo test -p knowledge-server`
Expected: PASS — all unit tests including the new `resolve_active`, `image_config_from_row`, `embedding_config_from_row`, `resolve_web_search_config`, `connection_to_json`, and request-parsing tests.

- [ ] **Step 2: Type-check the whole crate**

Run: `cargo check -p knowledge-server`
Expected: clean — no dead-code warnings for removed `build_provider` (confirm it's gone), no unused imports.

- [ ] **Step 3: Migrate + smoke-test against Postgres.** Bring the stack up and hard-verify the image fix end to end:

Run: `docker compose build backend && docker compose up -d backend db`
Then, with an operator session in the admin app (frontend still old until the frontend plan lands — verify via API or the existing settings form):
- Confirm `GET /api/system/settings` returns a `connections` array (one "Default" if a provider was previously configured), plus `image`, `embedding`, `search`, `defaults` blocks with keys redacted to `apiKeyConfigured`.
- `POST /api/system/provider-connections` with a second connection, then `POST …/{id}/activate`; confirm exactly one `isActive: true`.
- `DELETE` the active connection; confirm the next connection auto-activates.
- `PATCH /api/system/settings` with an `image` block (a real image model + `size`), then run a canvas `ai_image` node (`/image a red fox`): confirm it now produces an asset + preview instead of failing on the text model.

Expected: image generation succeeds against the image model; connection activation/deletion invariants hold.

- [ ] **Step 4: Final commit (if any verification tweaks were needed)**

```bash
git add -A
git commit -m "test(backend): verify multi-provider config end to end"
```

---

## Self-review checklist (run before handing off)

- **Spec coverage:** migration 0015 (✓ Task 1), `provider_connections` + `load_active_connection` (✓ Task 3), `load_image_config` + `ProviderImageRequest.size` (✓ Tasks 2, 4), repoint 5 sites — chat, analyze, image, embedding, web search (✓ Tasks 5, 6, 7), settings API GET shape (✓ Task 8), connection CRUD (✓ Task 9), extended PATCH (✓ Task 10). Frontend (settings UI, node model tag) is intentionally deferred to the frontend plan.
- **Type consistency:** `ProviderConnection` fields are identical across connections.rs (definition), the settings test, and `connection_to_json`. `ActiveConnection::provider()` and `ImageConfig::provider()` both call `OpenAiCompatibleProvider::new`. `ProviderImageRequest { prompt, size }` is constructed consistently in `run_image_skill` and both image tests.
- **No placeholders:** every code step contains complete code. All external types referenced (`SessionRecord`, `OpenAiCompatibleProvider::new`, `NewAsset::new`) are confirmed against the current source.
