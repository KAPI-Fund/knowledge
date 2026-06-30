# Knowledge Canvas (知识工作台) - Design

**Date:** 2026-06-30
**Status:** Approved for planning

## Goal

A top-level "infinite canvas" feature for deep research. Users link knowledge
nodes (web pages, wiki knowledge bases, notes, AI analyses, generated images)
with directed "reference" edges, and an AI reasons over the board. It is a
sibling of Dashboard / Projects / Graph in the global nav, not scoped to a
single project.

## Scope (v1)

In scope:
- Top-level Canvas destination + per-user canvas history.
- Five node types: Knowledge Base, URL/Web, AI-Analyze, Note, AI-Image.
- Directed "reference" edges; AI nodes consume their incoming nodes as context.
- Right-side canvas chat: plain questions (AI over the whole board or selected
  nodes) and slash skills.
- Slash skills: `/search`, `/image`, `/analyze`, `/kb`.
- Text-to-image generation (net-new server capability).
- Server-side URL fetch + readability extraction to markdown (net-new).
- Unlimited append-only version history on AI nodes.
- Debounced autosave with a save status indicator (including "save failed").

Deferred (explicitly out of v1):
- `/research` deep-research skill (add once the canvas works).
- Canvas sharing/collaboration UI (schema is reserved for it; not enabled).
- Real-time multi-user co-editing.

## Key Decisions

1. **Placement:** top-level nav item, personal (per-user). Not project-scoped.
2. **Persistence:** JSONB document model. One row per canvas holds the whole
   board. Chosen for fewest migrations, heterogeneous node shapes, and a v1
   that is private + single-user. (Alternatives weighed: normalized tables,
   hybrid; rejected for v1 as premature.)
3. **Ownership:** private now; reserve a `canvas_access` table so sharing can
   be switched on later without a migration rewrite. (Decision "C".)
4. **KB nodes feed the AI via retrieval (RAG)**, reusing the existing
   project-chat retrieval path - not whole-wiki context dumps. (Decision "A".)
5. **Text-to-image is in v1** via a new provider capability. (Decision "A".)
6. **Generated images go into a unified asset store** (not canvas-scoped) so it
   is reusable elsewhere; nodes store an asset URL, never base64 in the
   document.
7. **AI nodes keep unlimited version history** (append-only); the node UI has a
   version switcher.
8. **Canvas library:** `@xyflow/react` (React Flow). The existing
   sigma/graphology graph view is read-only and is not reused for editing.
9. **The server never mutates the document during a run.** The sole write path
   is the frontend autosave PUT. On SSE `done`, the client writes node content
   and triggers autosave. This keeps a single source of truth and avoids
   run/save races.

## Architecture

```
Frontend (apps/admin, React 19 + @xyflow/react)
  [history sidebar] [infinite canvas: nodes/edges/zoom/autosave] [canvas chat]
        |  REST (CRUD + autosave PUT)   |  SSE (node run / chat stream)
        v                               v
Backend (crates/knowledge-server) - new `canvas` module, merged into build_router
  canvas::routes -> canvas CRUD, autosave, node run (SSE), chat (SSE), skills
        |
        +-- reuse: provider bridge (stream_chat / complete)
        +-- reuse: project retrieval (RAG) for KB nodes
        +-- reuse: web_search (/search)
        +-- NEW:   image generation (/image)
        +-- NEW:   URL fetch -> readability -> markdown (URL node)
        |
Storage (Postgres, migration 0014)
  canvases(id, owner_id, title, document JSONB, created_at, updated_at)
  canvas_access(canvas_id, principal_id, role)   -- reserved, v1 writes owner only
  assets(id, owner_id, mime, bytes, created_at)   -- unified asset store
```

### Backend module layout

- `crates/knowledge-server/src/canvas/mod.rs`
- `crates/knowledge-server/src/canvas/routes.rs` - axum router + handlers
- `crates/knowledge-server/src/canvas/store.rs` - sqlx CRUD over `canvases`
- `crates/knowledge-server/src/canvas/service.rs` - run orchestration: parse
  document, gather referenced node content, RAG for KB, assemble prompt,
  call provider, stream
- `crates/knowledge-server/src/http/router.rs` - add
  `.merge(canvas::routes::router())`
