# Knowledge Platform Design

## Summary

This document defines the first full-system design for a team-internal knowledge platform derived from the Rust core ideas in `nashsu/llm_wiki`, but restructured into a deployable backend service plus a JavaScript web admin console.

The target environment is a LAN or internal team deployment with:

- one organization
- multiple projects
- project-level access isolation
- file-first knowledge storage
- Rust-managed LLM orchestration
- built-in account system

The system should preserve the core strengths of `llm_wiki`:

- project directory as the source of truth
- wiki generation from raw documents
- incremental ingest
- source traceability
- hybrid retrieval using text, graph, and optional vectors

It should also fix the main architectural problem in the current reference implementation: Rust domain logic, local HTTP API, desktop commands, and runtime state are too tightly mixed. This design separates them into clear backend layers suitable for shared team use.

## Goals

- Recreate the Rust knowledge-processing core as a backend service instead of a desktop-only runtime.
- Provide a JavaScript web admin console for project, source, task, review, and system management.
- Add stable HTTP APIs for both the admin console and future integrations.
- Keep Markdown and project folders as the canonical knowledge representation.
- Support multiple projects with project-level membership and permissions.
- Make long-running ingest and query flows recoverable, observable, and auditable.

## Non-Goals

The first phase does not include:

- browser extension support
- MCP support
- deep research orchestration
- online rich Markdown editing
- multi-organization tenancy
- object storage or distributed deployment
- file-level ACLs
- advanced graph analytics such as community detection or insight ranking

## Architecture

The system will be implemented as a Rust monolith with clean internal layering, plus a separate JavaScript admin frontend.

### Runtime Shape

- one Rust backend process
- one JS admin application served separately or bundled through normal web deployment

The backend process exposes HTTP APIs, runs background jobs, calls LLM providers, manages project directories, and maintains derived indexes.

The frontend is an operational and collaboration console. It does not own knowledge construction logic.

### Repository Layout

```text
/
├─ apps/
│  └─ admin/
├─ crates/
│  ├─ knowledge-core/
│  ├─ knowledge-server/
│  └─ knowledge-worker/
├─ packages/
│  └─ api-client/
├─ tests/
│  ├─ rust-integration/
│  └─ web/
├─ docs/
│  └─ superpowers/
│     ├─ specs/
│     └─ plans/
├─ Cargo.toml
├─ package.json
└─ .gitignore
```

### Internal Layering

- `knowledge-core`
  Pure domain logic. No HTTP, no web UI assumptions.
- `knowledge-worker`
  Background task orchestration, persistence of task state, retry, cancellation, and staged execution.
- `knowledge-server`
  Authentication, authorization, REST API, session handling, validation, and admin-facing service boundaries.

Although `knowledge-worker` is a distinct crate, phase one still runs it in the same process as `knowledge-server`. This keeps deployment simple while preserving a future path to split worker execution if needed.

### Technology Choices

Phase one should use a deliberately small stack:

- Rust edition `2024`
- `axum` for HTTP APIs
- `tokio` for async runtime and background execution
- `serde` and `serde_json` for wire and checkpoint formats
- `sqlx` with SQLite for accounts, sessions, membership, task summaries, and audit data
- `tantivy` for embedded text indexing under each project
- `lancedb` for optional vector indexes under each project
- React with TypeScript and Vite for the admin console

SQLite is preferred in phase one because the system runs as a single backend process and keeps knowledge content on disk as files. This avoids adding external database operations burden for an internal team deployment. If a later phase requires stronger horizontal scaling, the persistence layer can be reworked independently of the knowledge storage model.

## Project Storage Model

Each knowledge project is a directory on disk and remains the canonical source of truth for knowledge content.

### Project Directory Layout

