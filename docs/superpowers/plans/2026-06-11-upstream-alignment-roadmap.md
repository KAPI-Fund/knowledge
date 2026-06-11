# Upstream Alignment Roadmap

> **For agentic workers:** This is a roadmap, not an executable plan. Each phase gets its own detailed implementation plan (written with superpowers:writing-plans) when it starts. Phase 3a's detailed plan already exists: `docs/superpowers/plans/2026-06-11-wiki-page-editing.md`.

**Goal:** Bring the server-based knowledge system to functional parity with the `upstream_llm_wiki/` reference app for the features that make sense in a multi-user server deployment.

**Ground rules (from AGENTS.md):**
- Business logic is ported from `upstream_llm_wiki/` — do not reinvent semantics the upstream already settled.
- `cargo clippy --workspace --all-targets -- -D warnings` must stay clean.
- Key-node tests only; no excessive test suites.
- No Chinese in code. Conventional Commits in imperative mood.

---

## Current State (verified 2026-06-11)

### Aligned and real (no work needed)

- Three-stage ingest pipeline (analysis → generation → review suggestions) with provider calls, page merge, wikilink enrichment, ingest cache (`crates/knowledge-server/src/tasks/executors.rs`, `crates/knowledge-core/src/ingest.rs`)
- Document parsing: PDF (pdfium), docx, pptx, xlsx/xls/ods, image extraction + vision captioning
- Hybrid retrieval: markdown chunking, Postgres-stored embeddings, cosine ranking, keyword+vector RRF merge (`crates/knowledge-server/src/retrieval/`)
- Wikilink graph build + neighbors (`crates/knowledge-core/src/graph.rs`)
- Provider-backed query with save-to-wiki and auto-enrich
- Structural + semantic lint, review sweep, durable lease-based task queue, source-watch scheduler
- Admin UI feature modules (projects, files, search, graph, query, lint, reviews, tasks, audit, settings), Docker stack, Playwright e2e

### Stub / dead code

- `crates/knowledge-worker` — `WorkerStub` only; all tasks run embedded in the server process
- `crates/knowledge-core/src/query.rs::answer_from_results` — fake answer formatting, not referenced by the server; delete when convenient
- `users/routes.rs` — list-only, no user CRUD

### Missing vs upstream

| Upstream feature | Upstream source | Status here |
|---|---|---|
| Wiki page edit/delete + reference cleanup | `src/lib/wiki-cleanup.ts`, `wiki-page-delete.ts` | Files API is read-only |
| Multi-turn chat with streaming | `src/lib/chat-*.ts`, chat stores | One-shot query tasks only, no SSE |
| Dedup (detection queue/runner/storage) | `src/lib/dedup-*.ts` | Absent |
| API tokens | `src/lib/api-token.ts` | Absent (session auth only) |
| MCP server (7 tools) | `mcp-server/src/index.ts` | Absent |
| Web search + deep research | `src/lib/web-search.ts`, `deep-research*.ts` | Absent |
| CLIP image vector search | `src/lib/clip-*.ts` | Absent |
| MinerU ingestion path | `src/lib/mineru*.ts` | Absent |
| Settings coverage (16 sections) | `src/components/settings/sections/` | Single provider settings form |
| pgvector | n/a (upstream uses LanceDB) | In-memory cosine over JSON columns |

---

## Phase 3a — Wiki Page Editing (next; detailed plan ready)

**Scope:** Server-side wiki page save (create/update) and batch cascade delete with full reference cleanup, exposed in the admin Files page.