- Provider layer (`providers/openai_compatible.rs`, `providers/types.rs`) -
  add `generate_image` (OpenAI-compatible images endpoint, reusing the
  configured provider `base_url` / key from provider settings).
- New asset store module/handlers (or fold into canvas module) backing
  `assets` + `GET /api/assets/:id`.

### Data model

`canvases`:
- `id` (uuid, pk)
- `owner_id` (fk -> users/principal)
- `title` (text)
- `document` (jsonb)
- `created_at`, `updated_at` (timestamptz)

`document` shape:
```json
{
  "nodes": [
    { "id": "...", "type": "note|url|kb|ai_analyze|ai_image",
      "x": 0, "y": 0, "w": 280, "h": 160, "data": { } }
  ],
  "edges": [ { "id": "...", "source": "nodeId", "target": "nodeId" } ],
  "viewport": { "x": 0, "y": 0, "zoom": 1 }
}
```

Per-type `data`:
- `note`: `{ "markdown": "..." }`
- `url`: `{ "url": "...", "title": "...", "markdown": "...", "status": "ok|error", "error": null }`
- `kb`: `{ "projectId": "...", "projectName": "..." }`
- `ai_analyze`: `{ "prompt": "...", "versions": [ { "id": "...", "content": "...", "createdAt": "..." } ], "activeVersionId": "...", "status": "idle|running|error", "error": null }`
- `ai_image`: `{ "prompt": "...", "versions": [ { "id": "...", "assetId": "...", "assetUrl": "...", "createdAt": "..." } ], "activeVersionId": "...", "status": "idle|running|error", "error": null }`

`canvas_access` (reserved): `(canvas_id, principal_id, role)` using existing
`AccessRole` (Viewer/Editor/Owner). v1 reads only `owner_id = current user`.

`assets`: `(id, owner_id, mime, bytes, created_at)`; served by
`GET /api/assets/:id`.

### API surface

All behind existing auth: `authorized_principal` + `validate_csrf`.

- `GET    /api/canvases` - list current user's canvases (id, title, updated_at)
- `POST   /api/canvases` - create (optional title) -> returns canvas
- `GET    /api/canvases/:id` - read full canvas (incl. document)
- `PUT    /api/canvases/:id` - autosave: replace title + document
- `DELETE /api/canvases/:id` - delete
- `POST   /api/canvases/:id/nodes/:nodeId/run` - SSE; run an AI node
- `POST   /api/canvases/:id/chat` - SSE; canvas chat (plain ask or slash skill)
- `GET    /api/assets/:id` - serve a stored asset (image bytes)

SSE protocol reuses the existing chat events: `delta` (token text), `done`
(final payload), `error` (message). For skill/chat runs that create a node,
the `done` payload carries the new node (type + data + suggested x/y) so the
client can place it and autosave.

### Data flow

1. **Edit:** user drags/edits nodes -> debounced `PUT /api/canvases/:id` with
   the full document.