```text
project-root/
├─ purpose.md
├─ schema.md
├─ raw/
│  ├─ sources/
│  └─ assets/
├─ wiki/
│  ├─ index.md
│  ├─ log.md
│  ├─ overview.md
│  ├─ entities/
│  ├─ concepts/
│  ├─ sources/
│  ├─ queries/
│  ├─ synthesis/
│  └─ comparisons/
└─ .knowledge/
   ├─ project.json
   ├─ ingest/
   │  ├─ queue.json
   │  ├─ checkpoints/
   │  └─ cache/
   ├─ index/
   │  ├─ search.db
   │  └─ vectors/
   ├─ reviews/
   │  └─ items.json
   └─ locks/
```

### Storage Boundaries

- `raw/`
  Immutable source input layer. Files can be imported, deleted, or rescanned, but LLM pipelines must not rewrite their content.
- `wiki/`
  Generated knowledge layer and human-readable output. This is the main content exposed to users.
- `.knowledge/`
  Runtime and system-managed state. This is private to the backend and not intended for direct editing by normal users.

### Database Role

The database is not the primary store for knowledge content. It only supports collaboration and system operation.

The database stores:

- users
- password hashes
- sessions
- project registry entries
- project membership and roles
- system configuration references
- task summaries
- audit logs
- optional read-model caches for fast UI access

The database does not store canonical knowledge content:

- raw source file bodies
- wiki page bodies
- `index.md`
- `log.md`
- `overview.md`

The system must remain recoverable from project directories if the database is lost. The inverse is not required.

### Task State Source of Truth

Task execution state is split into two layers on purpose:

- project-local checkpoint and recovery state under `.knowledge/ingest/`
- database-backed task summary state for cross-project listing, filtering, permissions, and dashboard views

The project-local state is canonical for task recovery inside one project. The database view is a mirrored operational index for the admin console and system-level reporting.

## Backend Domain Modules

The domain core should be split into narrow units with explicit responsibilities.

### `knowledge-core` Modules

- `project`
  Project creation, directory validation, path normalization, and project metadata.
- `source`
  Source import, deletion, rescan, hashing, and change detection.
- `extract`
  Document preprocessing for PDF, Office formats, Markdown, and plain text into a normalized intermediate representation.
- `wiki`
  Wiki page model, frontmatter rules, and maintenance for `index.md`, `log.md`, and `overview.md`.
- `ingest`
  Two-stage ingest domain workflow, merge planning, and traceability rules.
- `search`
  Text retrieval, title matching, snippet creation, and ranking helpers.
- `graph`
  Node and edge extraction from wiki content and source relationships.
- `vector`
  Chunk mapping and vector index abstractions.
- `review`
  Human review item model and state transitions.
- `query`
  Context construction for question answering.
- `lint`
  Integrity checks such as dead links, orphan pages, missing frontmatter, and broken source references.
- `llm`
  Provider abstraction for LLM and embedding calls, without business orchestration.
- `audit`
  Structured events and traceable operation records.

### `knowledge-worker` Task Types

- `import_sources`
- `ingest_source`
- `reingest_source`
- `rescan_sources`
- `rebuild_index`
- `rebuild_graph`
- `rebuild_vectors`
- `run_lint`
- `materialize_review_items`
- `answer_query`

Each task type must support:

- persistence
- retry
- cancellation
- stage-level progress
- clear failure reasons
- safe restart without corrupting project state

### `knowledge-server` Service Scope

The server should expose stable resource-oriented APIs rather than file-system-shaped APIs. The main resource families are:

- auth
- users
- projects
- project members
- sources
- tasks
- search
- graph
- reviews
- queries
- settings
- audit logs

## Ingest Design

The ingest pipeline preserves the two-stage generation model from `llm_wiki`, but moves it into a stricter backend-controlled workflow.

### Single-Source Ingest Pipeline

```text
discover -> hash -> extract -> analyze -> plan outputs -> generate wiki artifacts -> merge -> rebuild derived indexes -> finalize
```

### Stage Semantics

- `discover`
  Detect new, modified, or deleted files under `raw/sources/` and create tasks.
- `hash`
  Compute content hash and decide whether reprocessing is necessary.
- `extract`
  Convert source files to normalized text and metadata.
- `analyze`
  First LLM call. Produce structured analysis only.
