# Knowledge Canvas (知识工作台) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a top-level "infinite canvas" feature where users link knowledge nodes (web pages, wiki knowledge bases, notes, AI analyses, generated images) with directed reference edges and an AI reasons over the board.

**Architecture:** A new `canvas` Rust module in `crates/knowledge-server` exposes canvas CRUD + autosave (JSONB-as-TEXT document), node-run SSE, and canvas-chat SSE; it orchestrates existing capabilities (provider `stream_chat`, project RAG retrieval, `web_search`) plus two net-new capabilities (text-to-image via the provider, and URL fetch -> readability -> markdown). A unified `assets` store serves generated images by URL. The React 19 admin gets a new top-level `/canvas` destination built on `@xyflow/react` with debounced autosave; the server never mutates the document — the sole write path is the frontend autosave PUT.

**Tech Stack:** Rust (Axum 0.8, sqlx 0.8 / Postgres, reqwest 0.12, scraper, base64), React 19 (Vite, React Router 7, TanStack Query 5, Tailwind v4, shadcn/ui, `@xyflow/react`), SSE streaming.

---

## Source of Truth

Design spec: `docs/superpowers/specs/2026-06-30-knowledge-canvas-design.md` (approved, committed).

## Standing Constraints (apply to EVERY task)

- **Port upstream logic, cite paths.** Key feature logic must be ported/adapted from `upstream_llm_wiki/`, not invented. Cite the upstream path in code comments where logic is ported (URL->markdown: `upstream_llm_wiki/extension/Readability.js` + `Turndown.js`; streaming/skills: `upstream_llm_wiki/src/components/chat/chat-panel.tsx`, `src/lib/llm-client.ts`, `src/lib/llm-providers.ts`; web search: `upstream_llm_wiki/src/lib/web-search.ts`).
- **Use shadcn/ui primitives.** Admin UI composes shadcn primitives (Button, Tooltip, Dialog, Input, Textarea, DropdownMenu, etc.) — never hand-roll one-off components. Node markdown renders through the existing `apps/admin/src/components/shared/markdown-message.tsx`.
- **Polish + reachable flows.** Every screen polished; the Canvas nav entry and the "new canvas" action must be surfaced.
- **ASCII-only** in all files. No emoji in code/files.
- **Build hygiene:** run admin npm scripts from `apps/admin` (e.g. `cd /e/Projects/Js/knowledge/apps/admin && npm test`). Never run npm from the repo root. Backend: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server`.
- **Git:** `git add` specific files (never `-A`); make NEW commits (never amend); commit after each task.
- **TDD:** write the failing test first, watch it fail, implement minimally, watch it pass, commit.

## Test Style (match the existing repo)

The repo has **no DB-backed sqlx tests**. Follow the existing patterns:
- **Backend pure-function tests:** `#[cfg(test)] mod tests` in the same file; assert serde_json serialization VALUES (not just field names), pure transforms, and helper outputs. See `crates/knowledge-server/src/providers/openai_compatible.rs` tests and `web_search/provider.rs`.
- **Backend HTTP-mock tests:** `async fn spawn_mock_*() -> (JoinHandle, String)` spins a TCP server returning fixed HTTP/JSON; `#[tokio::test]` calls the unit pointed at the mock base URL. See `crates/knowledge-server/src/web_search/provider.rs:440-568`.
- **Frontend tests:** vitest + jsdom + Testing Library; mock heavy child components via `vi.mock`; mock `fetch` via `vi.stubGlobal`. See `apps/admin/src/app/router.test.tsx` and `features/chat/stream.test.ts`.

---

## File Structure

**Backend (`crates/knowledge-server/`):**

- `migrations/0014_canvas.sql` — CREATE: `canvases`, `canvas_access` (reserved), `assets`.
- `src/canvas/mod.rs` — module exports (`pub mod routes; pub mod store; pub mod service; pub mod document;`).
- `src/canvas/document.rs` — `CanvasDocument` serde types (nodes/edges/viewport, per-type data), pure helpers (`incoming_source_ids`, `active_version_content`, `default_canvas_document`).
- `src/canvas/store.rs` — sqlx CRUD over `canvases` (document stored as TEXT-JSON).
- `src/canvas/service.rs` — node-run orchestration: gather referenced node content, KB RAG, assemble prompt; slash-skill helpers (`search_results_to_markdown`), URL fetch->markdown (`html_to_markdown`).
- `src/canvas/routes.rs` — axum router + handlers (CRUD, autosave PUT, node-run SSE, chat SSE, URL extract).
- `src/assets/mod.rs`, `src/assets/store.rs`, `src/assets/routes.rs` — unified asset store + `GET /api/assets/:id`.
- `src/providers/types.rs` — add `ProviderImageRequest`, `ProviderImageResult`.
- `src/providers/openai_compatible.rs` — add `generate_image` + `images_url`.
- `src/providers/mod.rs` (or wherever the `Provider` trait lives) — add `generate_image` to the trait.
- `src/lib.rs` — add `pub mod canvas;` + `pub mod assets;`.
- `src/http/router.rs` — `.merge(canvas::routes::router())` + `.merge(assets::routes::router())`.
- `Cargo.toml` + root `Cargo.toml` `[workspace.dependencies]` — add `scraper`.

**Frontend (`apps/admin/src/`):**

- `features/canvas/types.ts` — TS types + Zod schemas mirroring the document shape.
- `features/canvas/api.ts` — REST calls (list/create/get/save/delete, extract URL).
- `features/canvas/queries.ts` — TanStack Query keys + hooks.
- `features/canvas/stream.ts` — `runCanvasNode` + `streamCanvasChat` SSE clients.
- `features/canvas/use-autosave.ts` — debounced autosave hook + save status.
- `features/canvas/history-sidebar.tsx` — "my canvases" list + new-canvas action.
- `features/canvas/canvas-board.tsx` — `@xyflow/react` wrapper.
- `features/canvas/node-types/{note,url,kb,ai-analyze,ai-image}.tsx` — one component per node type.
- `features/canvas/chat-panel.tsx` — canvas chat + slash menu.
- `features/canvas/page.tsx` — three-pane composition.
- `lib/route-meta.ts` — add `{ to: "/canvas", label: "Canvas" }` to `globalNav`.
- `app/router.tsx` — lazy `/canvas` + `/canvas/:canvasId` routes.

---

# Phase 1 — Backend Foundation (storage + CRUD + assets)

## Task 1: Migration 0014 — canvases, canvas_access, assets

**Files:**
- Create: `crates/knowledge-server/migrations/0014_canvas.sql`

- [ ] **Step 1: Write the migration**

Follow the existing convention (see `migrations/0006_conversations.sql`): `id TEXT PRIMARY KEY NOT NULL`, TEXT timestamps, `FOREIGN KEY ... ON DELETE CASCADE`, `CREATE INDEX idx_{table}_{cols}`. `document` and `assets.bytes` are the only non-TEXT columns (document is TEXT-JSON; bytes is BYTEA).

```sql
-- Knowledge Canvas: per-user infinite canvas boards + unified asset store.

CREATE TABLE canvases (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL,
    title TEXT NOT NULL,
    document TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_canvases_owner_id ON canvases (owner_id);

-- Reserved for future sharing; v1 only ever reads owner_id = current user.
CREATE TABLE canvas_access (
    canvas_id TEXT NOT NULL,
    principal_id TEXT NOT NULL,
    role TEXT NOT NULL,
    PRIMARY KEY (canvas_id, principal_id),
    FOREIGN KEY (canvas_id) REFERENCES canvases(id) ON DELETE CASCADE
);

-- Unified asset store (e.g. generated images). Reusable beyond canvas.
CREATE TABLE assets (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL,
    mime TEXT NOT NULL,
    bytes BYTEA NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_assets_owner_id ON assets (owner_id);
```

- [ ] **Step 2: Verify it compiles into the migrator**

Run: `cd /e/Projects/Js/knowledge && cargo build -p knowledge-server`
Expected: builds clean. `MIGRATOR = sqlx::migrate!("./migrations")` (in `src/db/migrate.rs`) auto-discovers 0014; no code change needed to register it.

> Note: confirm the `users` table name/PK column matches the existing schema before committing. If the existing FKs reference a different principal table, match that table here (check `migrations/0006_conversations.sql` and earlier migrations for the exact referenced table).

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/migrations/0014_canvas.sql
git commit -m "feat(canvas): add migration 0014 for canvases, canvas_access, assets"
```

---

## Task 2: Canvas document types + pure helpers

**Files:**
- Create: `crates/knowledge-server/src/canvas/mod.rs`
- Create: `crates/knowledge-server/src/canvas/document.rs`
- Modify: `crates/knowledge-server/src/lib.rs` (add `pub mod canvas;`)

- [ ] **Step 1: Write the failing tests**

Create `crates/knowledge-server/src/canvas/document.rs` with the types below and this test module. The tests lock helper behavior and the default-document shape.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, ty: &str) -> CanvasNode {
        CanvasNode {
            id: id.to_string(),
            r#type: ty.to_string(),
            x: 0.0,
            y: 0.0,
            w: 280.0,
            h: 160.0,
            data: serde_json::json!({}),
        }
    }

    #[test]
    fn incoming_source_ids_returns_sources_of_edges_targeting_node() {
        let doc = CanvasDocument {
            nodes: vec![node("a", "note"), node("b", "note"), node("c", "ai_analyze")],
            edges: vec![
                CanvasEdge { id: "e1".into(), source: "a".into(), target: "c".into() },
                CanvasEdge { id: "e2".into(), source: "b".into(), target: "c".into() },
                CanvasEdge { id: "e3".into(), source: "a".into(), target: "b".into() },
            ],
            viewport: Viewport::default(),
        };
        let mut got = doc.incoming_source_ids("c");
        got.sort();
        assert_eq!(got, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn default_canvas_document_is_empty_with_unit_viewport() {
        let doc = default_canvas_document();
        assert!(doc.nodes.is_empty());
        assert!(doc.edges.is_empty());
        assert_eq!(doc.viewport.zoom, 1.0);
    }

    #[test]
    fn active_version_content_picks_active_id() {
        let data = serde_json::json!({
            "prompt": "p",
            "versions": [
                { "id": "v1", "content": "first", "createdAt": "t1" },
                { "id": "v2", "content": "second", "createdAt": "t2" }
            ],
            "activeVersionId": "v2",
            "status": "idle",
            "error": null
        });
        assert_eq!(active_version_content(&data).as_deref(), Some("second"));
    }

    #[test]
    fn active_version_content_none_when_no_versions() {
        let data = serde_json::json!({ "prompt": "p", "versions": [], "activeVersionId": null });
        assert_eq!(active_version_content(&data), None);
    }

    #[test]
    fn document_roundtrips_through_json() {
        let doc = default_canvas_document();
        let s = serde_json::to_string(&doc).unwrap();
        let back: CanvasDocument = serde_json::from_str(&s).unwrap();
        assert_eq!(back.nodes.len(), 0);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::document`
Expected: FAIL to compile (types/functions not defined).

- [ ] **Step 3: Implement the types + helpers**

