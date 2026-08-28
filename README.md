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