- `plan outputs`
  Decide which wiki pages, review items, and derived files are intended to change.
- `generate wiki artifacts`
  Second LLM call. Produce candidate Markdown outputs and metadata patches.
- `merge`
  Safely merge generated outputs with the current project state.
- `rebuild derived indexes`
  Incrementally update text indexes, graph data, and vectors as needed.
- `finalize`
  Persist task results, audit data, cache state, and cleanup markers.

### Analysis Output Contract

The first LLM stage must output structured intermediate results, not free-form wiki pages. It should include:

- source summary
- candidate entities
- candidate concepts
- candidate links
- contradictions or tensions
- suggested review items
- suggested research topics
- intended touched pages

This makes generation reproducible and lets the system regenerate final Markdown from analysis without rerunning extraction.

### Incremental Reprocessing Rules

Reprocessing is driven by a cache key made from:

- source content hash
- relevant prompt version
- `schema.md`
- `purpose.md`
- configured model identity
- language choice
- chunking policy

Expected behavior:

- unchanged source plus unchanged config can be skipped
- unchanged source plus changed schema, purpose, prompt, or model can trigger `reingest_source`
- vector-only config changes should rebuild vectors, not re-run content generation

### Traceability Contract

Every wiki page must carry minimum frontmatter fields:

- `id`
- `type`
- `title`
- `sources`
- `updated_at`
- `generated_by`
- `revision`

`sources` must point to stable source identifiers or canonical relative source paths.

This traceability is required for:

- safe source deletion
- shared-page source detachment
- index cleanup
- graph cleanup
- vector cleanup

### Failure Recovery

The system must persist ingest checkpoints, at minimum:

- `extracted.json`
- `analysis.json`
- `generation.json`
- `merge-plan.json`
- `task-state.json`

This allows restart after a crash without restarting the entire pipeline from the beginning.

### Safe Write Rules

- page writes use temp files followed by atomic rename
- multi-file updates are driven by a recorded merge plan
- `index.md`, `overview.md`, and `log.md` are backend-maintained only
- only one write-bearing task per project may enter merge at a time
- different projects may process in parallel

## Retrieval and Query Design

Search and LLM question answering share retrieval primitives but remain distinct flows.

### Search Flow

```text
token recall -> metadata boost -> graph expansion -> vector merge -> rerank -> snippet assembly
```

Signal sources for the first implementation:

- keyword score
- title and type boost
- graph proximity
- vector similarity

This keeps the scoring model interpretable and close in spirit to `llm_wiki` without introducing advanced graph analytics too early.

### Query Flow

```text
intent parse -> seed retrieval -> graph expansion -> context budgeting -> citation pack assembly -> llm answer
```

The purpose is not to maximize recall, but to assemble the best bounded context for answer generation.

Rules:

- use token and optional vector retrieval to get seed materials
- use graph expansion to add only necessary related pages
- enforce a strict token budget
- package materials with numbered citations and source paths
- let the backend own the LLM answer generation step

### Graph Model

The first graph model is intentionally conservative.

Nodes come from:

- wiki pages
- page types
- source references

Edges come from:

- `[[wikilink]]`
- shared source membership
- optional semantic relatedness from vector or derived signals

The initial feature set supports:

- graph snapshot
- local neighborhood exploration
- orphan-page detection
- bridge-page detection

Advanced clustering or surprise insights are out of scope for phase one.

### Vector Model

- vectors are derived indexes, not canonical content
- page and chunk representations are separate
- retrieval should prefer chunks and then map results back to pages
- vector support is optional and must degrade cleanly to text plus graph retrieval

If vectors are unavailable or damaged, search and query must still function.

## API Design

The HTTP API should be resource-oriented and safe for a multi-user admin console.

### Auth

- `POST /api/auth/login`
- `POST /api/auth/logout`
- `GET /api/auth/me`

### Users

- `GET /api/users`
- `POST /api/users`
- `PATCH /api/users/{userId}`
- `POST /api/users/{userId}/password`

### Projects