**Ported logic:** `upstream_llm_wiki/src/lib/wiki-cleanup.ts` (normalized ref keys, index-listing cleanup, wikilink stripping — including upstream's Bug A/Bug B guards) and `wiki-page-delete.ts` (slug+title snapshot → file+embedding+media cascade → best-effort sweep of surviving pages including `related:` frontmatter).

**Deliverables:**
- `knowledge-core`: `project/wiki_cleanup.rs` (pure helpers + unit tests), `project/wiki_pages.rs` (save + cascade delete)
- `knowledge-server`: `PUT /api/projects/{id}/files/content`, `POST /api/projects/{id}/wiki-pages:delete`, embedding-chunk deletion, audit logs
- Admin: edit/save/delete UI on the Files page preview pane
- Integration test proving cascade semantics end-to-end

**Exit criteria:** A wiki page can be created, edited, and deleted from the admin UI; deleting a page removes its embeddings, media dir (source pages), index entries, body wikilinks, and `related:` entries — without touching superstring links (`[[OpenAI]]` survives deleting `ai`).

## Phase 3b — Chat Query (conversations + streaming)

**Scope:** Multi-turn conversations persisted in Postgres, SSE streaming of provider responses, retrieval-context assembly per turn (port upstream context-budget logic), conversation list/rename/delete in admin UI.

**Key upstream sources:** `src/lib/chat-*.ts`, `context-budget.ts`, query prompt templates.

**Major decisions to settle at planning time:**
- SSE vs WebSocket (recommend SSE via axum's `Sse` — matches one-way token stream)
- Conversation schema: `conversations` + `conversation_messages` tables, project-scoped
- Whether chat replaces or coexists with the current one-shot query task flow (recommend coexist; chat is interactive, query tasks remain for save-to-wiki workflows)

**Exit criteria:** User can hold a multi-turn conversation grounded in project retrieval, with streamed responses, from the admin UI.

## Phase 3c — Dedup

**Scope:** Duplicate-page detection (embedding similarity + LLM confirmation), dedup queue with review/merge/dismiss actions, merge execution that preserves references (reuses Phase 3a cleanup helpers for redirects).

**Key upstream sources:** `src/lib/dedup-queue.ts`, `dedup-runner.ts`, `dedup-storage.ts`.

**Depends on:** Phase 3a (reference rewrite helpers), existing embeddings infra.

**Exit criteria:** Dedup scan task produces candidate pairs; admin UI can review and merge; merged pages leave no dangling references.

## Phase 4 — External Access: API Tokens, MCP, Web Search, Deep Research

**Scope:**
1. API tokens (hashed storage, per-project scope, middleware auth path parallel to sessions) — port `src/lib/api-token.ts` semantics
2. MCP server exposing the upstream 7 tools (status/projects/files/read_file/reviews/search/graph) against the REST API, authenticated by API token
3. Web search provider integration + deep-research task type (multi-step search/read/synthesize loops as queue tasks)

**Depends on:** Phase 3b (deep research reuses streaming + context assembly).

**Exit criteria:** An external MCP client (e.g. Claude Code) can browse and search a project with a scoped token; deep-research tasks produce sourced reports.

## Phase 5 — Platform Hardening

**Scope (independent tracks, schedule by need):**
- **Worker split:** move task execution from the embedded server loop into `knowledge-worker` as a separate deployable; queue contract already lease-based so this is mostly process plumbing
- **pgvector:** replace JSON-column embeddings + in-memory cosine with pgvector indexes; migration rewrites `project_embedding_chunks`
- **CLIP image search** and **MinerU ingestion** (port upstream pipelines; both need provider/sidecar decisions)
- **Settings parity:** expand the settings UI toward upstream's 16 sections as the underlying features land
- **User management:** full user CRUD + roles (currently list-only)
- **i18n** for the admin UI
- Delete dead code: `knowledge-core/src/query.rs::answer_from_results`

**Exit criteria:** per-track; each lands behind its own detailed plan.

---

## Sequencing rationale

3a is first because it is small, unblocks dedup (3c), and closes the most user-visible gap (the wiki is currently immutable from the server UI). 3b is second because chat is the highest-value missing user feature and Phase 4's deep research builds on it. Phase 4 before Phase 5 because external access (MCP/tokens) multiplies the system's usefulness, while Phase 5 items are internal quality improvements that don't change the feature surface.
