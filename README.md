# Knowledge

Self-hosted knowledge workspace for collecting source material, building a
searchable Markdown knowledge base, and using provider-backed AI workflows to
explore, maintain, and extend it.

[English](#english) | [简体中文](#简体中文)

## English

> Knowledge is currently an early-stage project. Interfaces and configuration
> details may change between releases.

### What it provides

- Project-scoped workspaces with Markdown wiki pages and source files.
- Text, PDF, and common Office source formats are parsed into searchable
  content; extracted source media can be retained in the project workspace.
- Source import, background ingestion, incremental rescans, and source-watch
  automation.
- Hybrid keyword, vector, and graph retrieval for search and chat.
- Provider-backed chat, deep research, multimodal extraction, and image
  generation through OpenAI-compatible endpoints.
- Knowledge graph exploration, backlink-aware context, linting, review queues,
  and duplicate detection/merge workflows.
- Multi-user projects with organizations, teams, project roles, API tokens,
  audit logs, and per-project access controls.
- A React/Vite administration UI and a Rust/Axum HTTP API.
- MCP access through the native `/api/mcp` endpoint and an optional standalone
  Node.js MCP server for Claude Code, Codex CLI, Cursor, and other clients.

### Product overview

Knowledge is designed for teams that need an evidence-backed knowledge base,
not only a chat window. It keeps imported evidence, derived wiki pages, and
operational metadata separate so that AI output can be traced, reviewed, and
updated without losing the original source material.

#### Who it is for

| Scenario | How Knowledge helps |
| --- | --- |
| Research and due diligence | Ingest papers, documents, and web material, then keep findings linked to their sources. |
| Engineering and project documentation | Watch a local folder, maintain Markdown pages, and navigate dependencies through the graph. |
| Knowledge operations | Route lint findings and uncertain facts to Review, merge duplicates, and inspect task/audit history. |
| Agent-enabled workflows | Give MCP-compatible agents scoped search, read, chat, and rescan capabilities without sharing a database credential. |

#### Core concepts

| Concept | Product meaning |
| --- | --- |
| Account | A user identity authenticated by a session cookie or an API token. |
| Space | A personal, organization, or team namespace that owns projects. |
| Project | An isolated knowledge base with its own files, members, tasks, and retrieval context. |
| Source | Imported or watched material stored under `raw/sources/`; it is the evidence layer for ingestion. |
| Wiki | Human-readable Markdown under `wiki/`, generated and edited as the maintained knowledge layer. |
| Task | A persisted asynchronous operation such as ingest, rescan, lint, deduplication, or research. |
| Canvas | A per-user JSON document containing notes, URLs, knowledge-base references, search results, and AI nodes. |
| Provider connection | The active OpenAI-compatible chat/model endpoint used by server-side AI workflows. |

#### Typical knowledge workflow

1. Create an account and a project in a personal, organization, or team space.
2. Import source files or configure Source Watch for a local directory.
3. Run ingestion. The server parses the source, calls the configured provider,
   writes traceable wiki pages, and records review items where needed.
4. Use Search, Chat, Graph, or Deep Research to explore the project. Chat can
   cite retrieved wiki pages and expand context through graph relationships.
5. Run Lint, Reviews, and Dedup to repair broken links, resolve knowledge gaps,
   and merge duplicate pages.
6. Grant project access to collaborators or connect an agent through MCP/API
   tokens.

#### Product boundaries

- Project files are the durable content boundary. PostgreSQL stores metadata,
  permissions, settings, task state, conversations, embedding chunks, and
  generated assets; it is not a replacement for the project file tree.
- AI features are provider-dependent. Importing and browsing files can work
  without an LLM, while generation, chat, embeddings, and deep research require
  the relevant provider capability to be configured.
- A Canvas is currently private to its owner. The schema reserves an access
  table for future sharing, but collaborative canvas editing is not enabled.
- The embedded worker is intentionally part of the backend process. Splitting
  it into a separate service is a future deployment option, not a requirement
  of the current stack.

### Architecture

| Component | Location | Responsibility |
| --- | --- | --- |
| `knowledge-server` | `crates/knowledge-server` | Rust API, authentication, task scheduling, providers, MCP, and project operations |
| `knowledge-core` | `crates/knowledge-core` | File handling, ingestion, parsing, retrieval primitives, graph, lint, review, and deduplication logic |
| Admin UI | `apps/admin` | React/Vite workbench for projects, settings, search, chat, graph, and operations |
| API client | `packages/api-client` | Shared TypeScript API schemas and HTTP helpers |
| Standalone MCP server | `packages/mcp-server` | stdio MCP bridge backed by a per-user Knowledge API token |
| Skill runner | `services/skill-runner` | Optional CubeSandbox sidecar for agentic skill rendering, such as PPT generation |
| PostgreSQL | Docker service | Persistent application metadata, settings, users, tasks, and conversations |
| Redis | Docker service | Cache and task/runtime coordination |

### Technical architecture

```mermaid
flowchart LR
  UI[Admin UI<br/>React + Vite] -->|REST / SSE| API[knowledge-server<br/>Rust + Axum]
  MCP[MCP clients] -->|Bearer token| API
  API --> AUTH[Auth + tenancy + CSRF]
  AUTH --> DOMAIN[Project / chat / canvas services]
  DOMAIN --> CORE[knowledge-core<br/>parsing + retrieval + graph]
  DOMAIN --> DB[(PostgreSQL)]
  DOMAIN --> REDIS[(Redis)]
  DOMAIN --> LLM[OpenAI-compatible providers]
  CORE --> FS[(Project file roots)]
  DOMAIN --> TASKS[Embedded scheduler + workers]
  TASKS -. optional .-> RUNNER[skill-runner<br/>Linux + KVM + CubeSandbox]
```

#### Runtime responsibilities

- **HTTP boundary:** `http::router` exposes health, REST resources, SSE
  streams, and the Streamable HTTP MCP endpoint. JSON schemas are validated at
  the API boundary before service calls.
- **Identity and authorization:** `auth` resolves a session or Bearer token
  into a principal. Session mutations require the CSRF token; API tokens can
  be unscoped or restricted to one project. `tenancy::access` resolves the
  effective `Owner`, `Editor`, or `Viewer` role from personal/org/team space
  ownership and project grants.
- **Domain services:** `projects`, `chat`, `agent`, `canvas`, `deep_research`,
  `retrieval`, `providers`, `web_search`, `web_fetch`, and `mcp` implement
  product operations while keeping transport concerns in route modules.
- **Knowledge core:** `knowledge-core` contains filesystem-safe project
  primitives plus parsing, ingestion helpers, search, graph, review, lint,
  deduplication, and wiki-maintenance logic. It does not depend on the HTTP UI.
- **Persistence:** SQL migrations run at startup. PostgreSQL stores accounts,
  tenancy, settings, task summaries, conversations, embedding chunks, canvas
  JSON, assets, and audit logs. Project content remains under the configured
  `KNOWLEDGE_PROJECT_ROOT`.
- **Runtime coordination:** Redis provides cache access and task wake-up
  signaling. The scheduler polls for queued work, claims tasks with a lease,
  and recovers expired work after a restart.

#### Main data flows

**Ingestion and maintenance**

```text
source import / Source Watch
        -> raw/sources/
        -> queued project task
        -> text or multimodal extraction
        -> provider analysis and wiki generation
        -> wiki/ + review items + graph/index refresh
        -> task result and audit log
```

Source Watch compares file hashes and sizes against a project-local snapshot.
Only changed files are copied or removed, and ingest/delete work is queued so
the request path stays short. PDF and Office files use the parser layer in
`knowledge-core`; PDF image extraction uses PDFium.

**Search and chat**

```text
query
  -> keyword ranking over wiki Markdown
  -> optional embedding refresh + vector ranking
  -> graph-related page expansion
  -> result blending and context budget
  -> provider chat / Agent loop
  -> JSON or SSE answer with references and tool events
```

Keyword search always remains available. When embeddings are enabled, chunks
are refreshed by content hash and embedding model, then combined with keyword
and graph signals. The Agent adds permission-aware tools for wiki/source reads,
search, controlled writes, and optional web access.

**Canvas and skills**

Canvas documents are stored as JSON documents in PostgreSQL and autosaved by
the frontend. Node runs and canvas chat stream deltas over SSE. Knowledge-base
nodes reuse project retrieval; URL nodes use the configured fetch provider;
search and image nodes call their corresponding provider capability. The
`LlmSkill` path (currently `/ppt`) is dispatched through `SkillExecutor` to the
optional skill-runner sidecar, which creates a fresh CubeSandbox VM per job and
returns the generated asset.

#### Task execution model

Long-running operations create a row in `project_tasks` with a type, payload,
attempt limit, and audit context. The embedded scheduler claims the oldest
eligible task using PostgreSQL row locking (`FOR UPDATE SKIP LOCKED`) and a
short lease. Executors update `queued`, `running`, `retry_waiting`,
`succeeded`, or `failed` state. Retryable provider/transport errors use
exponential backoff; expired leases are recoverable after process restarts.

### Module map

The repository keeps transport, domain, and UI responsibilities explicit:

| Module | Scope | Key extension points |
| --- | --- | --- |
| `crates/knowledge-core/src/project` | Project roots, safe paths, scaffolding, sources, wiki pages, file history, and project-local state | Add parsers, wiki page rules, or source lifecycle behavior here. |
| `crates/knowledge-core/src/ingest.rs` | Analyze/generate stages, cache checks, traceability, and ingest output parsing | Extend ingest stages or generation formats without touching HTTP routes. |
| `crates/knowledge-core/src/search.rs` | Deterministic Markdown keyword scoring, snippets, title matching, and CJK tokenization | Adjust local ranking or supported text extraction independently. |
| `crates/knowledge-core/src/graph.rs` and `retrieval_graph.rs` | Wiki-link/source relationship graph and related-page expansion | Add graph extraction or ranking signals. |
| `crates/knowledge-core/src/multimodal.rs` and `source_text.rs` | PDF, Office, image extraction, and normalized source text | Add or harden document-format support. |
| `crates/knowledge-core/src/lint.rs`, `reviews.rs`, `dedup.rs` | Structural/semantic quality checks, review state, duplicate detection, and merge planning | Add quality rules while preserving reviewable output. |
| `crates/knowledge-server/src/http` | Axum router, errors, health, and shared HTTP behavior | Register new resource routers and response contracts. |
| `crates/knowledge-server/src/auth` and `tenancy` | Sessions, CSRF, API tokens, spaces, orgs, teams, grants, and access roles | Add authorization rules through the shared principal/access path. |
| `crates/knowledge-server/src/projects` | Project APIs, source watch, task-facing project operations, audit, and file access | Add project resources or operations. |
| `crates/knowledge-server/src/retrieval` | Embedding configuration, chunk persistence, vector ranking, and keyword/vector/graph blending | Add retrieval channels or ranking strategies. |
| `crates/knowledge-server/src/tasks` | Embedded scheduler, leases, retries, recovery, and task executors | Add a task type and executor with explicit retry semantics. |
| `crates/knowledge-server/src/chat` and `agent` | RAG chat, Agent loop, capabilities, skills, tool calls, and SSE events | Add tools/skills with permission and cost limits. |
| `crates/knowledge-server/src/providers` | OpenAI-compatible chat, text, embeddings, and images plus active connection management | Add provider capabilities behind the shared abstraction. |
| `crates/knowledge-server/src/canvas` | Canvas JSON document, assets, node execution, chat streams, and skill jobs | Add node types or execution paths without moving persistence to the UI. |
| `crates/knowledge-server/src/web_search`, `web_fetch`, `mcp` | External search/fetch adapters and MCP protocol/tools | Add an adapter or MCP tool with explicit configuration and authorization. |
| `apps/admin/src/features` | Feature pages, queries, mutations, SSE clients, and operational UI | Add UI flows through the shared API client and route metadata. |
| `packages/api-client` | Typed schemas and HTTP helpers shared by frontend/integrations | Keep wire contracts and validation in sync with backend responses. |
| `packages/mcp-server` | Standalone stdio MCP bridge for clients without direct HTTP MCP support | Add client-side formatting or tool mapping. |
| `services/skill-runner` | Go HTTP sidecar and CubeSandbox lifecycle | Add isolated agentic render workflows; requires Linux/KVM. |

#### Storage model

```text
KNOWLEDGE_PROJECT_ROOT/<project-id>/
├── purpose.md                 Project purpose and research scope
├── schema.md                  Wiki page types and conventions
├── raw/sources/               Imported or watched source material
├── raw/assets/                Source assets
├── wiki/                      Maintained Markdown knowledge layer
│   ├── index.md / log.md / overview.md
│   ├── entities/ concepts/ sources/
│   ├── queries/ comparisons/ synthesis/
└── .knowledge/                Private project-local runtime state
    ├── ingest/                Queue, checkpoints, and cache
    ├── index/                 Local index state
    ├── reviews/               Review items
    └── locks/                 Project locks
```

The database complements this tree rather than replacing it. It contains
`users`, sessions, spaces/orgs/teams, `projects`, project members and grants,
provider/search/fetch settings, `project_tasks`, conversations, embedding
chunks, `canvases`, generated `assets`, and audit logs. `Redis` is not a source
of truth; it is used for cache and coordination.

### API surface

The API is resource-oriented and uses JSON for request/response bodies. Chat,
Agent, and Canvas execution endpoints stream progress with Server-Sent Events
(SSE). Authentication is session-cookie based for the UI and Bearer-token based
for integrations.

| Area | Representative endpoints | Response style |
| --- | --- | --- |
| Health | `GET /api/health` | JSON |
| Authentication | `POST /api/auth/login`, `POST /api/auth/register`, `GET /api/auth/me` | JSON + session cookie |
| Projects and files | `GET/POST /api/projects`, `GET /api/projects/{id}/files`, `GET/PUT /api/projects/{id}/files/content` | JSON / file bytes |
| Sources and tasks | `POST /api/projects/{id}/sources:import`, `POST /api/projects/{id}/ingest`, `GET /api/projects/{id}/tasks` | JSON |
| Retrieval and quality | `POST /api/projects/{id}/search`, `GET /api/projects/{id}/graph`, `GET /api/projects/{id}/reviews` | JSON |
| Chat and Agent | `POST /api/projects/{id}/conversations/{conversation}/messages` | SSE |
| Canvas | `GET/POST /api/canvases`, `PUT /api/canvases/{id}`, `POST /api/canvases/{id}/chat` | JSON / SSE |
| System configuration | `GET/PATCH /api/system/settings`, `/api/system/provider-connections/*` | JSON |
| MCP | `POST /api/mcp` | JSON-RPC over Streamable HTTP |

The complete wire contract is defined by Rust route types and the shared
schemas in `packages/api-client`. Project and system mutation routes enforce
the same authorization path regardless of whether they are called by the UI
or an integration.

### Quick start with Docker

#### Prerequisites

- Docker Engine with the Compose plugin.

#### Start the stack

```bash
docker compose up --build -d
```

The first build downloads a PDFium runtime used for PDF extraction. Check the
service status with:

```bash
docker compose ps
curl http://127.0.0.1:4001/api/health
```

The backend applies pending SQL migrations automatically when it starts.

#### Local endpoints

| Service | URL |
| --- | --- |
| Admin UI | <http://127.0.0.1:4173> |
| Backend API | <http://127.0.0.1:4001> |
| PostgreSQL | `127.0.0.1:55432` |
| Redis | `127.0.0.1:56379` |

#### First login

The development Compose file bootstraps an operator account from
`KNOWLEDGE_ADMIN_PASSWORD` (currently `admin` in `docker-compose.yml`):

- Username: `admin`
- Password: `admin`

Change this value in `docker-compose.yml` before exposing the stack outside a
trusted local network. The password is only used when no `admin` user exists;
changing it later does not change an existing account password.

#### Stop the stack

```bash
docker compose down
```

To remove the database, Redis data, and project volume as well, use
`docker compose down -v`. This permanently deletes local stack data.

### Local development

Docker is the recommended path for a complete local environment. For a faster
edit/test loop, run PostgreSQL and Redis in Docker and start the Rust server and
Vite UI on the host.

#### Prerequisites

- Rust stable (the repository uses the 2024 edition; `rust-toolchain.toml`
  also installs `clippy` and `rustfmt`).
- Node.js 20 or newer and npm.
- Docker, for PostgreSQL 16 and Redis 7.
- A PDFium shared library if you need local PDF extraction. Docker already
  installs one; set `PDFIUM_DYNAMIC_LIB_PATH` when running the server directly.

#### Install dependencies and start infrastructure

```bash
npm install
docker compose up -d postgres redis
```

Start the backend in one terminal:

```bash
# macOS/Linux
KNOWLEDGE_ADMIN_PASSWORD=change-me cargo run -p knowledge-server

# PowerShell
$env:KNOWLEDGE_ADMIN_PASSWORD = "change-me"
cargo run -p knowledge-server
```

Start the admin UI in another terminal:

```bash
npm run dev
```

Vite serves the UI at <http://127.0.0.1:5173> by default and proxies `/api`
requests to `http://127.0.0.1:4001`. If the backend is started without
`KNOWLEDGE_ADMIN_PASSWORD` and the database has no users, create an account via
the registration page or restart it with the variable set.

#### Build and preview

```bash
npm run build
npm run preview --workspace @knowledge/admin
```

The preview server listens on port `4173`.

### Configuration

The backend reads the following environment variables. Defaults are suitable
for the host-based development setup described above.

| Variable | Default | Description |
| --- | --- | --- |
| `KNOWLEDGE_BIND_ADDR` | `127.0.0.1:4001` | API listen address. Compose overrides this to `0.0.0.0:4001`. |
| `KNOWLEDGE_DATABASE_URL` | `postgres://postgres:postgres@127.0.0.1:55432/knowledge?sslmode=disable` | PostgreSQL connection string. |
| `KNOWLEDGE_REDIS_URL` | `redis://127.0.0.1:56379/` | Redis connection string. |
| `KNOWLEDGE_PROJECT_ROOT` | `<cwd>/.e2e` | Root directory for project workspaces. |
| `KNOWLEDGE_ADMIN_PASSWORD` | unset | Bootstrap password for the first `admin` operator only. |
| `KNOWLEDGE_SKILLS_DIR` | `/app/skills` | Read-only baseline skill registry. |
| `KNOWLEDGE_GLOBAL_SKILLS_DIR` | unset | Optional server-wide agent skills; project skills can override matching IDs. |
| `KNOWLEDGE_SKILL_RUNNER_URL` | `http://127.0.0.1:4600` | Optional skill-runner sidecar URL. |
| `KNOWLEDGE_SKILL_WORKER_CONCURRENCY` | `2` | Maximum concurrent skill-render jobs. |
| `KNOWLEDGE_SKILL_JOBS_PER_USER` | `2` | Maximum in-flight skill jobs per user. |
| `PDFIUM_DYNAMIC_LIB_PATH` | platform-dependent | Path to the PDFium shared library for local PDF parsing. |

LLM connections, embeddings, image generation, web search, URL fetching, MCP,
and default language/query settings are stored in PostgreSQL and managed from
**Settings** in the Admin UI. API keys are redacted from settings responses;
do not commit them to the repository.

#### Providers

1. Open **Settings → LLM** and add an OpenAI-compatible connection with a base
   URL, model, timeout, and API key. The first connection becomes active.
2. Optionally enable **Settings → Embedding** for vector retrieval. The
   embedding endpoint must be compatible with the configured provider API.
3. Configure **Settings → Image** if canvas image generation is required.
4. Choose a web search provider under **Settings → Search**. Supported
   providers are Tavily, SerpApi, SearXNG, Ollama, Brave, and Firecrawl.
5. Choose **Firecrawl** under **Settings → Fetch** when canvas URL extraction
   or remote page fetching is needed.

Provider credentials and external service terms are the operator's
responsibility. Search and fetch features remain disabled until configured.

### MCP integration

The Rust backend exposes a Streamable HTTP MCP endpoint at
`POST /api/mcp`. Enable it under **Settings → MCP**, then create a token under
**API Tokens**. Send the token as a Bearer token:

```bash
curl http://127.0.0.1:4001/api/mcp \
  -H 'Authorization: Bearer <knowledge-api-token>' \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}'
```

The native endpoint provides nine tools: status, projects, files, file
reading, reviews, hybrid search, agent chat, graph queries, and source rescans.
Access is constrained by the token's user/project scope, and all tools except
status require MCP to be enabled.

For clients that require a local stdio process, use the standalone package:

```bash
npm install
npm run build --workspace @knowledge/mcp-server
```

Set `KNOWLEDGE_API_BASE_URL` (default `http://127.0.0.1:4001`) and
`KNOWLEDGE_API_TOKEN`, then point your MCP client at
`packages/mcp-server/dist/src/index.js`. See the detailed examples for
[Claude Code and Codex CLI](packages/mcp-server/README.md).

### Optional skill runner

The `/ppt` canvas workflow uses `services/skill-runner`, which launches a
CubeSandbox micro-VM and runs the configured agent skill. It requires an
x86_64 Linux host with KVM and is intentionally not part of the default
Compose stack. Without a reachable sidecar, other features continue to work
but PPT rendering fails with a skill-runner availability error.

See the [skill-runner README](services/skill-runner/README.md) and the
[deployment guide](docs/skill-runner/deploy.md) for CubeSandbox requirements,
environment variables, and deployment steps.

### Testing and quality checks

Run the standard workspace checks from the repository root:

```bash
npm test
npm run lint
npm run build
```

The web integration suite starts its own mock provider and server. It needs
PostgreSQL and Redis; the default test setup uses the same local ports as the
Compose services. The live-provider smoke tests are skipped unless explicitly
enabled:

```bash
KNOWLEDGE_RUN_REAL_PROVIDER_SMOKE=1 \
KNOWLEDGE_PROVIDER_BASE_URL=https://api.example.com/v1 \
KNOWLEDGE_PROVIDER_API_KEY=<key> \
KNOWLEDGE_PROVIDER_MODEL=<model> \
cargo test -p rust-integration --test real_provider_smoke
```

Do not use live credentials in CI logs or committed configuration.

### Data and security notes

- The Compose file is for local development. It publishes PostgreSQL, Redis,
  and the API without TLS; put a reverse proxy and network controls in front
  of any non-local deployment.
- Replace the development `admin` password before sharing the service.
- Treat API tokens, provider keys, session cookies, and project files as
  secrets. Revoke tokens from **API Tokens** when no longer needed.
- `docker compose down -v` deletes local persistent volumes. Back up the
  PostgreSQL and project volumes before using it.

### Repository layout

```text
apps/admin/                 React/Vite administration UI
packages/api-client/        Shared TypeScript API client and schemas
packages/mcp-server/        Standalone stdio MCP server
crates/knowledge-core/      Parsing, ingestion, retrieval, graph, and wiki logic
crates/knowledge-server/    Rust HTTP API and background runtime
crates/knowledge-worker/    Workspace worker crate placeholder
services/skill-runner/      Optional CubeSandbox skill-rendering sidecar
tests/rust-integration/     Rust API/integration tests
tests/web/                  Playwright end-to-end tests
docs/                       Deployment and design documentation
```

### Contributing

1. Create a focused branch from `main`.
2. Make the smallest change that addresses the issue and add or update tests.
3. Run `npm run lint`, `npm test`, and any targeted integration tests.
4. Open a pull request describing behavior changes, migration impact, and
   configuration requirements.

Please avoid committing generated build output, local databases, provider
credentials, or secrets.

### License

Knowledge is released under the [MIT License](https://opensource.org/license/mit/).

## 简体中文

Knowledge 是一个可自托管的知识工作台：用于收集资料、构建可检索的
Markdown 知识库，并通过兼容 OpenAI API 的模型服务完成问答、研究和知识维护。

> 当前项目仍处于早期阶段，版本之间可能调整界面和配置方式。

### 功能概览

- 按项目隔离的工作区，支持 Markdown Wiki 页面和原始资料。
- 支持文本、PDF 和常见 Office 资料解析为可检索内容，并可将提取的媒体保留在项目工作区。
- 资料导入、后台摄取、增量重扫和 Source Watch 自动监测。
- 关键词、向量和知识图谱结合的混合检索，用于搜索和对话。
- 通过 OpenAI-compatible 接口提供聊天、深度研究、多模态解析和图片生成。
- 知识图谱浏览、反向链接上下文、Lint、Review 队列和重复内容合并。
- 多用户项目、组织、团队、项目角色、API Token、审计日志和项目级权限。
- React/Vite 管理端与 Rust/Axum HTTP API。
- 原生 `/api/mcp` MCP 端点，以及可选的 Node.js 独立 MCP 服务，可接入
  Claude Code、Codex CLI、Cursor 等客户端。

### 产品说明

Knowledge 面向需要“有证据、可维护”的知识库团队，而不仅是一个聊天窗口。
系统将原始资料、生成的 Wiki 页面和运行元数据分开保存，让 AI 输出可追溯、可
审核、可持续更新，同时保留原始依据。

#### 适用场景

| 场景 | Knowledge 提供的能力 |
| --- | --- |
| 研究与尽调 | 摄取论文、文档和网页资料，让结论与来源保持关联。 |
| 工程与项目文档 | 监测本地目录、维护 Markdown 页面，并通过图谱浏览依赖关系。 |
| 知识运营 | 将 Lint 结果和不确定事实送入 Review，合并重复页面，查看任务/审计历史。 |
| Agent 工作流 | 为 MCP 兼容 Agent 提供受范围限制的搜索、读取、聊天和重扫能力，无需共享数据库凭据。 |

#### 核心概念

| 概念 | 产品含义 |
| --- | --- |
| Account | 通过 session cookie 或 API Token 认证的用户身份。 |
| Space | 拥有项目的个人、组织或团队命名空间。 |
| Project | 独立的知识库，包含自己的文件、成员、任务和检索上下文。 |
| Source | 位于 `raw/sources/` 的导入或监测资料，是摄取流程的证据层。 |
| Wiki | 位于 `wiki/` 的可读 Markdown 知识层，可由系统生成并由人维护。 |
| Task | 持久化的异步操作，例如摄取、重扫、Lint、去重或研究。 |
| Canvas | 按用户隔离的 JSON 文档，包含笔记、URL、知识库引用、搜索结果和 AI 节点。 |
| Provider connection | 服务端 AI 工作流使用的当前 OpenAI-compatible 模型端点。 |

#### 典型使用流程

1. 注册账号，在个人、组织或团队 Space 中创建项目。
2. 导入资料，或为本地目录配置 Source Watch。
3. 执行摄取。服务端解析资料、调用已配置的 Provider、生成带来源关系的 Wiki
   页面，并在需要时记录 Review 项。
4. 使用 Search、Chat、Graph 或 Deep Research 探索项目；Chat 会引用检索到的
   Wiki 页面，并沿图谱关系扩展上下文。
5. 使用 Lint、Review 和 Dedup 修复坏链、处理知识缺口、合并重复页面。
6. 为协作者授予项目权限，或通过 MCP/API Token 连接外部 Agent。

#### 产品边界

- 项目文件是持久内容边界。PostgreSQL 保存元数据、权限、配置、任务状态、会话、
  Embedding 块和生成的 Asset，不替代项目文件树。
- AI 能力依赖 Provider。没有 LLM 时仍可导入和浏览文件；生成、聊天、Embedding
  和 Deep Research 需要配置对应能力。
- Canvas 当前仅对所有者可见。数据库预留了访问表，但尚未开放协作编辑。
- Worker 当前与后端同进程运行；拆分为独立服务是未来部署选项，不是当前必需条件。

### 技术架构

当前实现是“Rust 单体后端 + React 管理端 + 可选 sidecar”的部署形态：

```text
Admin UI (React/Vite) ── REST / SSE ──┐
                                      ▼
MCP clients ─────── Bearer token ─► knowledge-server (Rust/Axum)
                                      │
          ┌───────────────────────────┼───────────────────────────┐
          ▼                           ▼                           ▼
   auth + tenancy              domain services                 tasks
   + CSRF + roles       projects/chat/agent/canvas       scheduler + executors
          │                           │                           │
          └───────────────┬───────────┴───────────────┬───────────┘
                          ▼                           ▼
                  knowledge-core                 PostgreSQL + Redis
             parsing/search/graph/wiki       metadata/index/tasks/cache
                          │
                          ▼
                    project files
                          │
                          └── optional skill-runner (Go) → CubeSandbox (Linux/KVM)
```

#### 运行时职责

- **HTTP 边界：** `http::router` 汇总健康检查、REST 资源、SSE 流和 Streamable
  HTTP MCP 端点；请求在进入服务层前完成 schema 校验。
- **身份与授权：** `auth` 将 session 或 Bearer Token 解析为 principal；session
  写操作必须带 CSRF，API Token 可以是全局或单项目范围。`tenancy::access` 根据
  personal/org/team Space 所有权和项目 grant 计算 `Owner`、`Editor`、`Viewer`。
- **领域服务：** `projects`、`chat`、`agent`、`canvas`、`deep_research`、
  `retrieval`、`providers`、`web_search`、`web_fetch` 和 `mcp` 负责产品行为，
  route 模块只处理传输层。
- **知识核心：** `knowledge-core` 提供安全路径、解析、摄取、搜索、图谱、Review、
  Lint、去重和 Wiki 维护等逻辑，不依赖 HTTP 或前端。
- **持久化：** 启动时自动执行 SQL migration。PostgreSQL 保存账号、租户、设置、
  任务、会话、Embedding 块、Canvas JSON、Asset 和审计日志；项目内容位于
  `KNOWLEDGE_PROJECT_ROOT`。
- **运行时协调：** Redis 提供缓存和任务唤醒信号；调度器领取带 lease 的任务，
  进程重启后可恢复过期任务。

#### 主要数据流

**资料摄取与维护：**

```text
导入资料 / Source Watch
        -> raw/sources/
        -> 创建 project task
        -> 文本或多模态解析
        -> Provider 分析与 Wiki 生成
        -> wiki/ + Review + 图谱/索引刷新
        -> task 结果与审计日志
```

Source Watch 根据项目本地快照比较文件 hash 和大小，只复制或删除发生变化的文件，
并将摄取/删除操作放入任务队列，缩短 HTTP 请求时间。PDF 和 Office 资料由
`knowledge-core` 解析，PDF 图片提取使用 PDFium。

**搜索与聊天：**

```text
查询 -> Wiki 关键词排序 -> 可选 Embedding 刷新与向量排序
     -> 图谱相关页面扩展 -> 结果融合与上下文预算
     -> Provider Chat / Agent loop -> JSON 或带引用的 SSE 响应
```

关键词搜索始终可用。开启 Embedding 后，服务按内容 hash 和模型刷新块，再融合关键词、
向量和图谱信号；Agent 额外提供受权限控制的 Wiki/Source 读取、搜索、写入和可选联网工具。

**Canvas 与技能：**

Canvas 文档以 JSON 形态存入 PostgreSQL，由前端自动保存；节点运行和 Canvas Chat 通过
SSE 发送增量结果。知识库节点复用项目检索，URL 节点使用 Fetch Provider，Search/Image
节点调用对应能力。`LlmSkill`（当前为 `/ppt`）通过 `SkillExecutor` 调用可选的
Skill runner；sidecar 为每个任务创建独立 CubeSandbox VM，并返回生成的 Asset。

#### 任务执行模型

长任务先在 `project_tasks` 中写入类型、payload、重试次数和审计上下文。内嵌调度器通过
PostgreSQL 行锁（`FOR UPDATE SKIP LOCKED`）领取最早可执行任务，并设置短 lease。执行器
维护 `queued`、`running`、`retry_waiting`、`succeeded`、`failed` 状态；Provider 或网络
临时错误使用指数退避，lease 过期的任务可在重启后恢复。

### 模块划分

| 模块 | 职责范围 | 扩展方式 |
| --- | --- | --- |
| `crates/knowledge-core/src/project` | 项目根目录、安全路径、脚手架、Source、Wiki、文件历史和项目状态 | 增加解析器、Wiki 规则或 Source 生命周期逻辑 |
| `crates/knowledge-core/src/ingest.rs` | 分析/生成阶段、缓存、溯源和摄取输出解析 | 增加摄取阶段或输出格式 |
| `crates/knowledge-core/src/search.rs` | Markdown 关键词评分、标题匹配、摘要和 CJK 分词 | 调整本地排序和文本支持 |
| `crates/knowledge-core/src/graph.rs` / `retrieval_graph.rs` | Wiki 链接/Source 关系图和相关页面扩展 | 增加图谱提取或排序信号 |
| `crates/knowledge-core/src/multimodal.rs` / `source_text.rs` | PDF、Office、图片提取和文本标准化 | 增加或强化文档格式支持 |
| `crates/knowledge-core/src/lint.rs` / `reviews.rs` / `dedup.rs` | 质量检查、Review 状态、重复检测和合并计划 | 增加可审核的质量规则 |
| `crates/knowledge-server/src/http` | Axum 路由、错误、健康检查和通用 HTTP 行为 | 注册资源路由和响应契约 |
| `crates/knowledge-server/src/auth` / `tenancy` | Session、CSRF、API Token、Space、组织、团队、Grant 和角色 | 通过统一 principal/access 路径增加授权规则 |
| `crates/knowledge-server/src/projects` | 项目 API、Source Watch、文件访问、任务操作和审计 | 增加项目资源或操作 |
| `crates/knowledge-server/src/retrieval` | Embedding 配置、块持久化、向量排序和多路结果融合 | 增加检索通道或排序策略 |
| `crates/knowledge-server/src/tasks` | 调度器、lease、重试、恢复和执行器 | 增加任务类型并定义重试语义 |
| `crates/knowledge-server/src/chat` / `agent` | RAG Chat、Agent loop、能力、技能、工具调用和 SSE 事件 | 在权限和成本限制下增加工具/技能 |
| `crates/knowledge-server/src/providers` | OpenAI-compatible Chat、文本、Embedding、图片和连接管理 | 在统一抽象下增加 Provider 能力 |
| `crates/knowledge-server/src/canvas` | Canvas JSON、Asset、节点执行、Chat 流和技能任务 | 增加节点类型或执行路径 |
| `crates/knowledge-server/src/web_search` / `web_fetch` / `mcp` | 外部搜索/抓取适配器与 MCP 协议/工具 | 增加有配置和权限边界的适配器/工具 |
| `apps/admin/src/features` | 功能页面、查询、Mutation、SSE 客户端和运维界面 | 通过共享 API client 增加 UI 流程 |
| `packages/api-client` | 前端和集成共用的类型 schema 与 HTTP 工具 | 与后端响应同步维护 wire contract |
| `packages/mcp-server` | 面向不支持 HTTP MCP 客户端的独立 stdio 桥接 | 增加格式化或工具映射 |
| `services/skill-runner` | Go HTTP sidecar 和 CubeSandbox 生命周期 | 增加隔离的 Agent 技能渲染流程 |

#### 存储模型

```text
KNOWLEDGE_PROJECT_ROOT/<project-id>/
├── purpose.md                 项目目的和研究范围
├── schema.md                  Wiki 页面类型和约定
├── raw/sources/               导入或监测的原始资料
├── raw/assets/                原始资料附件
├── wiki/                      维护中的 Markdown 知识层
│   ├── index.md / log.md / overview.md
│   ├── entities/ concepts/ sources/
│   ├── queries/ comparisons/ synthesis/
└── .knowledge/                私有的项目运行状态
    ├── ingest/                队列、checkpoint 和缓存
    ├── index/                 本地索引状态
    ├── reviews/               Review 项
    └── locks/                 项目锁
```

数据库存储 `users`、session、Space/组织/团队、`projects`、成员与 grant、Provider/
Search/Fetch 设置、`project_tasks`、会话、Embedding 块、`canvases`、生成的 `assets`
和审计日志。Redis 不是事实源，只用于缓存和协调；项目内容的事实源仍是项目文件树。

### API 能力面

API 采用面向资源的设计，请求和响应主体使用 JSON；Chat、Agent 和 Canvas 执行接口通过
Server-Sent Events（SSE）推送进度。管理端使用 session cookie，外部集成使用 Bearer Token。

| 能力 | 代表性接口 | 响应形式 |
| --- | --- | --- |
| 健康检查 | `GET /api/health` | JSON |
| 认证 | `POST /api/auth/login`、`POST /api/auth/register`、`GET /api/auth/me` | JSON + session cookie |
| 项目与文件 | `GET/POST /api/projects`、`GET /api/projects/{id}/files`、`GET/PUT /api/projects/{id}/files/content` | JSON / 文件字节 |
| Source 与任务 | `POST /api/projects/{id}/sources:import`、`POST /api/projects/{id}/ingest`、`GET /api/projects/{id}/tasks` | JSON |
| 检索与质量 | `POST /api/projects/{id}/search`、`GET /api/projects/{id}/graph`、`GET /api/projects/{id}/reviews` | JSON |
| Chat 与 Agent | `POST /api/projects/{id}/conversations/{conversation}/messages` | SSE |
| Canvas | `GET/POST /api/canvases`、`PUT /api/canvases/{id}`、`POST /api/canvases/{id}/chat` | JSON / SSE |
| 系统配置 | `GET/PATCH /api/system/settings`、`/api/system/provider-connections/*` | JSON |
| MCP | `POST /api/mcp` | Streamable HTTP 上的 JSON-RPC |

完整 wire contract 由 Rust route 类型和 `packages/api-client` 中的共享 schema 定义。
无论来自管理端还是外部集成，项目和系统写操作都经过同一套授权路径。

### Docker 快速开始

#### 前置条件

- Docker Engine 和 Compose 插件。

#### 启动

```bash
docker compose up --build -d
```

首次构建会下载用于 PDF 解析的 PDFium 运行库。查看状态和健康检查：

```bash
docker compose ps
curl http://127.0.0.1:4001/api/health
```

后端启动时会自动执行尚未应用的 SQL migration。

#### 本地地址

| 服务 | 地址 |
| --- | --- |
| 管理端 | <http://127.0.0.1:4173> |
| 后端 API | <http://127.0.0.1:4001> |
| PostgreSQL | `127.0.0.1:55432` |
| Redis | `127.0.0.1:56379` |

#### 首次登录

Docker Compose 会根据 `KNOWLEDGE_ADMIN_PASSWORD` 创建首个 operator 账号；
当前 `docker-compose.yml` 中的开发值是 `admin`：

- 用户名：`admin`
- 密码：`admin`

在暴露到非本地网络前，请先修改 `docker-compose.yml` 中的值。该变量只在
数据库尚无 `admin` 用户时生效，之后修改它不会修改已有账号密码。

#### 停止

```bash
docker compose down
```

如需连同数据库、Redis 和项目卷一起删除，使用 `docker compose down -v`。
这会永久删除本地数据。

### 本地开发

推荐使用 Docker 运行完整栈。若需要更快的编辑/测试循环，可以只用 Docker
启动 PostgreSQL 和 Redis，再在主机上运行 Rust 后端和 Vite 管理端。

#### 前置条件

- Rust stable（仓库使用 2024 edition，`rust-toolchain.toml` 会安装
  `clippy` 和 `rustfmt`）。
- Node.js 20+ 和 npm。
- Docker，用于 PostgreSQL 16 和 Redis 7。
- 若要在主机直接解析 PDF，需要 PDFium 动态库；Docker 已自动安装，主机运行
  时请设置 `PDFIUM_DYNAMIC_LIB_PATH`。

#### 安装依赖并启动基础设施

```bash
npm install
docker compose up -d postgres redis
```

在一个终端启动后端：

```bash
# macOS/Linux
KNOWLEDGE_ADMIN_PASSWORD=change-me cargo run -p knowledge-server

# PowerShell
$env:KNOWLEDGE_ADMIN_PASSWORD = "change-me"
cargo run -p knowledge-server
```

在另一个终端启动管理端：

```bash
npm run dev
```

Vite 默认地址为 <http://127.0.0.1:5173>，并将 `/api` 请求代理到
`http://127.0.0.1:4001`。如果后端未设置 `KNOWLEDGE_ADMIN_PASSWORD` 且数据库
中没有用户，可以通过注册页面创建账号，或设置变量后重启后端。

#### 构建与预览

```bash
npm run build
npm run preview --workspace @knowledge/admin
```

预览服务监听 `4173` 端口。

### 配置

后端读取以下环境变量。默认值适用于上面的主机开发环境：

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `KNOWLEDGE_BIND_ADDR` | `127.0.0.1:4001` | API 监听地址；Compose 会覆盖为 `0.0.0.0:4001`。 |
| `KNOWLEDGE_DATABASE_URL` | `postgres://postgres:postgres@127.0.0.1:55432/knowledge?sslmode=disable` | PostgreSQL 连接串。 |
| `KNOWLEDGE_REDIS_URL` | `redis://127.0.0.1:56379/` | Redis 连接串。 |
| `KNOWLEDGE_PROJECT_ROOT` | `<cwd>/.e2e` | 项目工作区根目录。 |
| `KNOWLEDGE_ADMIN_PASSWORD` | 未设置 | 仅用于创建首个 `admin` operator 账号。 |
| `KNOWLEDGE_SKILLS_DIR` | `/app/skills` | 只读的基础技能目录。 |
| `KNOWLEDGE_GLOBAL_SKILLS_DIR` | 未设置 | 可选的全局 Agent 技能目录；项目技能可覆盖同 ID 技能。 |
| `KNOWLEDGE_SKILL_RUNNER_URL` | `http://127.0.0.1:4600` | 可选 Skill runner sidecar 地址。 |
| `KNOWLEDGE_SKILL_WORKER_CONCURRENCY` | `2` | 同时执行的技能渲染任务数。 |
| `KNOWLEDGE_SKILL_JOBS_PER_USER` | `2` | 每个用户的在途技能任务上限。 |
| `PDFIUM_DYNAMIC_LIB_PATH` | 依平台而定 | 主机直接运行时的 PDFium 动态库路径。 |

LLM、Embedding、图片生成、网页搜索、URL 抓取、MCP 以及默认语言/检索条数
都存储在 PostgreSQL 中，并通过管理端 **Settings** 配置。设置 API 会隐藏
API key；不要把密钥提交到仓库。

#### Provider 配置

1. 打开 **Settings → LLM**，添加 OpenAI-compatible 连接，填写 Base URL、模型、
   超时和 API key。第一个连接会自动激活。
2. 如需向量检索，在 **Settings → Embedding** 启用 Embedding，并确保接口兼容
   当前 Provider API。
3. 需要画布生成图片时，在 **Settings → Image** 配置图片模型。
4. 在 **Settings → Search** 选择网页搜索 Provider。支持 Tavily、SerpApi、
   SearXNG、Ollama、Brave 和 Firecrawl。
5. 需要画布 URL 提取或远程页面抓取时，在 **Settings → Fetch** 选择 Firecrawl。

Provider 凭据和第三方服务条款由部署者负责。未配置时，搜索和抓取能力保持禁用。

### MCP 集成

Rust 后端在 `POST /api/mcp` 暴露 Streamable HTTP MCP 端点。先在
**Settings → MCP** 开启，再到 **API Tokens** 创建 Token，并以 Bearer Token 调用：

```bash
curl http://127.0.0.1:4001/api/mcp \
  -H 'Authorization: Bearer <knowledge-api-token>' \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}'
```

原生端点提供 9 个工具：状态、项目、文件、文件读取、Review、混合搜索、Agent
聊天、图谱查询和资料重扫。权限受 Token 的用户/项目范围限制；除 status 外，
其余工具都要求 MCP 已开启。

如果客户端需要本地 stdio 进程，可使用独立包：

```bash
npm install
npm run build --workspace @knowledge/mcp-server
```

设置 `KNOWLEDGE_API_BASE_URL`（默认 `http://127.0.0.1:4001`）和
`KNOWLEDGE_API_TOKEN`，然后将 MCP 客户端指向
`packages/mcp-server/dist/src/index.js`。Claude Code 和 Codex CLI 示例见
[独立 MCP 文档](packages/mcp-server/README.md)。

### 可选 Skill runner

画布的 `/ppt` 工作流使用 `services/skill-runner`：它会启动 CubeSandbox 微虚拟机
并执行技能。该组件需要带 KVM 的 x86_64 Linux 主机，刻意没有加入默认 Compose
服务。没有可访问的 sidecar 时，其他功能仍可用，但 PPT 渲染会返回 Skill runner
不可用错误。

详见 [skill-runner README](services/skill-runner/README.md) 和
[部署指南](docs/skill-runner/deploy.md)。

### 测试与质量检查

在仓库根目录执行：

```bash
npm test
npm run lint
npm run build
```

Web 集成测试会启动 mock provider 和服务，需要 PostgreSQL、Redis；默认测试配置
使用与 Compose 相同的本地端口。真实 Provider 冒烟测试默认跳过，显式开启方式：

```bash
KNOWLEDGE_RUN_REAL_PROVIDER_SMOKE=1 \
KNOWLEDGE_PROVIDER_BASE_URL=https://api.example.com/v1 \
KNOWLEDGE_PROVIDER_API_KEY=<key> \
KNOWLEDGE_PROVIDER_MODEL=<model> \
cargo test -p rust-integration --test real_provider_smoke
```

不要在 CI 日志或提交的配置中暴露真实凭据。

### 数据与安全说明

- Compose 文件面向本地开发，会直接暴露 PostgreSQL、Redis 和 API，且没有 TLS；
  非本地部署应放在反向代理和网络访问控制之后。
- 对外提供服务前必须替换开发用 `admin` 密码。
- API Token、Provider key、session cookie 和项目文件都应视为敏感数据；不再使用的
  Token 应从 **API Tokens** 页面撤销。
- `docker compose down -v` 会删除本地持久化卷，执行前请备份 PostgreSQL 和项目卷。

### 仓库结构

```text
apps/admin/                 React/Vite 管理端
packages/api-client/        TypeScript API client 与 schema
packages/mcp-server/        独立 stdio MCP 服务
crates/knowledge-core/      解析、摄取、检索、图谱和 Wiki 逻辑
crates/knowledge-server/    Rust HTTP API 与后台运行时
crates/knowledge-worker/    workspace worker crate 占位
services/skill-runner/      可选 CubeSandbox 技能渲染 sidecar
tests/rust-integration/     Rust API/集成测试
tests/web/                  Playwright 端到端测试
docs/                       部署与设计文档
```

### 参与贡献

1. 从 `main` 创建聚焦的分支。
2. 以最小范围完成变更，并补充或更新测试。
3. 执行 `npm run lint`、`npm test` 和相关集成测试。
4. 提交 Pull Request，说明行为变化、迁移影响和配置要求。

请勿提交构建产物、本地数据库、Provider 凭据或其他机密信息。

### 许可证

Knowledge 采用 [MIT License](https://opensource.org/license/mit/) 发布。