- `GET /api/projects`
- `POST /api/projects`
- `GET /api/projects/{projectId}`
- `PATCH /api/projects/{projectId}`

### Project Members

- `GET /api/projects/{projectId}/members`
- `POST /api/projects/{projectId}/members`
- `PATCH /api/projects/{projectId}/members/{memberId}`
- `DELETE /api/projects/{projectId}/members/{memberId}`

### Sources

- `GET /api/projects/{projectId}/sources`
- `POST /api/projects/{projectId}/sources:import`
- `POST /api/projects/{projectId}/sources:rescan`
- `DELETE /api/projects/{projectId}/sources/{sourceId}`

### Tasks

- `POST /api/projects/{projectId}/tasks`
- `GET /api/projects/{projectId}/tasks`
- `GET /api/projects/{projectId}/tasks/{taskId}`
- `POST /api/projects/{projectId}/tasks/{taskId}:cancel`
- `POST /api/projects/{projectId}/tasks/{taskId}:retry`

### Retrieval

- `POST /api/projects/{projectId}/search`
- `POST /api/projects/{projectId}/query`
- `GET /api/projects/{projectId}/graph`
- `GET /api/projects/{projectId}/graph/{nodeId}/neighbors`

### Reviews and Audit

- `GET /api/projects/{projectId}/reviews`
- `PATCH /api/projects/{projectId}/reviews/{reviewId}`
- `GET /api/projects/{projectId}/audit-logs`

### System Settings

- `GET /api/system/settings`
- `PATCH /api/system/settings`

## Authentication and Authorization

The first version uses a built-in account system.

### Authentication

- username and password login
- passwords stored with `argon2id`
- backend-managed session or signed cookie
- CSRF protection for all write operations
- health endpoints may remain unauthenticated
- all normal APIs require login

### Authorization

The first permission model uses three role levels:

- `admin`
  Full system administration across users, settings, and projects.
- `project_owner`
  Full control of one project, including membership, import actions, settings, and reviews.
- `project_member`
  Read project content, issue queries, and inspect task outcomes. Import permission may optionally be a separate boolean capability.

File-level ACLs are intentionally out of scope.

## Task and Audit Model

All long-running operations should be task-based rather than synchronous request-based.

This includes:

- source import
- source rescan
- full reingest
- vector rebuild
- lint runs
- batch review handling
- LLM-backed query answering

The `query` API may look interactive to the frontend, but it should still run through a task model for consistency, cancelability, and observability.

Phase one task progress delivery should use polling rather than WebSocket or SSE streaming. This keeps the server model simpler and is sufficient for admin-style operational flows. The frontend can poll task detail endpoints for stage, percent, result summary, and failure information.

### Audit Requirements

Audit records should exist for:

- login and logout
- user management changes
- project creation and project updates
- membership changes
- source import, delete, and rescan
- ingest start, success, failure, and cancellation
- review decisions
- system configuration changes
- index rebuilds and other high-risk operations

Each audit record should include:

- `actor`
- `project_id`
- `action`
- `target_type`
- `target_id`
- `task_id`
- `timestamp`
- `summary`
- `metadata`

## Admin Console Design

The JavaScript admin console should be designed for operations and collaboration, not as a browser clone of the desktop app.

### Information Architecture

System-level navigation:

- Dashboard
- Projects
- Users
- Settings

Project-level navigation:

- Overview
- Sources
- Tasks
- Search
- Graph
- Reviews
- Audit
- Project Settings

### Main Screens

- `Dashboard`
  System health, queue backlog, failed task counts, and recently active projects.
- `Projects`
  Create projects, browse projects, inspect membership, and check index health.
- `Overview`
  Project metrics such as source count, wiki page count, recent ingest activity, pending reviews, and vector status.
- `Sources`
  Import, rescan, inspect source hash and status, and view linked outputs.
- `Tasks`
  Inspect progress, stage, failures, cancel, and retry.
- `Search`
  Human-facing retrieval view with summaries and related content.
- `Graph`
  Local graph exploration and filtering.