2. **Run AI-Analyze node:** `POST .../nodes/:nodeId/run` -> server reads the
   document, finds incoming edges to the node, collects each referenced node's
   content (Note -> markdown text, URL -> extracted markdown, KB -> RAG
   retrieval keyed on the node's prompt) -> assembles a prompt -> streams via
   `provider.stream_chat`. Client renders tokens live; on `done` it appends a
   new version, sets it active, and autosaves.
3. **Canvas chat / skills:** `POST .../chat` -> SSE.
   - Plain question: AI answers over the whole board (or the user's selected
     nodes); the answer stays in the chat thread.
   - `/search <q>`: web_search -> `done` returns a result node.
   - `/image <prompt>`: generate_image -> store asset -> `done` returns an image
     node referencing the asset URL.
   - `/analyze`: analyze selected/connected nodes -> `done` returns an analyze
     node (or runs an existing selected node).
   - `/kb`: opens a project picker (client-side) listing projects the user can
     access; selecting one adds a KB node.
4. **URL node:** on URL entry the client calls the server fetch/extract path;
   the returned markdown + title is written into the node and autosaved.

## Error handling

- **URL fetch fails:** node enters `error` status, shows the message + a retry
  action.
- **AI / image run error (SSE `error`):** node flagged `error`; the previous
  version is preserved (history is append-only, nothing is lost).
- **Autosave fails:** header shows "保存失败" / save-failed; retry with backoff;
  the local document is kept so no edits are lost.
- **KB permission revoked:** the run-time access check on the referenced
  `projectId` fails; the KB node is flagged "no access" and excluded from
  context.
- **Provider not configured:** friendly message directing the user to settings.

## Testing

Backend:
- `canvas::store` CRUD (sqlx) round-trips document JSONB.
- Route auth/csrf rejection + happy paths.
- `GET /api/assets/:id` serves stored bytes with correct mime.
- URL fetch -> markdown extraction (deterministic fixture HTML).
- Image generation with a mocked provider (lock the request values, not just
  field names, per existing web-search/settings test style).
- Node-run SSE with a mocked provider (delta/done/error).
- KB permission check (referenced project the user cannot access is excluded /
  flagged).

Frontend (vitest + Testing Library):
- Canvas page renders the three panes.
- Autosave debounce fires a single PUT after rapid edits.
- Version switcher navigates AI node versions.
- Slash menu lists the four v1 skills and dispatches the right request.
- Optimistic node creation appears immediately, reconciles on `done`.
- SSE handlers mocked (reuse the chat `stream` test approach).

### Frontend files

- `apps/admin/src/features/canvas/page.tsx`
- `apps/admin/src/features/canvas/canvas-board.tsx` (@xyflow/react wrapper)
- `apps/admin/src/features/canvas/node-types/*` (one per node type)
- `apps/admin/src/features/canvas/chat-panel.tsx`
- `apps/admin/src/features/canvas/history-sidebar.tsx`
- `apps/admin/src/features/canvas/queries.ts`
- `apps/admin/src/features/canvas/stream.ts`
- Route + nav registration: `apps/admin/src/app/router.tsx`,
  `apps/admin/src/lib/route-meta.ts`

UI composes shadcn primitives only (Button, Tooltip, Dialog for the project
picker, Input/Textarea, dropdown for the slash menu). Node markdown and AI
results render through the existing `components/shared/markdown-message.tsx`.

## Upstream / existing-code provenance

Port key logic from `upstream_llm_wiki/`; cite paths in the plan:
- URL -> markdown: `upstream_llm_wiki/extension/Readability.js`,
  `upstream_llm_wiki/extension/Turndown.js`.
- AI streaming + slash skills: `upstream_llm_wiki/src/components/chat/chat-panel.tsx`,
  `upstream_llm_wiki/src/lib/llm-client.ts` (streamChat),
  `upstream_llm_wiki/src/lib/llm-providers.ts` (ChatMessage/ContentBlock/buildBody).
- Web search: `upstream_llm_wiki/src/lib/web-search.ts` (Tavily/SerpAPI/SearXNG/
  Ollama/AnyTXT); server already exposes `POST /api/web-search`
  (`crates/knowledge-server/src/web_search/`).
- Text-to-image: no upstream equivalent (upstream only has vision captioning,
  `src/lib/vision-caption.ts` + `src-tauri/src/commands/extract_images.rs`).
  Build `/image` net-new on the existing provider pattern.
- Deep research (deferred): `upstream_llm_wiki/src/lib/deep-research.ts`,
  `upstream_llm_wiki/src/components/layout/research-panel.tsx`.

Existing repo integration points:
- Streaming chat: `crates/knowledge-server/src/chat/routes.rs`
  (`send_message_handler`, `Sse` pattern), `chat/store.rs`.
- Provider bridge: `crates/knowledge-server/src/providers/openai_compatible.rs`,
  `providers/types.rs` (add `generate_image`).
- Router merge: `crates/knowledge-server/src/http/router.rs` (`build_router`).
- Auth: `authorized_principal`, `authorized_principal_with_role(AccessRole)`,
  `validate_csrf`; `AccessRole` Viewer/Editor/Owner.
- Frontend SSE + query conventions: `apps/admin/src/features/chat/stream.ts`,
  `features/chat/queries.ts`; CSRF token in localStorage key
  `knowledge.csrfToken` (`features/auth/csrf.ts`).