Top of `crates/knowledge-server/src/canvas/document.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasDocument {
    #[serde(default)]
    pub nodes: Vec<CanvasNode>,
    #[serde(default)]
    pub edges: Vec<CanvasEdge>,
    #[serde(default)]
    pub viewport: Viewport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasNode {
    pub id: String,
    pub r#type: String,
    pub x: f64,
    pub y: f64,
    #[serde(default = "default_w")]
    pub w: f64,
    #[serde(default = "default_h")]
    pub h: f64,
    #[serde(default)]
    pub data: serde_json::Value,
}

fn default_w() -> f64 { 280.0 }
fn default_h() -> f64 { 160.0 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanvasEdge {
    pub id: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, zoom: 1.0 }
    }
}

impl CanvasDocument {
    /// Source node ids of edges whose target is `node_id`.
    pub fn incoming_source_ids(&self, node_id: &str) -> Vec<String> {
        self.edges
            .iter()
            .filter(|e| e.target == node_id)
            .map(|e| e.source.clone())
            .collect()
    }

    pub fn node(&self, node_id: &str) -> Option<&CanvasNode> {
        self.nodes.iter().find(|n| n.id == node_id)
    }
}

pub fn default_canvas_document() -> CanvasDocument {
    CanvasDocument {
        nodes: Vec::new(),
        edges: Vec::new(),
        viewport: Viewport::default(),
    }
}

/// For an AI node `data`, return the content string of the active version.
pub fn active_version_content(data: &serde_json::Value) -> Option<String> {
    let active = data.get("activeVersionId")?.as_str()?;
    let versions = data.get("versions")?.as_array()?;
    versions
        .iter()
        .find(|v| v.get("id").and_then(|i| i.as_str()) == Some(active))
        .and_then(|v| v.get("content").and_then(|c| c.as_str()))
        .map(|s| s.to_string())
}
```

Create `crates/knowledge-server/src/canvas/mod.rs`:

```rust
pub mod document;
pub mod store;
pub mod service;
pub mod routes;
```

> Note: `store`, `service`, `routes` are created in later tasks. To keep the crate compiling after THIS task, temporarily declare only `pub mod document;` in `mod.rs`, and add the other `pub mod` lines in their respective tasks (Task 3 adds `store`, Task 8 adds `service`, Task 4/9 add `routes`).

Add to `crates/knowledge-server/src/lib.rs` module list:

```rust
pub mod canvas;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::document`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/mod.rs crates/knowledge-server/src/canvas/document.rs crates/knowledge-server/src/lib.rs
git commit -m "feat(canvas): add canvas document types and pure helpers"
```

---

## Task 3: Canvas store (sqlx CRUD)

**Files:**
- Create: `crates/knowledge-server/src/canvas/store.rs`
- Modify: `crates/knowledge-server/src/canvas/mod.rs` (uncomment `pub mod store;`)

- [ ] **Step 1: Write the failing test (pure serialization of the row record)**

The repo has no DB tests, so test the pure parts: that a `CanvasRecord` parses its TEXT `document` into a `CanvasDocument`, and that a summary projection drops the document. Put this in `store.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_document_reads_text_json() {
        let rec = CanvasRecord {
            id: "c1".into(),
            owner_id: "u1".into(),
            title: "Board".into(),
            document: r#"{"nodes":[],"edges":[],"viewport":{"x":0,"y":0,"zoom":1}}"#.into(),
            created_at: "t1".into(),
            updated_at: "t2".into(),
        };
        let doc = rec.parse_document().expect("valid json");
        assert!(doc.nodes.is_empty());
        assert_eq!(doc.viewport.zoom, 1.0);
    }

    #[test]
    fn parse_document_errs_on_bad_json() {
        let rec = CanvasRecord {
            id: "c1".into(),
            owner_id: "u1".into(),
            title: "Board".into(),
            document: "not json".into(),
            created_at: "t1".into(),
            updated_at: "t2".into(),
        };
        assert!(rec.parse_document().is_err());
    }

    #[test]
    fn summary_from_record_drops_document() {
        let rec = CanvasRecord {
            id: "c1".into(),
            owner_id: "u1".into(),
            title: "Board".into(),
            document: "{}".into(),
            created_at: "t1".into(),
            updated_at: "t2".into(),
        };
        let summary = CanvasSummary::from(&rec);
        assert_eq!(summary.id, "c1");
        assert_eq!(summary.title, "Board");
        assert_eq!(summary.updated_at, "t2");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::store`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the store**

Model on `crates/knowledge-server/src/chat/store.rs` (String fields, `sqlx::query`/`query_as`, `now_rfc3339`, `Uuid::new_v4().to_string()`).

```rust
use serde::Serialize;
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use super::document::{default_canvas_document, CanvasDocument};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CanvasRecord {
    pub id: String,
    pub owner_id: String,
    pub title: String,
    pub document: String,
    pub created_at: String,
    pub updated_at: String,
}

impl CanvasRecord {
    pub fn parse_document(&self) -> Result<CanvasDocument, serde_json::Error> {
        serde_json::from_str(&self.document)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CanvasSummary {
    pub id: String,
    pub title: String,
    pub updated_at: String,
}

impl From<&CanvasRecord> for CanvasSummary {
    fn from(rec: &CanvasRecord) -> Self {
        Self {
            id: rec.id.clone(),
            title: rec.title.clone(),
            updated_at: rec.updated_at.clone(),
        }
    }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub async fn create_canvas(
    pool: &PgPool,
    owner_id: &str,
    title: &str,
) -> Result<CanvasRecord, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let document = serde_json::to_string(&default_canvas_document()).unwrap_or_else(|_| "{}".into());
    sqlx::query(
        "INSERT INTO canvases (id, owner_id, title, document, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&id)
    .bind(owner_id)
    .bind(title)
    .bind(&document)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(CanvasRecord {
        id,
        owner_id: owner_id.to_string(),
        title: title.to_string(),
        document,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn list_canvases(
    pool: &PgPool,
    owner_id: &str,
) -> Result<Vec<CanvasRecord>, sqlx::Error> {
    sqlx::query_as::<_, CanvasRecord>(
        "SELECT id, owner_id, title, document, created_at, updated_at \
         FROM canvases WHERE owner_id = $1 ORDER BY updated_at DESC",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
}

pub async fn get_canvas(
    pool: &PgPool,
    id: &str,
    owner_id: &str,
) -> Result<Option<CanvasRecord>, sqlx::Error> {
    sqlx::query_as::<_, CanvasRecord>(
        "SELECT id, owner_id, title, document, created_at, updated_at \
         FROM canvases WHERE id = $1 AND owner_id = $2",
    )
    .bind(id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
}

pub async fn update_canvas(
    pool: &PgPool,
    id: &str,
    owner_id: &str,
    title: &str,
    document: &str,
) -> Result<Option<CanvasRecord>, sqlx::Error> {
    let now = now_rfc3339();
    let affected = sqlx::query(
        "UPDATE canvases SET title = $1, document = $2, updated_at = $3 \
         WHERE id = $4 AND owner_id = $5",
    )
    .bind(title)
    .bind(document)
    .bind(&now)
    .bind(id)
    .bind(owner_id)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Ok(None);
    }
    get_canvas(pool, id, owner_id).await
}

pub async fn delete_canvas(
    pool: &PgPool,
    id: &str,
    owner_id: &str,
) -> Result<bool, sqlx::Error> {
    let affected = sqlx::query("DELETE FROM canvases WHERE id = $1 AND owner_id = $2")
        .bind(id)
        .bind(owner_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(affected > 0)
}
```

Uncomment `pub mod store;` in `crates/knowledge-server/src/canvas/mod.rs`.

> Note: confirm `uuid` and `time` are already crate deps (chat/store.rs uses them). If `CanvasRecord` field types need to match the DB driver, mirror `chat/store.rs` exactly.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::store`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/store.rs crates/knowledge-server/src/canvas/mod.rs
git commit -m "feat(canvas): add canvas store CRUD over canvases table"
```

---

## Task 4: Canvas CRUD + autosave routes

**Files:**
- Create: `crates/knowledge-server/src/canvas/routes.rs`
- Modify: `crates/knowledge-server/src/canvas/mod.rs` (uncomment `pub mod routes;`)
- Modify: `crates/knowledge-server/src/http/router.rs` (merge canvas router)

- [ ] **Step 1: Write the failing test (request/response shapes)**

Test the pure request/response serde. Put in `routes.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_request_defaults_title_when_absent() {
        let req: CreateCanvasRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(req.resolved_title(), "Untitled canvas");
    }

    #[test]
    fn create_request_uses_provided_title() {
        let req: CreateCanvasRequest =
            serde_json::from_str(r#"{"title":"Research B"}"#).unwrap();
        assert_eq!(req.resolved_title(), "Research B");
    }

    #[test]
    fn save_request_serializes_document_to_text() {
        let req: SaveCanvasRequest = serde_json::from_str(
            r#"{"title":"T","document":{"nodes":[],"edges":[],"viewport":{"x":0,"y":0,"zoom":1}}}"#,
        )
        .unwrap();
        let text = req.document_text();
        assert!(text.contains("\"viewport\""));
        // round-trips back to a CanvasDocument
        let _doc: crate::canvas::document::CanvasDocument =
            serde_json::from_str(&text).unwrap();
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::routes`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the routes**

Model auth on `crates/knowledge-server/src/web_search/routes.rs` (top-level: `resolve_principal` + inline CSRF). Use `AppState` exactly as other route modules do (check the concrete state type name in `http/router.rs`; below uses `AppState` as a placeholder — match the real one).

```rust
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::auth::principal::resolve_principal;
use crate::canvas::document::CanvasDocument;
use crate::canvas::store;
use crate::error::ApiError; // match the repo's error type/path
use crate::state::AppState; // match the repo's state type/path

#[derive(Debug, Deserialize)]
pub struct CreateCanvasRequest {
    #[serde(default)]
    pub title: Option<String>,
}

impl CreateCanvasRequest {
    pub fn resolved_title(&self) -> String {
        match self.title.as_deref().map(str::trim) {
            Some(t) if !t.is_empty() => t.to_string(),
            _ => "Untitled canvas".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SaveCanvasRequest {
    pub title: String,
    pub document: CanvasDocument,
}

impl SaveCanvasRequest {
    pub fn document_text(&self) -> String {
        serde_json::to_string(&self.document).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct CanvasResponse {
    pub id: String,
    pub title: String,
    pub document: CanvasDocument,
    pub created_at: String,
    pub updated_at: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/canvases", get(list_handler).post(create_handler))
        .route(
            "/api/canvases/:id",
            get(get_handler).put(save_handler).delete(delete_handler),
        )
}

// Top-level CSRF check, mirroring web_search/routes.rs.
fn require_csrf(principal: &crate::auth::principal::Principal, headers: &HeaderMap) -> Result<(), ApiError> {
    if principal.requires_csrf() {
        let provided = headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let expected = principal.csrf_token.as_deref().unwrap_or("");
        if expected.is_empty() || provided != expected {
            return Err(ApiError::forbidden("invalid csrf token"));
        }
    }
    Ok(())
}

async fn list_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<store::CanvasSummary>>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    let records = store::list_canvases(state.pool(), &principal.user_id).await?;
    Ok(Json(records.iter().map(store::CanvasSummary::from).collect()))
}

async fn create_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateCanvasRequest>,
) -> Result<Json<CanvasResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let rec = store::create_canvas(state.pool(), &principal.user_id, &body.resolved_title()).await?;
    let document = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    Ok(Json(CanvasResponse {
        id: rec.id,
        title: rec.title,
        document,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
    }))
}

async fn get_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<CanvasResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    let rec = store::get_canvas(state.pool(), &id, &principal.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("canvas not found"))?;
    let document = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    Ok(Json(CanvasResponse {
        id: rec.id,
        title: rec.title,
        document,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
    }))
}

async fn save_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<SaveCanvasRequest>,
) -> Result<Json<CanvasResponse>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let rec = store::update_canvas(
        state.pool(),
        &id,
        &principal.user_id,
        &body.title,
        &body.document_text(),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("canvas not found"))?;
    let document = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    Ok(Json(CanvasResponse {
        id: rec.id,
        title: rec.title,
        document,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
    }))
}

async fn delete_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let deleted = store::delete_canvas(state.pool(), &id, &principal.user_id).await?;
    if !deleted {
        return Err(ApiError::not_found("canvas not found"));
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}
```

Uncomment `pub mod routes;` in `mod.rs`. In `crates/knowledge-server/src/http/router.rs`, add inside `build_router`:

```rust
.merge(crate::canvas::routes::router())
```

> Note: the exact names `AppState`, `state.pool()`, `ApiError::forbidden/internal/not_found`, and `resolve_principal`'s signature MUST be matched to the repo. Before writing, open `web_search/routes.rs` and `http/router.rs` and copy the precise types/constructors. Adjust `:id` vs `{id}` path syntax to match the Axum version's other routes.

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::routes`
Expected: PASS (3 tests). Then `cargo build -p knowledge-server` to confirm the router merges.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs crates/knowledge-server/src/canvas/mod.rs crates/knowledge-server/src/http/router.rs
git commit -m "feat(canvas): add canvas CRUD and autosave routes"
```

---

## Task 5: Asset store + GET /api/assets/:id

**Files:**
- Create: `crates/knowledge-server/src/assets/mod.rs`
- Create: `crates/knowledge-server/src/assets/store.rs`
- Create: `crates/knowledge-server/src/assets/routes.rs`
- Modify: `crates/knowledge-server/src/lib.rs` (add `pub mod assets;`)
- Modify: `crates/knowledge-server/src/http/router.rs` (merge assets router)

- [ ] **Step 1: Write the failing test**

Test the asset URL helper + record shape in `store.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_url_uses_api_path() {
        assert_eq!(asset_url("abc123"), "/api/assets/abc123");
    }

    #[test]
    fn new_asset_generates_id_and_keeps_mime() {
        let asset = NewAsset::new("user1", "image/png", vec![1, 2, 3]);
        assert_eq!(asset.owner_id, "user1");
        assert_eq!(asset.mime, "image/png");
        assert_eq!(asset.bytes, vec![1, 2, 3]);
        assert!(!asset.id.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server assets::store`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the asset store + routes**

`crates/knowledge-server/src/assets/store.rs`:

```rust
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn asset_url(id: &str) -> String {
    format!("/api/assets/{id}")
}

#[derive(Debug, Clone)]
pub struct NewAsset {
    pub id: String,
    pub owner_id: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

impl NewAsset {
    pub fn new(owner_id: &str, mime: &str, bytes: Vec<u8>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            owner_id: owner_id.to_string(),
            mime: mime.to_string(),
            bytes,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AssetRecord {
    pub id: String,
    pub owner_id: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub created_at: String,
}

pub async fn insert_asset(pool: &PgPool, asset: &NewAsset) -> Result<String, sqlx::Error> {
    let now = OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default();
    sqlx::query(
        "INSERT INTO assets (id, owner_id, mime, bytes, created_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&asset.id)
    .bind(&asset.owner_id)
    .bind(&asset.mime)
    .bind(&asset.bytes)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(asset.id.clone())
}

pub async fn get_asset(
    pool: &PgPool,
    id: &str,
    owner_id: &str,
) -> Result<Option<AssetRecord>, sqlx::Error> {
    sqlx::query_as::<_, AssetRecord>(
        "SELECT id, owner_id, mime, bytes, created_at FROM assets WHERE id = $1 AND owner_id = $2",
    )
    .bind(id)
    .bind(owner_id)
    .fetch_optional(pool)
    .await
}
```

`crates/knowledge-server/src/assets/routes.rs` (GET is owner-scoped via cookie principal; no CSRF on GET since `<img>` can't send custom headers):

```rust
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::assets::store;
use crate::auth::principal::resolve_principal;
use crate::error::ApiError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/assets/:id", get(get_asset_handler))
}

async fn get_asset_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    let asset = store::get_asset(state.pool(), &id, &principal.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("asset not found"))?;
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, asset.mime)
        .header(header::CACHE_CONTROL, "private, max-age=31536000, immutable")
        .body(Body::from(asset.bytes))
        .map_err(|_| ApiError::internal("failed to build asset response"))?;
    Ok(response)
}
```

`crates/knowledge-server/src/assets/mod.rs`:

```rust
pub mod store;
pub mod routes;
```

Add `pub mod assets;` to `lib.rs`. Add `.merge(crate::assets::routes::router())` to `build_router`.

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server assets::store && cargo build -p knowledge-server`
Expected: PASS (2 tests) + clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/assets/ crates/knowledge-server/src/lib.rs crates/knowledge-server/src/http/router.rs
git commit -m "feat(assets): add unified asset store and GET /api/assets/:id"
```

---

# Phase 2 — Backend Capabilities (image gen, URL extract, AI runs)

## Task 6: Provider image-generation types + request serialization

**Files:**
- Modify: `crates/knowledge-server/src/providers/types.rs`

- [ ] **Step 1: Write the failing test (lock request VALUES, per repo style)**

Add to the `types.rs` test module (or create one). Mirror the existing serialization-value asserts in `openai_compatible.rs`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_image_request_holds_prompt() {
        let req = ProviderImageRequest { prompt: "a red fox".to_string() };
        assert_eq!(req.prompt, "a red fox");
    }

    #[test]
    fn provider_image_result_carries_mime_and_bytes() {
        let res = ProviderImageResult { mime: "image/png".to_string(), bytes: vec![9, 9, 9] };
        assert_eq!(res.mime, "image/png");
        assert_eq!(res.bytes, vec![9, 9, 9]);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server providers::types`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the types**

Add to `crates/knowledge-server/src/providers/types.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ProviderImageRequest {
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub struct ProviderImageResult {
    /// e.g. "image/png"
    pub mime: String,
    pub bytes: Vec<u8>,
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server providers::types`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/providers/types.rs
git commit -m "feat(providers): add image request/result types"
```

---

## Task 7: Provider generate_image (OpenAI-compatible images endpoint)

**Files:**
- Modify: `crates/knowledge-server/src/providers/openai_compatible.rs`

This is net-new: no upstream equivalent exists (upstream only has vision captioning). Build on the existing provider pattern.

- [ ] **Step 1: Write the failing test (request body VALUES + b64 decode via mock TCP)**

Two tests: (1) the request body serializes to the locked values; (2) `generate_image` against a mock images endpoint base64-decodes `data[0].b64_json`. Mirror `spawn_mock_*` from `web_search/provider.rs:440-568` and the URL-helper tests already in this file.

```rust
#[cfg(test)]
mod image_tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn image_generation_request_serializes_expected_values() {
        let body = ImageGenerationRequest {
            model: "gpt-image-1".to_string(),
            prompt: "a red fox".to_string(),
            n: 1,
            response_format: "b64_json".to_string(),
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["model"], "gpt-image-1");
        assert_eq!(json["prompt"], "a red fox");
        assert_eq!(json["n"], 1);
        assert_eq!(json["response_format"], "b64_json");
    }

    #[test]
    fn images_url_appends_v1_images_generations() {
        let provider = OpenAiCompatibleProvider::new(
            "https://api.example.com".to_string(),
            "key".to_string(),
            "gpt-image-1".to_string(),
            30,
        );
        assert_eq!(provider.images_url(), "https://api.example.com/v1/images/generations");
    }

    #[tokio::test]
    async fn generate_image_decodes_b64_payload() {
        let raw = vec![1u8, 2, 3, 4];
        let b64 = base64::engine::general_purpose::STANDARD.encode(&raw);
        let body = format!("{{\"data\":[{{\"b64_json\":\"{b64}\"}}]}}");
        let (handle, base) = spawn_mock_images_server(body).await;

        let provider = OpenAiCompatibleProvider::new(base, "key".to_string(), "gpt-image-1".to_string(), 30);
        let result = provider
            .generate_image(ProviderImageRequest { prompt: "x".to_string() })
            .await
            .expect("image generated");

        assert_eq!(result.mime, "image/png");
        assert_eq!(result.bytes, raw);
        handle.abort();
    }

    async fn spawn_mock_images_server(json_body: String) -> (tokio::task::JoinHandle<()>, String) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                let mut buf = [0u8; 2048];
                let _ = sock.read(&mut buf).await;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    json_body.len(),
                    json_body
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.flush().await;
            }
        });
        (handle, format!("http://{addr}"))
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server openai_compatible`
Expected: FAIL to compile.

- [ ] **Step 3: Implement generate_image + images_url**

Add to `crates/knowledge-server/src/providers/openai_compatible.rs`. Reuse `self.base_url`, `self.api_key`, `self.model`, `auth_headers()`, and the existing reqwest client construction. Use the workspace `base64` 0.22 crate.

```rust
use serde::Serialize;
use crate::providers::types::{ProviderImageRequest, ProviderImageResult};

#[derive(Debug, Serialize)]
pub struct ImageGenerationRequest {
    pub model: String,
    pub prompt: String,
    pub n: u8,
    pub response_format: String,
}

impl OpenAiCompatibleProvider {
    pub fn images_url(&self) -> String {
        // matches chat_completions_url/embeddings_url style: trim trailing slash, append /v1/...
        format!("{}/v1/images/generations", self.base_url.trim_end_matches('/'))
    }

    pub async fn generate_image(
        &self,
        request: ProviderImageRequest,
    ) -> Result<ProviderImageResult, ProviderError> {
        use base64::Engine;

        let body = ImageGenerationRequest {
            model: self.model.clone(),
            prompt: request.prompt,
            n: 1,
            response_format: "b64_json".to_string(),
        };

        let response = self
            .client
            .post(self.images_url())
            .headers(self.auth_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Request(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::Response(format!("image generation failed ({status}): {text}")));
        }

        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ProviderError::Response(e.to_string()))?;

        let b64 = payload
            .get("data")
            .and_then(|d| d.get(0))
            .and_then(|d| d.get("b64_json"))
            .and_then(|b| b.as_str())
            .ok_or_else(|| ProviderError::Response("missing data[0].b64_json".to_string()))?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| ProviderError::Response(format!("invalid base64: {e}")))?;

        Ok(ProviderImageResult { mime: "image/png".to_string(), bytes })
    }
}
```

> Note: match the real names — `ProviderError` variants (`Request`/`Response` are placeholders; copy the actual variants used by `stream_chat`), the struct field names (`base_url`, `api_key`, `model`, `client`), and `auth_headers()`. If the provider is accessed through a trait, also add `generate_image` to the trait in `providers/mod.rs` (or wherever the trait is defined) and to any other impls; otherwise call the concrete type. Confirm `base64` is in the crate's `Cargo.toml` deps (it is a workspace dep).

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server openai_compatible`
Expected: PASS (image_generation_request + images_url + generate_image decode).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/providers/openai_compatible.rs
git commit -m "feat(providers): add generate_image for OpenAI-compatible images endpoint"
```

---

## Task 8: URL fetch -> readability -> markdown

**Files:**
- Modify: root `Cargo.toml` (`[workspace.dependencies]` add `scraper`)
- Modify: `crates/knowledge-server/Cargo.toml` (add `scraper`)
- Create: `crates/knowledge-server/src/canvas/service.rs`
- Modify: `crates/knowledge-server/src/canvas/mod.rs` (uncomment `pub mod service;`)

Port the behavioral intent from `upstream_llm_wiki/extension/Readability.js` (main-content extraction) + `upstream_llm_wiki/extension/Turndown.js` (HTML->markdown). We implement a focused subset with `scraper`: strip script/style/nav/footer, then convert headings, paragraphs, links, lists, and code to markdown.

- [ ] **Step 1: Add the dependency**

Root `Cargo.toml` under `[workspace.dependencies]`:

```toml
scraper = "0.20"
```

`crates/knowledge-server/Cargo.toml` under `[dependencies]`:

```toml
scraper = { workspace = true }
```

Run: `cd /e/Projects/Js/knowledge && cargo build -p knowledge-server`
Expected: builds (dependency resolves). Pin the version to whatever resolves cleanly with the existing lockfile; if 0.20 conflicts, use the latest compatible.

- [ ] **Step 2: Write the failing test (deterministic fixture HTML)**

In `crates/knowledge-server/src/canvas/service.rs`:

```rust
#[cfg(test)]
mod url_tests {
    use super::*;

    #[test]
    fn html_to_markdown_extracts_title_and_text() {
        let html = r#"
            <html><head><title>Sample Page</title></head>
            <body>
              <nav>ignore me</nav>
              <article>
                <h1>Big Heading</h1>
                <p>First paragraph with a <a href="https://x.test">link</a>.</p>
                <h2>Sub</h2>
                <p>Second paragraph.</p>
              </article>
              <footer>footer junk</footer>
            </body></html>
        "#;
        let extracted = html_to_markdown(html);
        assert_eq!(extracted.title, "Sample Page");
        assert!(extracted.markdown.contains("# Big Heading"));
        assert!(extracted.markdown.contains("## Sub"));
        assert!(extracted.markdown.contains("First paragraph with a [link](https://x.test)."));
        assert!(extracted.markdown.contains("Second paragraph."));
        assert!(!extracted.markdown.contains("ignore me"));
        assert!(!extracted.markdown.contains("footer junk"));
    }

    #[test]
    fn html_to_markdown_falls_back_to_untitled() {
        let extracted = html_to_markdown("<html><body><p>hi</p></body></html>");
        assert_eq!(extracted.title, "Untitled");
        assert!(extracted.markdown.contains("hi"));
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::service::url_tests`
Expected: FAIL to compile.

- [ ] **Step 4: Implement html_to_markdown + fetch_url**

```rust
use scraper::{Html, Selector};

/// Reference: upstream_llm_wiki/extension/Readability.js (content extraction)
/// + upstream_llm_wiki/extension/Turndown.js (HTML -> markdown).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedPage {
    pub title: String,
    pub markdown: String,
}

pub fn html_to_markdown(html: &str) -> ExtractedPage {
    let doc = Html::parse_document(html);

    let title = doc
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|t| t.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Untitled".to_string());

    // Prefer <article>/<main> as the content root; fall back to <body>.
    let root_html = ["article", "main", "body"]
        .iter()
        .find_map(|sel| {
            doc.select(&Selector::parse(sel).unwrap())
                .next()
                .map(|el| el.html())
        })
        .unwrap_or_else(|| html.to_string());

    let root = Html::parse_fragment(&root_html);
    let mut out = String::new();

    // Walk top-level block elements in document order.
    let block_sel = Selector::parse("h1, h2, h3, h4, p, li, pre").unwrap();
    let skip_sel = Selector::parse("nav, footer, script, style, aside").unwrap();
    let skip: std::collections::HashSet<_> =
        root.select(&skip_sel).map(|e| e.id()).collect();

    for el in root.select(&block_sel) {
        // Skip elements inside nav/footer/etc.
        if el.ancestors().any(|a| {
            scraper::ElementRef::wrap(a)
                .map(|er| skip.contains(&er.id()))
                .unwrap_or(false)
        }) {
            continue;
        }
        let text = inline_markdown(el);
        if text.trim().is_empty() {
            continue;
        }
        let name = el.value().name();
        match name {
            "h1" => out.push_str(&format!("# {text}\n\n")),
            "h2" => out.push_str(&format!("## {text}\n\n")),
            "h3" => out.push_str(&format!("### {text}\n\n")),
            "h4" => out.push_str(&format!("#### {text}\n\n")),
            "li" => out.push_str(&format!("- {text}\n")),
            "pre" => out.push_str(&format!("```\n{text}\n```\n\n")),
            _ => out.push_str(&format!("{text}\n\n")),
        }
    }

    ExtractedPage { title, markdown: out.trim().to_string() }
}

/// Render an element's inline children, converting <a> to markdown links.
fn inline_markdown(el: scraper::ElementRef) -> String {
    let mut s = String::new();
    for child in el.children() {
        if let Some(text) = child.value().as_text() {
            s.push_str(text);
        } else if let Some(child_el) = scraper::ElementRef::wrap(child) {
            if child_el.value().name() == "a" {
                let href = child_el.value().attr("href").unwrap_or("");
                let label = child_el.text().collect::<String>();
                if href.is_empty() {
                    s.push_str(&label);
                } else {
                    s.push_str(&format!("[{label}]({href})"));
                }
            } else {
                s.push_str(&inline_markdown(child_el));
            }
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Fetch a URL and extract readable markdown. Uses the crate's reqwest client.
pub async fn fetch_url(client: &reqwest::Client, url: &str) -> Result<ExtractedPage, String> {
    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (compatible; KnowledgeCanvas/1.0)")
        .send()
        .await
        .map_err(|e| format!("fetch failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("fetch failed: HTTP {}", response.status()));
    }
    let html = response.text().await.map_err(|e| format!("read body failed: {e}"))?;
    Ok(html_to_markdown(&html))
}
```

Uncomment `pub mod service;` in `mod.rs`.

> Note: `html_to_markdown` is the unit under test and is fully deterministic. The `ancestors()`/`id()` skip approach assumes `scraper`'s ego-tree API; if the installed `scraper` version differs, simplify by removing nav/footer subtrees before selecting blocks (parse, collect ids to skip, filter). Keep the test assertions as the contract.

- [ ] **Step 5: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::service::url_tests`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/knowledge-server/Cargo.toml crates/knowledge-server/src/canvas/service.rs crates/knowledge-server/src/canvas/mod.rs
git commit -m "feat(canvas): add URL fetch and HTML-to-markdown extraction"
```

---

## Task 9: Canvas service — context assembly + skill helpers

**Files:**
- Modify: `crates/knowledge-server/src/canvas/service.rs`

Pure helpers the SSE handlers will call: build the AI prompt from referenced nodes, and convert web-search results to a markdown result-node body.

- [ ] **Step 1: Write the failing tests**

Append to `service.rs`:

```rust
#[cfg(test)]
mod context_tests {
    use super::*;
    use crate::canvas::document::{CanvasDocument, CanvasEdge, CanvasNode, Viewport};

    fn node_with(id: &str, ty: &str, data: serde_json::Value) -> CanvasNode {
        CanvasNode { id: id.into(), r#type: ty.into(), x: 0.0, y: 0.0, w: 280.0, h: 160.0, data }
    }

    #[test]
    fn collect_reference_blocks_includes_note_and_url_text() {
        let doc = CanvasDocument {
            nodes: vec![
                node_with("n1", "note", serde_json::json!({ "markdown": "note body" })),
                node_with("u1", "url", serde_json::json!({ "title": "Page", "markdown": "url body", "status": "ok" })),
                node_with("ai", "ai_analyze", serde_json::json!({ "prompt": "summarize" })),
            ],
            edges: vec![
                CanvasEdge { id: "e1".into(), source: "n1".into(), target: "ai".into() },
                CanvasEdge { id: "e2".into(), source: "u1".into(), target: "ai".into() },
            ],
            viewport: Viewport::default(),
        };
        let blocks = collect_reference_blocks(&doc, "ai");
        let joined = blocks.join("\n---\n");
        assert!(joined.contains("note body"));
        assert!(joined.contains("url body"));
        assert!(joined.contains("Page"));
    }

    #[test]
    fn collect_reference_blocks_lists_kb_nodes_for_rag() {
        let doc = CanvasDocument {
            nodes: vec![
                node_with("kb1", "kb", serde_json::json!({ "projectId": "p1", "projectName": "Docs" })),
                node_with("ai", "ai_analyze", serde_json::json!({ "prompt": "q" })),
            ],
            edges: vec![CanvasEdge { id: "e1".into(), source: "kb1".into(), target: "ai".into() }],
            viewport: Viewport::default(),
        };
        let kb_ids = referenced_kb_project_ids(&doc, "ai");
        assert_eq!(kb_ids, vec!["p1".to_string()]);
    }

    #[test]
    fn search_results_to_markdown_formats_entries() {
        let results = vec![
            SearchResultEntry { title: "Title A".into(), url: "https://a.test".into(), snippet: "snippet a".into() },
            SearchResultEntry { title: "Title B".into(), url: "https://b.test".into(), snippet: "snippet b".into() },
        ];
        let md = search_results_to_markdown("rust async", &results);
        assert!(md.contains("rust async"));
        assert!(md.contains("[Title A](https://a.test)"));
        assert!(md.contains("snippet a"));
        assert!(md.contains("[Title B](https://b.test)"));
    }

    #[test]
    fn build_analyze_prompt_combines_prompt_and_blocks() {
        let prompt = build_analyze_prompt("Summarize the sources", &["block one".into(), "block two".into()]);
        assert!(prompt.contains("Summarize the sources"));
        assert!(prompt.contains("block one"));
        assert!(prompt.contains("block two"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::service::context_tests`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the helpers**

Append to `service.rs`:

```rust
use crate::canvas::document::CanvasDocument;

#[derive(Debug, Clone)]
pub struct SearchResultEntry {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Text blocks from incoming Note/URL nodes (KB handled separately via RAG).
pub fn collect_reference_blocks(doc: &CanvasDocument, node_id: &str) -> Vec<String> {
    let sources = doc.incoming_source_ids(node_id);
    let mut blocks = Vec::new();
    for src in sources {
        let Some(node) = doc.node(&src) else { continue };
        match node.r#type.as_str() {
            "note" => {
                if let Some(md) = node.data.get("markdown").and_then(|v| v.as_str()) {
                    if !md.trim().is_empty() {
                        blocks.push(format!("Note:\n{md}"));
                    }
                }
            }
            "url" => {
                let title = node.data.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let md = node.data.get("markdown").and_then(|v| v.as_str()).unwrap_or("");
                if !md.trim().is_empty() {
                    blocks.push(format!("Web page: {title}\n{md}"));
                }
            }
            "ai_analyze" => {
                if let Some(content) = crate::canvas::document::active_version_content(&node.data) {
                    blocks.push(format!("Prior analysis:\n{content}"));
                }
            }
            _ => {}
        }
    }
    blocks
}

/// Project ids of incoming KB nodes, for RAG retrieval at run time.
pub fn referenced_kb_project_ids(doc: &CanvasDocument, node_id: &str) -> Vec<String> {
    doc.incoming_source_ids(node_id)
        .into_iter()
        .filter_map(|src| doc.node(&src).cloned())
        .filter(|n| n.r#type == "kb")
        .filter_map(|n| n.data.get("projectId").and_then(|v| v.as_str()).map(String::from))
        .collect()
}

pub fn search_results_to_markdown(query: &str, results: &[SearchResultEntry]) -> String {
    let mut out = format!("Search results for \"{query}\":\n\n");
    for r in results {
        out.push_str(&format!("- [{}]({})\n  {}\n", r.title, r.url, r.snippet));
    }
    out.trim_end().to_string()
}

pub fn build_analyze_prompt(node_prompt: &str, blocks: &[String]) -> String {
    let mut prompt = String::new();
    if !blocks.is_empty() {
        prompt.push_str("Use the following referenced sources to answer.\n\n");
        for (i, b) in blocks.iter().enumerate() {
            prompt.push_str(&format!("--- Source {} ---\n{}\n\n", i + 1, b));
        }
    }
    prompt.push_str("Task:\n");
    prompt.push_str(node_prompt);
    prompt
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::service::context_tests`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/service.rs
git commit -m "feat(canvas): add reference-context assembly and skill helpers"
```

---

## Task 10: Node-run SSE + canvas-chat SSE + URL-extract route

**Files:**
- Modify: `crates/knowledge-server/src/canvas/routes.rs`

Wire the SSE endpoints. Mirror the SSE handler structure in `crates/knowledge-server/src/chat/routes.rs:141-287` (`async_stream::stream!`, `Event::default().event("delta"|"done"|"error")`, `.keep_alive(KeepAlive::default())`). The server NEVER writes the document — on `done` it returns the new content/node and the client autosaves.

- [ ] **Step 1: Write the failing test (done-payload shapes are pure)**

The streaming I/O is integration-level; unit-test the pure payload builders. Add to the `routes.rs` test module:

```rust
#[test]
fn analyze_done_payload_carries_version_fields() {
    let payload = build_analyze_done_payload("v-new", "the answer", "2026-06-30T00:00:00Z");
    assert_eq!(payload["versionId"], "v-new");
    assert_eq!(payload["content"], "the answer");
    assert_eq!(payload["createdAt"], "2026-06-30T00:00:00Z");
}

#[test]
fn skill_node_done_payload_describes_new_node() {
    let node = serde_json::json!({ "type": "url", "data": { "title": "T" } });
    let payload = build_skill_node_done_payload(node.clone(), 120.0, 240.0);
    assert_eq!(payload["node"]["type"], "url");
    assert_eq!(payload["x"], 120.0);
    assert_eq!(payload["y"], 240.0);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::routes`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the payload builders + SSE handlers + routes**

Add the pure builders:

```rust
pub fn build_analyze_done_payload(version_id: &str, content: &str, created_at: &str) -> serde_json::Value {
    serde_json::json!({ "versionId": version_id, "content": content, "createdAt": created_at })
}

pub fn build_skill_node_done_payload(node: serde_json::Value, x: f64, y: f64) -> serde_json::Value {
    serde_json::json!({ "node": node, "x": x, "y": y })
}
```

Add routes to the `router()` builder from Task 4:

```rust
.route("/api/canvases/:id/nodes/:node_id/run", post(run_node_handler))
.route("/api/canvases/:id/chat", post(chat_handler))
.route("/api/canvas/extract-url", post(extract_url_handler))
```

Sketch of the node-run SSE handler (fill in using the chat/routes.rs pattern for the exact `Sse`/`stream!` types and the provider call):

```rust
async fn run_node_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, node_id)): Path<(String, String)>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;

    let rec = store::get_canvas(state.pool(), &id, &principal.user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("canvas not found"))?;
    let doc = rec.parse_document().map_err(|_| ApiError::internal("corrupt document"))?;
    let node = doc.node(&node_id).cloned().ok_or_else(|| ApiError::not_found("node not found"))?;

    // Gather Note/URL/prior-analysis blocks.
    let mut blocks = crate::canvas::service::collect_reference_blocks(&doc, &node_id);

    // KB nodes -> RAG, with per-project permission check; excluded if no access.
    for project_id in crate::canvas::service::referenced_kb_project_ids(&doc, &node_id) {
        match crate::tenancy::access::project_access_role(&state, &principal, &project_id).await {
            Ok(Some(_role)) => {
                let root = crate::projects::service::project_root_for_id(&state, &project_id).await?;
                let node_prompt = node.data.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                let assembled = crate::chat::context::assemble_chat_context(
                    &state, &project_id, &root, node_prompt, 8,
                ).await?;
                for b in assembled.context_blocks {
                    blocks.push(format!("Knowledge base ({project_id}):\n{b}"));
                }
            }
            _ => {
                // No access (revoked or never granted): exclude, optionally note it.
                blocks.push(format!("[Knowledge base {project_id}: no access, excluded]"));
            }
        }
    }

    let node_prompt = node.data.get("prompt").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let prompt = crate::canvas::service::build_analyze_prompt(&node_prompt, &blocks);

    // Stream via provider.stream_chat, emitting delta/done/error exactly like chat/routes.rs.
    // On done: build_analyze_done_payload(new_version_id, full_text, now_rfc3339()).
    // (Copy the concrete Sse/stream! plumbing and provider acquisition from chat/routes.rs.)
    todo!("assemble stream per chat/routes.rs; emit delta/done/error")
}
```

The `chat_handler` parses the message: if it starts with `/search`, `/image`, `/analyze`, `/kb`, dispatch the skill; otherwise stream a plain answer over the board (or selected nodes passed in the body). On a skill that creates a node, the `done` payload uses `build_skill_node_done_payload`:
- `/search <q>` -> call `crate::web_search` -> map hits into `SearchResultEntry` -> `search_results_to_markdown` -> done payload with a `url`-less "search result" note node (`type: "note"`, `data.markdown`).
- `/image <prompt>` -> `provider.generate_image` -> `assets::store::insert_asset` -> done payload with an `ai_image` node carrying `assetId` + `asset_url(id)`.
- `/analyze` -> build an `ai_analyze` node referencing selected node ids (client wires edges) -> done payload.
- `/kb` is handled entirely client-side (project picker); no server call.

The `extract_url_handler`:

```rust
#[derive(serde::Deserialize)]
struct ExtractUrlRequest { url: String }

async fn extract_url_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ExtractUrlRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let principal = resolve_principal(&state, &headers).await?;
    require_csrf(&principal, &headers)?;
    let _ = principal; // owner scoping not needed; just auth
    match crate::canvas::service::fetch_url(state.http_client(), &body.url).await {
        Ok(page) => Ok(Json(serde_json::json!({
            "status": "ok", "title": page.title, "markdown": page.markdown, "error": null
        }))),
        Err(e) => Ok(Json(serde_json::json!({
            "status": "error", "title": "", "markdown": "", "error": e
        }))),
    }
}
```

> Note: copy the EXACT SSE plumbing (`Sse`, `KeepAlive`, `async_stream::stream!`, provider acquisition, error mapping) from `chat/routes.rs:141-287` — do not invent it. Confirm helper names: `crate::tenancy::access::project_access_role`, `crate::projects::service::project_root_for_id`, `crate::chat::context::assemble_chat_context` (and `AssembledContext.context_blocks`), `state.http_client()` (or the real accessor for the shared reqwest client), and the web_search entry function. The two pure payload builders are the only TDD-covered units here; the streaming wiring is verified by build + manual/integration.

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server canvas::routes && cargo build -p knowledge-server`
Expected: PASS (payload-builder tests) + clean build (replace the `todo!()` with the real stream before build passes).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs
git commit -m "feat(canvas): add node-run SSE, canvas chat SSE, and URL extract route"
```

---

# Phase 3 — Frontend (admin canvas UI)

## Task 11: Install @xyflow/react + register route and nav

**Files:**
- Modify: `apps/admin/package.json` (dependency)
- Modify: `apps/admin/src/lib/route-meta.ts` (globalNav entry)
- Modify: `apps/admin/src/app/router.tsx` (lazy routes)
- Modify: `apps/admin/src/app/router.test.tsx` (assert canvas route renders)

- [ ] **Step 1: Install the dependency**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm i @xyflow/react`
Expected: adds `@xyflow/react` to `apps/admin/package.json`. The CSS import `@xyflow/react/dist/style.css` will be added in the board component (Task 16).

- [ ] **Step 2: Add the failing test**

In `apps/admin/src/app/router.test.tsx`, follow the existing mock-pages pattern (every page is mocked). Add a mock + a test that navigating to `/canvas` renders the canvas page stub.

```tsx
vi.mock("../features/canvas/page", () => ({
  CanvasPage: () => <div>Canvas Page</div>,
}));

it("renders the canvas route", async () => {
  renderAt("/canvas"); // use the file's existing render helper
  expect(await screen.findByText("Canvas Page")).toBeInTheDocument();
});
```

- [ ] **Step 3: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- router`
Expected: FAIL (no `/canvas` route; `CanvasPage` import unresolved).

- [ ] **Step 4: Register nav + routes**

`apps/admin/src/lib/route-meta.ts` — add the Canvas entry to `globalNav` (after Projects so it reads Dashboard / Projects / Canvas):

```ts
export const globalNav = [
  { to: "/", label: "Dashboard" },
  { to: "/projects", label: "Projects" },
  { to: "/canvas", label: "Canvas" },
  { to: "/users", label: "Users" },
  { to: "/api-tokens", label: "API Tokens" },
  { to: "/settings", label: "Settings" },
] as const;
```

`apps/admin/src/app/router.tsx` — add the lazy import (mirror the `ChatPage` lazy pattern) and two routes under the top-level `<Route path="/" element={<AppShell />}>`, siblings of `projects`:

```tsx
const CanvasPage = lazy(() =>
  import("../features/canvas/page").then((m) => ({ default: m.CanvasPage })),
);
```

```tsx
<Route
  path="canvas"
  element={
    <Suspense fallback={<RouteFallback />}>
      <CanvasPage />
    </Suspense>
  }
/>
<Route
  path="canvas/:canvasId"
  element={
    <Suspense fallback={<RouteFallback />}>
      <CanvasPage />
    </Suspense>
  }
/>
```

Use the existing fallback component name from the file (e.g. `RouteFallback` or the inline `<Suspense fallback=...>` the file already uses — match it).

> Note: `CanvasPage` doesn't exist until Task 18. To keep the build green between tasks, create a minimal stub now: `apps/admin/src/features/canvas/page.tsx` exporting `export function CanvasPage() { return <div>Canvas Page</div>; }`. Task 18 replaces it.

- [ ] **Step 5: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- router`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/package.json apps/admin/package-lock.json apps/admin/src/lib/route-meta.ts apps/admin/src/app/router.tsx apps/admin/src/app/router.test.tsx apps/admin/src/features/canvas/page.tsx
git commit -m "feat(canvas): register /canvas route, nav entry, and xyflow dependency"
```

---

## Task 12: Canvas types + Zod schemas

**Files:**
- Create: `apps/admin/src/features/canvas/types.ts`
- Create: `apps/admin/src/features/canvas/types.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, expect, it } from "vitest";
import { canvasDocumentSchema, canvasResponseSchema } from "./types";

describe("canvas schemas", () => {
  it("parses a minimal document", () => {
    const doc = canvasDocumentSchema.parse({
      nodes: [],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    });
    expect(doc.nodes).toEqual([]);
    expect(doc.viewport.zoom).toBe(1);
  });

  it("parses a canvas response with a note node", () => {
    const res = canvasResponseSchema.parse({
      id: "c1",
      title: "Board",
      document: {
        nodes: [{ id: "n1", type: "note", x: 0, y: 0, w: 280, h: 160, data: { markdown: "hi" } }],
        edges: [],
        viewport: { x: 0, y: 0, zoom: 1 },
      },
      created_at: "t1",
      updated_at: "t2",
    });
    expect(res.document.nodes[0].type).toBe("note");
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/types`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement the schemas**

```ts
import { z } from "zod";

export const canvasNodeSchema = z.object({
  id: z.string(),
  type: z.enum(["note", "url", "kb", "ai_analyze", "ai_image"]),
  x: z.number(),
  y: z.number(),
  w: z.number().default(280),
  h: z.number().default(160),
  data: z.record(z.unknown()).default({}),
});
export type CanvasNode = z.infer<typeof canvasNodeSchema>;

export const canvasEdgeSchema = z.object({
  id: z.string(),
  source: z.string(),
  target: z.string(),
});
export type CanvasEdge = z.infer<typeof canvasEdgeSchema>;

export const viewportSchema = z.object({ x: z.number(), y: z.number(), zoom: z.number() });

export const canvasDocumentSchema = z.object({
  nodes: z.array(canvasNodeSchema).default([]),
  edges: z.array(canvasEdgeSchema).default([]),
  viewport: viewportSchema.default({ x: 0, y: 0, zoom: 1 }),
});
export type CanvasDocument = z.infer<typeof canvasDocumentSchema>;

export const canvasResponseSchema = z.object({
  id: z.string(),
  title: z.string(),
  document: canvasDocumentSchema,
  created_at: z.string(),
  updated_at: z.string(),
});
export type CanvasResponse = z.infer<typeof canvasResponseSchema>;

export const canvasSummarySchema = z.object({
  id: z.string(),
  title: z.string(),
  updated_at: z.string(),
});
export type CanvasSummary = z.infer<typeof canvasSummarySchema>;
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/types`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/types.ts apps/admin/src/features/canvas/types.test.ts
git commit -m "feat(canvas): add canvas TS types and Zod schemas"
```

---

## Task 13: Canvas REST API + queries

**Files:**
- Create: `apps/admin/src/features/canvas/api.ts`
- Create: `apps/admin/src/features/canvas/queries.ts`
- Create: `apps/admin/src/features/canvas/api.test.ts`

- [ ] **Step 1: Write the failing test (mock fetch via vi.stubGlobal)**

Mirror `features/shared/api.ts` usage (`apiFetch(path, init, schema)`, `csrfHeader()`).

```ts
import { afterEach, describe, expect, it, vi } from "vitest";
import { listCanvases, saveCanvas } from "./api";

function jsonResponse(body: unknown) {
  return Promise.resolve(new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  }));
}

afterEach(() => vi.unstubAllGlobals());

describe("canvas api", () => {
  it("listCanvases returns summaries", async () => {
    vi.stubGlobal("fetch", vi.fn(() => jsonResponse([{ id: "c1", title: "A", updated_at: "t" }])));
    const list = await listCanvases();
    expect(list[0].id).toBe("c1");
  });

  it("saveCanvas PUTs title + document and sends csrf header", async () => {
    const fetchMock = vi.fn(() =>
      jsonResponse({
        id: "c1",
        title: "B",
        document: { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } },
        created_at: "t1",
        updated_at: "t2",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);
    await saveCanvas("c1", { title: "B", document: { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } } });
    const [url, init] = fetchMock.mock.calls[0];
    expect(String(url)).toContain("/api/canvases/c1");
    expect(init.method).toBe("PUT");
    expect(init.headers["x-csrf-token"]).toBeDefined();
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/api`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement api + queries**

`api.ts` (match the repo's `apiFetch`/`csrfHeader` import paths — copy from `features/shared/api.ts`):

```ts
import { apiFetch } from "@/features/shared/api";
import { csrfHeader } from "@/features/shared/api";
import {
  canvasDocumentSchema,
  canvasResponseSchema,
  canvasSummarySchema,
  type CanvasDocument,
  type CanvasResponse,
  type CanvasSummary,
} from "./types";
import { z } from "zod";

export function listCanvases(): Promise<CanvasSummary[]> {
  return apiFetch("/api/canvases", { credentials: "include" }, z.array(canvasSummarySchema));
}

export function createCanvas(title?: string): Promise<CanvasResponse> {
  return apiFetch(
    "/api/canvases",
    {
      method: "POST",
      credentials: "include",
      headers: { "content-type": "application/json", ...csrfHeader() },
      body: JSON.stringify({ title }),
    },
    canvasResponseSchema,
  );
}

export function getCanvas(id: string): Promise<CanvasResponse> {
  return apiFetch(`/api/canvases/${id}`, { credentials: "include" }, canvasResponseSchema);
}

export function saveCanvas(
  id: string,
  body: { title: string; document: CanvasDocument },
): Promise<CanvasResponse> {
  return apiFetch(
    `/api/canvases/${id}`,
    {
      method: "PUT",
      credentials: "include",
      headers: { "content-type": "application/json", ...csrfHeader() },
      body: JSON.stringify(body),
    },
    canvasResponseSchema,
  );
}

export function deleteCanvas(id: string): Promise<{ deleted: boolean }> {
  return apiFetch(
    `/api/canvases/${id}`,
    { method: "DELETE", credentials: "include", headers: { ...csrfHeader() } },
    z.object({ deleted: z.boolean() }),
  );
}

const extractResultSchema = z.object({
  status: z.enum(["ok", "error"]),
  title: z.string(),
  markdown: z.string(),
  error: z.string().nullable(),
});

export function extractUrl(url: string) {
  return apiFetch(
    "/api/canvas/extract-url",
    {
      method: "POST",
      credentials: "include",
      headers: { "content-type": "application/json", ...csrfHeader() },
      body: JSON.stringify({ url }),
    },
    extractResultSchema,
  );
}
```

`queries.ts` (mirror `features/chat/queries.ts`):

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createCanvas, deleteCanvas, getCanvas, listCanvases, saveCanvas } from "./api";
import type { CanvasDocument } from "./types";

export const canvasKeys = {
  all: ["canvases"] as const,
  detail: (id: string) => ["canvases", id] as const,
};

export function useCanvasList() {
  return useQuery({ queryKey: canvasKeys.all, queryFn: listCanvases });
}

export function useCanvas(id: string | undefined) {
  return useQuery({
    queryKey: id ? canvasKeys.detail(id) : canvasKeys.all,
    queryFn: () => getCanvas(id as string),
    enabled: Boolean(id),
  });
}

export function useCreateCanvas() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (title?: string) => createCanvas(title),
    onSuccess: () => qc.invalidateQueries({ queryKey: canvasKeys.all }),
  });
}

export function useSaveCanvas(id: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { title: string; document: CanvasDocument }) => saveCanvas(id, body),
    onSuccess: (data) => qc.setQueryData(canvasKeys.detail(id), data),
  });
}

export function useDeleteCanvas() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteCanvas(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: canvasKeys.all }),
  });
}
```

> Note: confirm `apiFetch`'s exact signature and whether `csrfHeader` lives in `features/shared/api.ts` or `features/auth/csrf.ts` (CSRF token is in localStorage key `knowledge.csrfToken`). Match the import paths and the `headers` shape the test asserts.

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/api`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/api.ts apps/admin/src/features/canvas/queries.ts apps/admin/src/features/canvas/api.test.ts
git commit -m "feat(canvas): add canvas REST api and query hooks"
```

---

## Task 14: SSE stream clients (node run + canvas chat)

**Files:**
- Create: `apps/admin/src/features/canvas/stream.ts`
- Create: `apps/admin/src/features/canvas/stream.test.ts`

Mirror `apps/admin/src/features/chat/stream.ts` (`parseSseBuffer`, fetch reader/decoder loop, `credentials:"include"`, `x-csrf-token`). Chat adds an `onNode` callback for skill-result nodes.

- [ ] **Step 1: Write the failing test (reuse the chat stream test approach)**

```ts
import { describe, expect, it, vi } from "vitest";
import { runCanvasNode } from "./stream";

function sseStream(chunks: string[]) {
  return new ReadableStream({
    start(controller) {
      const enc = new TextEncoder();
      for (const c of chunks) controller.enqueue(enc.encode(c));
      controller.close();
    },
  });
}

describe("runCanvasNode", () => {
  it("emits deltas then done", async () => {
    const body = sseStream([
      'event: delta\ndata: {"text":"Hel"}\n\n',
      'event: delta\ndata: {"text":"lo"}\n\n',
      'event: done\ndata: {"versionId":"v1","content":"Hello","createdAt":"t"}\n\n',
    ]);
    vi.stubGlobal("fetch", vi.fn(() => Promise.resolve(new Response(body, { status: 200 }))));

    const deltas: string[] = [];
    let done: unknown = null;
    await runCanvasNode("c1", "n1", {
      onDelta: (t) => deltas.push(t),
      onDone: (p) => (done = p),
      onError: () => {},
    });
    expect(deltas.join("")).toBe("Hello");
    expect(done).toEqual({ versionId: "v1", content: "Hello", createdAt: "t" });
    vi.unstubAllGlobals();
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/stream`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement the stream clients**

Copy `parseSseBuffer` + the reader loop from `features/chat/stream.ts` (do not re-derive). Then:

```ts
import { getCsrfToken } from "@/features/auth/csrf";
// import { parseSseBuffer } from the shared location, or inline-copy from chat/stream.ts

export interface NodeRunHandlers {
  onDelta: (text: string) => void;
  onDone: (payload: { versionId: string; content: string; createdAt: string }) => void;
  onError: (message: string) => void;
}

export async function runCanvasNode(
  canvasId: string,
  nodeId: string,
  handlers: NodeRunHandlers,
): Promise<void> {
  const res = await fetch(`/api/canvases/${canvasId}/nodes/${nodeId}/run`, {
    method: "POST",
    credentials: "include",
    headers: { "x-csrf-token": getCsrfToken() ?? "" },
  });
  await consumeSse(res, handlers);
}

export interface ChatHandlers {
  onDelta: (text: string) => void;
  onNode?: (payload: { node: unknown; x: number; y: number }) => void;
  onDone: (payload: unknown) => void;
  onError: (message: string) => void;
}

export async function streamCanvasChat(
  canvasId: string,
  message: string,
  selectedNodeIds: string[],
  handlers: ChatHandlers,
): Promise<void> {
  const res = await fetch(`/api/canvases/${canvasId}/chat`, {
    method: "POST",
    credentials: "include",
    headers: { "content-type": "application/json", "x-csrf-token": getCsrfToken() ?? "" },
    body: JSON.stringify({ message, selectedNodeIds }),
  });
  await consumeSse(res, handlers);
}

// consumeSse: read res.body, decode, parseSseBuffer, dispatch by event name
// (delta -> onDelta, node -> onNode, done -> onDone, error -> onError).
// Copy the loop body verbatim from features/chat/stream.ts and switch on event type.
async function consumeSse(res: Response, handlers: NodeRunHandlers | ChatHandlers): Promise<void> {
  // ... mirror chat/stream.ts reader/decoder/parseSseBuffer loop ...
}
```

> Note: open `features/chat/stream.ts` and copy `parseSseBuffer` + the exact reader/decoder loop into `consumeSse`. Confirm `getCsrfToken`'s import path (`features/auth/csrf.ts`, localStorage key `knowledge.csrfToken`). Dispatch on the SSE event name; the test only exercises `delta`/`done`.

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/stream`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/stream.ts apps/admin/src/features/canvas/stream.test.ts
git commit -m "feat(canvas): add SSE clients for node run and canvas chat"
```

---

## Task 15: Autosave hook with debounce + save status

**Files:**
- Create: `apps/admin/src/features/canvas/use-autosave.ts`
- Create: `apps/admin/src/features/canvas/use-autosave.test.ts`

- [ ] **Step 1: Write the failing test (fake timers; one PUT after rapid edits)**

```ts
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAutosave } from "./use-autosave";

describe("useAutosave", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("fires a single save after rapid edits settle", async () => {
    const save = vi.fn(() => Promise.resolve());
    const { result, rerender } = renderHook(
      ({ doc }) => useAutosave({ value: doc, delayMs: 500, onSave: save }),
      { initialProps: { doc: { v: 0 } } },
    );

    for (let v = 1; v <= 5; v++) {
      rerender({ doc: { v } });
      act(() => { vi.advanceTimersByTime(100); });
    }
    expect(save).not.toHaveBeenCalled();
    await act(async () => { vi.advanceTimersByTime(500); });
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith({ v: 5 });
    expect(result.current.status).toBe("saved");
  });

  it("reports save-failed on rejection", async () => {
    const save = vi.fn(() => Promise.reject(new Error("nope")));
    const { result, rerender } = renderHook(
      ({ doc }) => useAutosave({ value: doc, delayMs: 300, onSave: save }),
      { initialProps: { doc: { v: 0 } } },
    );
    rerender({ doc: { v: 1 } });
    await act(async () => { vi.advanceTimersByTime(300); });
    expect(result.current.status).toBe("error");
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/use-autosave`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement the hook**

```ts
import { useEffect, useRef, useState } from "react";

export type SaveStatus = "idle" | "pending" | "saving" | "saved" | "error";

interface UseAutosaveOptions<T> {
  value: T;
  delayMs: number;
  onSave: (value: T) => Promise<unknown>;
}

export function useAutosave<T>({ value, delayMs, onSave }: UseAutosaveOptions<T>) {
  const [status, setStatus] = useState<SaveStatus>("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const first = useRef(true);
  const latest = useRef(value);
  latest.current = value;

  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    setStatus("pending");
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      setStatus("saving");
      onSave(latest.current)
        .then(() => setStatus("saved"))
        .catch(() => setStatus("error"));
    }, delayMs);
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, [value, delayMs, onSave]);

  return { status };
}
```

> Note: the test passes `onSave` as a stable `vi.fn`; in the page, wrap the real save in `useCallback` so the effect doesn't re-fire on every render. A manual "retry" calls `onSave(latest.current)` directly with backoff handled by the caller.

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/use-autosave`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/use-autosave.ts apps/admin/src/features/canvas/use-autosave.test.ts
git commit -m "feat(canvas): add debounced autosave hook with save status"
```

---

## Task 16: Node components (5 types) + version switcher

**Files:**
- Create: `apps/admin/src/features/canvas/node-types/note.tsx`
- Create: `apps/admin/src/features/canvas/node-types/url.tsx`
- Create: `apps/admin/src/features/canvas/node-types/kb.tsx`
- Create: `apps/admin/src/features/canvas/node-types/ai-analyze.tsx`
- Create: `apps/admin/src/features/canvas/node-types/ai-image.tsx`
- Create: `apps/admin/src/features/canvas/node-types/version-switcher.tsx`
- Create: `apps/admin/src/features/canvas/node-types/version-switcher.test.tsx`

The version switcher is the only logic-bearing piece worth a unit test; the node shells are presentational (shadcn primitives + `markdown-message.tsx`).

- [ ] **Step 1: Write the failing test (version switcher navigation)**

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { VersionSwitcher } from "./version-switcher";

describe("VersionSwitcher", () => {
  it("shows position and navigates between versions", () => {
    const onChange = vi.fn();
    render(
      <VersionSwitcher
        versions={[{ id: "v1" }, { id: "v2" }, { id: "v3" }]}
        activeId="v2"
        onChange={onChange}
      />,
    );
    expect(screen.getByText("2/3")).toBeInTheDocument();
    fireEvent.click(screen.getByLabelText("previous version"));
    expect(onChange).toHaveBeenCalledWith("v1");
    fireEvent.click(screen.getByLabelText("next version"));
    expect(onChange).toHaveBeenCalledWith("v3");
  });

  it("disables prev at first and next at last", () => {
    render(
      <VersionSwitcher versions={[{ id: "v1" }, { id: "v2" }]} activeId="v1" onChange={() => {}} />,
    );
    expect(screen.getByLabelText("previous version")).toBeDisabled();
    expect(screen.getByLabelText("next version")).not.toBeDisabled();
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/node-types/version-switcher`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement the version switcher (and the node shells)**

`version-switcher.tsx` (compose shadcn `Button`):

```tsx
import { Button } from "@/components/ui/button";
import { ChevronLeft, ChevronRight } from "lucide-react";

interface Version {
  id: string;
}

interface VersionSwitcherProps {
  versions: Version[];
  activeId: string;
  onChange: (id: string) => void;
}

export function VersionSwitcher({ versions, activeId, onChange }: VersionSwitcherProps) {
  const index = Math.max(0, versions.findIndex((v) => v.id === activeId));
  const atStart = index <= 0;
  const atEnd = index >= versions.length - 1;
  return (
    <div className="flex items-center gap-1 font-mono text-xs">
      <Button
        type="button"
        variant="ghost"
        size="icon"
        aria-label="previous version"
        disabled={atStart}
        onClick={() => onChange(versions[index - 1].id)}
      >
        <ChevronLeft className="size-3" />
      </Button>
      <span>{`${index + 1}/${versions.length}`}</span>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        aria-label="next version"
        disabled={atEnd}
        onClick={() => onChange(versions[index + 1].id)}
      >
        <ChevronRight className="size-3" />
      </Button>
    </div>
  );
}
```

Node shells — each is a `@xyflow/react` custom node. Add `import "@xyflow/react/dist/style.css";` once in `canvas-board.tsx` (Task 17), not here. Each node uses shadcn primitives and renders content through `@/components/shared/markdown-message`. Implement:
- `note.tsx`: a `Textarea` bound to `data.markdown`; updates flow to the board via the node's `onChange` callback (board owns document state).
- `url.tsx`: an `Input` for the URL + a fetch button; shows `data.title`/markdown; on `status === "error"` shows the message + a "retry" `Button`.
- `kb.tsx`: shows `data.projectName`; a badge if the run flagged "no access".
- `ai-analyze.tsx`: header with `AI · Analyze` + `<VersionSwitcher>`; body renders the active version through `markdown-message`; a "rerun" `Button`; `status === "running"` shows a spinner; `status === "error"` shows the error and preserves the prior version.
- `ai-image.tsx`: header + `<VersionSwitcher>`; body renders `<img src={activeVersion.assetUrl}>`; a "regenerate" `Button`; running/error states.

> Note: keep node shells presentational and stateless beyond local input; the board (Task 17) owns the document and passes `data` + change/run handlers. Use existing shadcn components under `@/components/ui/*` (Button, Textarea, Input, Badge, Tooltip). Confirm `markdown-message`'s export name from `components/shared/markdown-message.tsx`.

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/node-types/version-switcher`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/node-types/
git commit -m "feat(canvas): add node components and version switcher"
```

---

## Task 17: Canvas board (xyflow wrapper)

**Files:**
- Create: `apps/admin/src/features/canvas/canvas-board.tsx`
- Create: `apps/admin/src/features/canvas/canvas-board.test.tsx`

The board maps our `CanvasDocument` to React Flow nodes/edges, registers the custom node types, and reports edits up (the page owns autosave). React Flow is heavy/canvas-based; mock it in the test and assert our mapping.

- [ ] **Step 1: Write the failing test (mock @xyflow/react)**

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@xyflow/react", () => ({
  ReactFlow: ({ nodes }: { nodes: Array<{ id: string }> }) => (
    <div data-testid="rf">{nodes.map((n) => <span key={n.id}>{n.id}</span>)}</div>
  ),
  Background: () => null,
  Controls: () => null,
  ReactFlowProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  applyNodeChanges: (_: unknown, nodes: unknown) => nodes,
  applyEdgeChanges: (_: unknown, edges: unknown) => edges,
  addEdge: (_: unknown, edges: unknown) => edges,
}));

import { CanvasBoard } from "./canvas-board";

describe("CanvasBoard", () => {
  it("renders a node per document node", () => {
    render(
      <CanvasBoard
        document={{
          nodes: [
            { id: "n1", type: "note", x: 0, y: 0, w: 280, h: 160, data: { markdown: "" } },
            { id: "n2", type: "note", x: 0, y: 0, w: 280, h: 160, data: { markdown: "" } },
          ],
          edges: [],
          viewport: { x: 0, y: 0, zoom: 1 },
        }}
        onChange={() => {}}
        onRunNode={() => {}}
      />,
    );
    expect(screen.getByText("n1")).toBeInTheDocument();
    expect(screen.getByText("n2")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/canvas-board`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement the board**

```tsx
import "@xyflow/react/dist/style.css";
import {
  Background,
  Controls,
  ReactFlow,
  ReactFlowProvider,
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
} from "@xyflow/react";
import { useCallback, useMemo } from "react";
import type { CanvasDocument } from "./types";
import { NoteNode } from "./node-types/note";
import { UrlNode } from "./node-types/url";
import { KbNode } from "./node-types/kb";
import { AiAnalyzeNode } from "./node-types/ai-analyze";
import { AiImageNode } from "./node-types/ai-image";

interface CanvasBoardProps {
  document: CanvasDocument;
  onChange: (next: CanvasDocument) => void;
  onRunNode: (nodeId: string) => void;
}

const nodeTypes = {
  note: NoteNode,
  url: UrlNode,
  kb: KbNode,
  ai_analyze: AiAnalyzeNode,
  ai_image: AiImageNode,
};

export function CanvasBoard({ document, onChange, onRunNode }: CanvasBoardProps) {
  const rfNodes = useMemo(
    () =>
      document.nodes.map((n) => ({
        id: n.id,
        type: n.type,
        position: { x: n.x, y: n.y },
        data: { ...n.data, onRunNode },
      })),
    [document.nodes, onRunNode],
  );
  const rfEdges = useMemo(
    () => document.edges.map((e) => ({ id: e.id, source: e.source, target: e.target })),
    [document.edges],
  );

  const onNodesChange = useCallback(
    (changes: unknown) => {
      const next = applyNodeChanges(changes, rfNodes as never) as typeof rfNodes;
      onChange({
        ...document,
        nodes: next.map((n) => {
          const orig = document.nodes.find((d) => d.id === n.id)!;
          return { ...orig, x: n.position.x, y: n.position.y };
        }),
      });
    },
    [document, onChange, rfNodes],
  );

  const onEdgesChange = useCallback(
    (changes: unknown) => {
      const next = applyEdgeChanges(changes, rfEdges as never) as typeof rfEdges;
      onChange({ ...document, edges: next.map((e) => ({ id: e.id, source: e.source, target: e.target })) });
    },
    [document, onChange, rfEdges],
  );

  const onConnect = useCallback(
    (conn: unknown) => {
      const next = addEdge(conn as never, rfEdges as never) as typeof rfEdges;
      onChange({ ...document, edges: next.map((e) => ({ id: e.id, source: e.source, target: e.target })) });
    },
    [document, onChange, rfEdges],
  );

  return (
    <ReactFlowProvider>
      <ReactFlow
        nodes={rfNodes}
        edges={rfEdges}
        nodeTypes={nodeTypes}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        fitView
      >
        <Background />
        <Controls />
      </ReactFlow>
    </ReactFlowProvider>
  );
}
```

> Note: the exact React Flow change-handler signatures vary by version. The test mocks the lib, so it only verifies the document->node mapping. When wiring against the real lib, follow `@xyflow/react`'s controlled-flow example; keep `onChange` emitting our `CanvasDocument` shape so autosave stays the single write path.

- [ ] **Step 4: Run to verify it passes**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/canvas-board`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/canvas-board.tsx apps/admin/src/features/canvas/canvas-board.test.tsx
git commit -m "feat(canvas): add xyflow canvas board wrapper"
```

---

## Task 18: History sidebar + chat panel (slash menu) + page composition

**Files:**
- Create: `apps/admin/src/features/canvas/history-sidebar.tsx`
- Create: `apps/admin/src/features/canvas/chat-panel.tsx`
- Create: `apps/admin/src/features/canvas/chat-panel.test.tsx`
- Modify: `apps/admin/src/features/canvas/page.tsx` (replace the Task 11 stub)
- Create: `apps/admin/src/features/canvas/page.test.tsx`

- [ ] **Step 1: Write the failing tests**

Slash menu lists the four v1 skills and dispatches; page renders three panes. Mock the board + queries.

`chat-panel.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ChatPanel } from "./chat-panel";

describe("ChatPanel slash menu", () => {
  it("lists the four v1 skills when input starts with /", () => {
    render(<ChatPanel canvasId="c1" selectedNodeIds={[]} onSkillNode={() => {}} />);
    const input = screen.getByPlaceholderText("/ 或提问");
    fireEvent.change(input, { target: { value: "/" } });
    expect(screen.getByText("/search")).toBeInTheDocument();
    expect(screen.getByText("/image")).toBeInTheDocument();
    expect(screen.getByText("/analyze")).toBeInTheDocument();
    expect(screen.getByText("/kb")).toBeInTheDocument();
  });
});
```

`page.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./canvas-board", () => ({ CanvasBoard: () => <div>board</div> }));
vi.mock("./history-sidebar", () => ({ HistorySidebar: () => <div>history</div> }));
vi.mock("./chat-panel", () => ({ ChatPanel: () => <div>chat</div> }));
vi.mock("./queries", () => ({
  useCanvasList: () => ({ data: [], isLoading: false }),
  useCanvas: () => ({ data: { id: "c1", title: "B", document: { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } } } }),
  useCreateCanvas: () => ({ mutate: vi.fn() }),
  useSaveCanvas: () => ({ mutateAsync: vi.fn() }),
  useDeleteCanvas: () => ({ mutate: vi.fn() }),
}));

import { CanvasPage } from "./page";

describe("CanvasPage", () => {
  it("renders the three panes", () => {
    render(<CanvasPage />);
    expect(screen.getByText("history")).toBeInTheDocument();
    expect(screen.getByText("board")).toBeInTheDocument();
    expect(screen.getByText("chat")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/chat-panel canvas/page`
Expected: FAIL (modules not found / page is still the stub).

- [ ] **Step 3: Implement the panes + page**

`history-sidebar.tsx`: lists `useCanvasList()` items as `NavLink`/buttons; a surfaced "新建画布" `Button` calling `useCreateCanvas().mutate()` then navigating to `/canvas/:id` (keeps the create flow reachable). Highlight the active canvas.

`chat-panel.tsx`: a scroll area of messages + an `Input` (placeholder `"/ 或提问"`). When the value starts with `/`, show a shadcn `DropdownMenu`/command list of the four skills (`/search`, `/image`, `/analyze`, `/kb`) with descriptions. On submit:
- plain text -> `streamCanvasChat(canvasId, text, selectedNodeIds, ...)`, append streamed answer to the thread.
- `/search`, `/image`, `/analyze` -> `streamCanvasChat`; on the `node` SSE payload call `onSkillNode(payload)` so the page adds the node + autosaves.
- `/kb` -> open a shadcn `Dialog` project picker (list projects the user can access) and add a `kb` node locally.

```tsx
export const CANVAS_SKILLS = [
  { name: "/search", description: "Web search -> result node" },
  { name: "/image", description: "Text to image -> image node" },
  { name: "/analyze", description: "Analyze selected/connected nodes" },
  { name: "/kb", description: "Add a knowledge-base node" },
] as const;
```

`page.tsx` (three-pane composition; owns the live document + autosave):

```tsx
import { useParams } from "react-router-dom";
import { useCallback, useEffect, useState } from "react";
import { CanvasBoard } from "./canvas-board";
import { HistorySidebar } from "./history-sidebar";
import { ChatPanel } from "./chat-panel";
import { useAutosave } from "./use-autosave";
import { useCanvas, useSaveCanvas } from "./queries";
import type { CanvasDocument } from "./types";

export function CanvasPage() {
  const { canvasId } = useParams();
  const canvas = useCanvas(canvasId);
  const save = useSaveCanvas(canvasId ?? "");
  const [doc, setDoc] = useState<CanvasDocument | null>(null);
  const [title, setTitle] = useState("");

  useEffect(() => {
    if (canvas.data) {
      setDoc(canvas.data.document);
      setTitle(canvas.data.title);
    }
  }, [canvas.data]);

  const onSave = useCallback(
    (value: CanvasDocument) => save.mutateAsync({ title, document: value }),
    [save, title],
  );
  const { status } = useAutosave({ value: doc ?? emptyDoc(), delayMs: 800, onSave });

  return (
    <div className="flex h-full">
      <HistorySidebar activeId={canvasId} />
      <div className="flex min-w-0 flex-1 flex-col">
        <CanvasHeader title={title} status={status} onRetry={() => doc && onSave(doc)} />
        {doc ? (
          <CanvasBoard
            document={doc}
            onChange={setDoc}
            onRunNode={(nodeId) => runNodeAndAppend(nodeId, doc, setDoc)}
          />
        ) : null}
      </div>
      <ChatPanel
        canvasId={canvasId ?? ""}
        selectedNodeIds={[]}
        onSkillNode={(payload) => addSkillNode(payload, setDoc)}
      />
    </div>
  );
}
```

> Note: `CanvasHeader` shows the title + the save-status indicator ("已保存" green / "保存失败 · 重试" red retry per the design mock) — implement inline or as a small local component using shadcn `Button`. `runNodeAndAppend` calls `runCanvasNode` and, on `done`, appends a new version + sets it active + lets autosave persist (server never writes). `addSkillNode` places the returned node at the suggested x/y and lets autosave persist. `emptyDoc()` returns `{ nodes: [], edges: [], viewport: { x:0,y:0,zoom:1 } }`. When `canvasId` is absent, show an empty-state prompting "新建画布". Keep everything composed from shadcn primitives.

- [ ] **Step 4: Run to verify they pass**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test -- canvas/chat-panel canvas/page`
Expected: PASS.

- [ ] **Step 5: Run the full frontend test + typecheck**

Run: `cd /e/Projects/Js/knowledge/apps/admin && npm test && npx tsc --noEmit`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/canvas/history-sidebar.tsx apps/admin/src/features/canvas/chat-panel.tsx apps/admin/src/features/canvas/chat-panel.test.tsx apps/admin/src/features/canvas/page.tsx apps/admin/src/features/canvas/page.test.tsx
git commit -m "feat(canvas): add history sidebar, chat panel with slash menu, and page"
```

---

## Final verification

- [ ] **Backend:** `cd /e/Projects/Js/knowledge && cargo test -p knowledge-server && cargo build -p knowledge-server` — all green.
- [ ] **Frontend:** `cd /e/Projects/Js/knowledge/apps/admin && npm test && npx tsc --noEmit` — all green.
- [ ] **Manual smoke (optional, needs DB + provider):** create a canvas, add a Note + URL + KB node, wire edges into an AI-Analyze node, run it, switch versions; run `/search`, `/image`; confirm autosave indicator transitions idle -> saving -> saved, and "保存失败" on a forced failure.
- [ ] After all tasks: use **superpowers:finishing-a-development-branch** to complete the work.

## Spec coverage map

- Top-level nav + per-user history: Tasks 11, 13, 18.
- Five node types: Task 16; node data shapes: Task 12.
- Directed reference edges + AI consumes incoming: Tasks 9, 10, 17.
- Canvas chat (plain + slash) /search //image //analyze //kb: Tasks 10, 14, 18.
- Text-to-image (net-new): Tasks 6, 7; asset store + GET: Task 5.
- URL fetch -> readability -> markdown (net-new): Task 8.
- Unlimited append-only version history + switcher: Tasks 12 (data), 16 (switcher), 18 (append on done).
- Debounced autosave + save status incl. failure: Task 15, 18.
- Persistence JSONB-as-TEXT, owner-scoped, reserved canvas_access: Tasks 1, 3, 4.
- KB via RAG with runtime permission check: Task 10.
- Server never mutates document (sole write = autosave PUT): Tasks 4, 10, 18.
- Deferred (not built): `/research`, sharing UI, real-time co-edit.