- `Reviews`
  Human review workflow for flagged items.
- `Audit`
  Operational traceability and failure analysis.
- `Project Settings`
  Project root registration, provider references, ingest policy, vector settings, and membership management.

### UI Interaction Rules

- runtime data under `.knowledge/` is never directly edited from the UI
- source import, delete, and rescan are always task-based
- phase one wiki page editing is read-only plus file-path visibility
- search, graph, and review should reuse a shared detail panel where practical
- overview pages should link directly into failing tasks and pending reviews

## Error Handling

Errors should be categorized rather than flattened into generic internal failures.

### Error Classes

- `user error`
  Unauthenticated access, invalid request data, unsupported file type, missing project path.
- `domain error`
  Duplicate source, invalid frontmatter, broken traceability, merge conflict.
- `infra error`
  Database outage, vector index failure, disk write error, file lock contention.
- `provider error`
  LLM timeout, malformed provider response, embedding failure.

The API must map these classes to actionable responses so the frontend can distinguish:

- immediately fixable input problems
- retryable problems
- admin-only issues
- cases that require manual review

## Concurrency Model

The concurrency model should stay conservative in phase one.

Rules:

- reads may run concurrently within a project
- only one write-bearing knowledge task per project may enter merge
- different projects may ingest, query, and rebuild in parallel
- vector rebuilds and ingest writes must not compete for the same project write lock
- source rescan detects changes and creates tasks, but does not mutate wiki content directly
- read-only retrieval may run concurrently
- LLM-backed query tasks should still respect project-level execution limits

The first lock model only needs:

- project read/write lock
- task lease
- atomic file write discipline

No distributed lock design is needed in phase one.

## Testing Strategy

The project should follow a narrow critical-path testing strategy rather than broad ceremonial coverage.

### Required Test Categories

- `core unit tests`
  Path safety, frontmatter parsing, wikilink extraction, merge decision logic, and query budget trimming.
- `core integration tests`
  End-to-end single-source ingest, source deletion cleanup, and recovery after task interruption.
- `server API tests`
  Login, permission isolation, membership access boundaries, and task lifecycle APIs.
- `admin smoke tests`
  Login, project entry, source import flow, task list rendering, and search result display.

### Explicit Test Constraints

- no excessive UI snapshot coverage
- no ceremonial tests for trivial helpers
- focus on knowledge safety, permission boundaries, and recovery behavior

## Phase One Delivery Scope

Phase one must deliver:

- built-in login and project-level permissions
- project creation and directory registration
- source import, rescan, and deletion
- two-stage ingest
- incremental reprocessing and checkpoint recovery
- wiki generation with source traceability
- text search with optional vector support
- basic graph query support
- review listing and review action handling
- task center
- audit logs
- foundational admin console pages

This scope is intentionally large enough to be useful for an internal team, but still bounded enough to avoid collapsing into a full clone of every advanced `llm_wiki` feature.

## Delivery Decomposition

Even though this document describes the full phase-one architecture, implementation should still proceed in bounded slices:

1. platform foundation
   Workspace layout, database bootstrap, auth, users, projects, and membership.
2. project file core
   Project directory model, source import, rescan, deletion, path safety, and task persistence.
3. ingest engine
   Extraction, two-stage analysis and generation, merge planning, traceability, and safe writes.
4. retrieval layer
   Text search, graph extraction, optional vectors, and task-based query answering.
5. admin operations UI
   Dashboard, projects, sources, tasks, reviews, audit, and project settings.

The first implementation plan should target these slices in order rather than treating the entire system as one flat feature.

## Implementation Guidance

When implementation starts, the initial delivery order should prioritize foundational backend correctness over UI breadth:

1. workspace scaffolding and crate boundaries
2. auth, users, and projects
3. project directory and source handling
4. ingest pipeline and safe writes
5. task persistence and recovery
6. search, graph, and vector integration
7. review and audit endpoints
8. admin console screens for the critical operational flows

This ordering preserves momentum while keeping the highest-risk system behavior under test first.
