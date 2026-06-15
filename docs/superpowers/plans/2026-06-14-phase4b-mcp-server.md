# Phase 4b — MCP Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a standalone Model Context Protocol server that lets external agent clients (Claude Code, Codex CLI, Cursor, etc.) browse, search, and manage a project via the REST API, authenticating with the per-user/per-project Bearer tokens from Phase 4a.

**Architecture:** A new npm workspace package `packages/mcp-server` (`@knowledge/mcp-server`) ports `upstream_llm_wiki/mcp-server` verbatim where possible. It exposes 8 stdio tools (status/projects/files/read_file/reviews/search/graph/rescan_sources) via `@modelcontextprotocol/sdk`. A thin `KnowledgeApiClient` wraps `fetch` with `Authorization: Bearer ${KNOWLEDGE_API_TOKEN}` and our REST shape. A small change to `crates/knowledge-server/src/http/router.rs` extends `/api/health` to return a JSON body so the MCP `status` tool has something to parse without changing observable behavior for any existing client.

**Tech Stack:** Node.js 20+, TypeScript 5.9, `@modelcontextprotocol/sdk` ^1.29, vitest. Rust change is a one-line handler tweak in `knowledge-server`.

**Upstream references (memory rule: port, don't invent):**
- `upstream_llm_wiki/mcp-server/src/index.ts` — tool definitions, dispatch, formatters (formatFileTree, formatSearchResults, formatReviews, formatGraph), MCP error mapping. Port verbatim, only adjust 3 things: (a) `current` project default is rejected — every tool requires an explicit `project_id` (we are multi-tenant); (b) drop `assertMcpEnabled()` — server enables MCP whenever any token is presented; (c) tool name prefix `llm_wiki_` → `knowledge_`.
- `upstream_llm_wiki/mcp-server/src/api-client.ts` — request shape (Bearer header, /api/v1 prefix, JSON error parsing). Port verbatim, adjust the path prefix to `/api` (we don't version), and adjust the rescan endpoint suffix (`:rescan` not `/rescan`).
- `upstream_llm_wiki/mcp-server/test/api-client.test.ts` — mocked-fetch test patterns. Port semantically (we use vitest, not node:test).
- `upstream_llm_wiki/mcp-server/package.json` — devDeps, scripts.

**Documented divergences from upstream:**
1. **No "current project" semantics.** This server is multi-tenant; there is no single active project. Every tool's `project_id` argument is **required**, and the MCP schema marks it so. Tools that upstream defaulted to `current` now reject `current` with `ErrorCode.InvalidParams`.
2. **No `mcpEnabled` toggle.** Upstream guards every tool with a desktop-only setting because its API is open by default; ours is closed by default (Bearer token required for every protected route), so the MCP tools are gated by token possession alone.
3. **Path prefix `/api/` (not `/api/v1/`).** Our REST routes don't version.
4. **Rescan endpoint:** `POST /api/projects/{id}/sources:rescan` (axum literal-suffix routing — same convention as `reviews:sweep`, `dedup:detect`).
5. **Tool name prefix `knowledge_*`** instead of `llm_wiki_*` — matches this project's identity. Tool count stays at 8.
6. **`/api/health` returns JSON body** `{"ok": true, "service": "knowledge-server"}` instead of bare 200. Adding a body to a 200 response doesn't break any caller that only checked status; it gives the MCP `status` tool something to render.
7. **No "8th tool" omission.** The roadmap text mentions 7 tools, but upstream ships 8 (it adds `rescan_sources`). We port all 8 to stay aligned with upstream — the extra tool is small, useful, and free.

**Indentation conventions:** TS/TSX = 2-space. `crates/**` source = 2-space. `tests/rust-integration/**` = 4-space.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `package.json` | Modify | Add `packages/*` workspace already covers it; no change needed unless workspace glob excludes mcp-server (verify) |
| `packages/mcp-server/package.json` | Create | `@knowledge/mcp-server`, deps on `@modelcontextprotocol/sdk`, devDeps vitest + typescript |
| `packages/mcp-server/tsconfig.json` | Create | NodeNext, ES2022, declaration off, outDir `dist` |
| `packages/mcp-server/src/api-client.ts` | Create | `KnowledgeApiClient` — port of upstream `LlmWikiApiClient`, our REST shape |
| `packages/mcp-server/src/index.ts` | Create | MCP server: 8 tools, stdio transport |
| `packages/mcp-server/src/formatters.ts` | Create | Pure text formatters (port `formatFileTree`, `formatSearchResults`, `formatReviews`, `formatGraph`) |
| `packages/mcp-server/test/api-client.test.ts` | Create | vitest unit tests for client (mocked fetch) |
| `packages/mcp-server/test/formatters.test.ts` | Create | vitest unit tests for formatters |
| `packages/mcp-server/test/server.test.ts` | Create | vitest spawn-stdio integration test — boots the MCP server as a child process, talks via `@modelcontextprotocol/sdk/client/stdio.js`, verifies `tools/list` and one `tools/call` round-trip against a fake HTTP server |
| `packages/mcp-server/README.md` | Create | Install / configure / Claude Code + Codex snippets |
| `crates/knowledge-server/src/http/router.rs` | Modify | `/api/health` → JSON body `{"ok": true, "service": "knowledge-server"}` |
| `tests/rust-integration/tests/health_api.rs` | Create | Verify the new health JSON body without breaking the StatusCode contract |

---

### Task 1: Workspace scaffolding

**Files:**
- Create: `packages/mcp-server/package.json`
- Create: `packages/mcp-server/tsconfig.json`
- Verify: `package.json` workspaces glob already includes `packages/*`

Upstream reference: `upstream_llm_wiki/mcp-server/package.json` for dep versions and scripts.

- [ ] **Step 1: Verify workspace glob covers the new package**

```bash
grep -A3 workspaces package.json
```

Expected: shows `"packages/*"` in the array. If not, add `"packages/mcp-server"` to the array.

- [ ] **Step 2: Create `packages/mcp-server/package.json`**

```json
{
  "name": "@knowledge/mcp-server",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "main": "./dist/src/index.js",
  "bin": {
    "knowledge-mcp": "./dist/src/index.js"
  },
  "scripts": {
    "build": "tsc -p tsconfig.json",
    "typecheck": "tsc -p tsconfig.json --noEmit",
    "test": "vitest run"
  },
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.29.0"
  },
  "devDependencies": {
    "@types/node": "^20.0.0",
    "typescript": "^5.9.3",
    "vitest": "^4.0.8"
  }
}
```

- [ ] **Step 3: Create `packages/mcp-server/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "outDir": "dist",
    "rootDir": ".",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true,
    "declaration": false
  },
  "include": ["src/**/*.ts", "test/**/*.ts"]
}
```

- [ ] **Step 4: Install**

Run: `npm install`
Expected: lockfile updates, `@modelcontextprotocol/sdk` lands in `node_modules`. No build errors.

- [ ] **Step 5: Verify the workspace registers**

Run: `npm run typecheck --workspace @knowledge/mcp-server`
Expected: typecheck succeeds (empty package — no source yet, so this should just exit 0).

- [ ] **Step 6: Commit**

```bash
git add packages/mcp-server/package.json packages/mcp-server/tsconfig.json package-lock.json
git commit -m "feat: scaffold @knowledge/mcp-server workspace package"
```

---

### Task 2: KnowledgeApiClient

**Files:**
- Create: `packages/mcp-server/src/api-client.ts`
- Create: `packages/mcp-server/test/api-client.test.ts`

Upstream reference: `upstream_llm_wiki/mcp-server/src/api-client.ts` — port the request/response shape, the Bearer header logic, and the JSON error wrapping verbatim. Two divergences: path prefix `/api/` (not `/api/v1/`) and rescan suffix `:rescan` (not `/rescan`).

- [ ] **Step 1: Write the failing tests**

Create `packages/mcp-server/test/api-client.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import { KnowledgeApiClient, normalizeBaseUrl } from "../src/api-client.js";

describe("normalizeBaseUrl", () => {
  it("trims trailing slashes", () => {
    expect(normalizeBaseUrl("http://127.0.0.1:4001///")).toBe("http://127.0.0.1:4001");
  });

  it("falls back to the default when empty", () => {
    expect(normalizeBaseUrl("")).toBe("http://127.0.0.1:4001");
  });
});

describe("KnowledgeApiClient.projects", () => {
  it("sends Bearer token and parses the list", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = [];
    const fetchImpl = (async (url: string | URL | Request, init?: RequestInit) => {
      calls.push({ url: String(url), init });
      return new Response(
        JSON.stringify({ projects: [{ id: "p1", name: "Demo", rootPath: "/tmp/demo" }] }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    }) as typeof fetch;

    const client = new KnowledgeApiClient({
      baseUrl: "http://localhost:4001/",
      token: "secret",
      fetchImpl,
    });
    const result = await client.projects();

    expect(calls[0]?.url).toBe("http://localhost:4001/api/projects");
    expect((calls[0]?.init?.headers as Record<string, string>).Authorization).toBe("Bearer secret");
    expect(result.projects[0]?.id).toBe("p1");
  });
});

describe("KnowledgeApiClient.health", () => {
  it("does not send Authorization", async () => {
    const seen: Array<RequestInit | undefined> = [];
    const fetchImpl = (async (_url: unknown, init?: RequestInit) => {
      seen.push(init);
      return new Response(JSON.stringify({ ok: true, service: "knowledge-server" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }) as typeof fetch;

    const client = new KnowledgeApiClient({ token: "secret", fetchImpl });
    await client.health();

    expect((seen[0]?.headers as Record<string, string> | undefined)?.Authorization).toBeUndefined();
  });
});

describe("KnowledgeApiClient.search", () => {
  it("posts JSON body", async () => {
    let body = "";
    const fetchImpl = (async (_url: unknown, init?: RequestInit) => {
      body = String(init?.body ?? "");
      return new Response(
        JSON.stringify({
          mode: "hybrid",
          tokenHits: 2,
          vectorHits: 1,
          results: [{ path: "wiki/a.md", title: "A", snippet: "hit", score: 0.5, vectorScore: 0.9 }],
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    }) as typeof fetch;

    const client = new KnowledgeApiClient({ token: "secret", fetchImpl });
    const result = await client.search("p1", "query", { topK: 5 });

    expect(JSON.parse(body)).toEqual({ query: "query", topK: 5 });
    expect(result.results[0]?.path).toBe("wiki/a.md");
  });
});

describe("KnowledgeApiClient.rescan", () => {
  it("posts to the :rescan suffix endpoint", async () => {
    const calls: string[] = [];
    const fetchImpl = (async (url: unknown) => {
      calls.push(String(url));
      return new Response(JSON.stringify({ taskId: "task-1", status: "queued" }), {
        status: 202,
        headers: { "content-type": "application/json" },
      });
    }) as typeof fetch;

    const client = new KnowledgeApiClient({ token: "secret", fetchImpl });
    await client.rescan("p1");

    expect(calls[0]).toBe("http://127.0.0.1:4001/api/projects/p1/sources:rescan");
  });
});

describe("KnowledgeApiClient error handling", () => {
  it("wraps HTTP errors with the response message", async () => {
    const fetchImpl = (async () =>
      new Response(JSON.stringify({ error: "not a project member" }), {
        status: 403,
        headers: { "content-type": "application/json" },
      })) as typeof fetch;

    const client = new KnowledgeApiClient({ token: "secret", fetchImpl });
    await expect(client.projects()).rejects.toThrow(/403.*not a project member/);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: FAIL — `Cannot find module '../src/api-client.js'`.

- [ ] **Step 3: Implement the client**

Create `packages/mcp-server/src/api-client.ts`:

```ts
export const DEFAULT_API_BASE_URL = "http://127.0.0.1:4001";

export interface KnowledgeApiClientOptions {
  baseUrl?: string;
  token?: string;
  fetchImpl?: typeof fetch;
}

export interface KnowledgeProject {
  id: string;
  name: string;
  rootPath: string;
}

export interface KnowledgeFileNode {
  name: string;
  path: string;
  isDir: boolean;
  children?: KnowledgeFileNode[];
}

export interface KnowledgeFilesResponse {
  files: KnowledgeFileNode[];
  truncated?: boolean;
}

export interface KnowledgeSearchImage {
  url: string;
  alt: string;
}

export interface KnowledgeSearchResult {
  path: string;
  title: string;
  snippet: string;
  score: number;
  titleMatch?: boolean;
  images?: KnowledgeSearchImage[];
  vectorScore?: number | null;
}

export interface KnowledgeSearchResponse {
  results: KnowledgeSearchResult[];
  mode?: string;
  tokenHits?: number;
  vectorHits?: number;
}

export interface KnowledgeGraphNode {
  id: string;
  label: string;
  type: string;
  path?: string;
  linkCount?: number;
  weight?: number;
}

export interface KnowledgeGraphEdge {
  source: string;
  target: string;
  weight?: number;
}

export type KnowledgeReviewStatus = "unresolved" | "resolved" | "all";

export interface KnowledgeReviewOption {
  label: string;
  action: string;
}

export interface KnowledgeReviewItem {
  id: string;
  type: string;
  title: string;
  description?: string;
  sourcePath?: string;
  affectedPages?: string[];
  searchQueries?: string[];
  options: KnowledgeReviewOption[];
  status: string;
}

export interface KnowledgeReviewsResponse {
  reviews: KnowledgeReviewItem[];
}

export interface KnowledgeHealth {
  ok?: boolean;
  service?: string;
  [key: string]: unknown;
}

export function normalizeBaseUrl(value?: string): string {
  const raw = (value ?? DEFAULT_API_BASE_URL).trim() || DEFAULT_API_BASE_URL;
  return raw.replace(/\/+$/, "");
}

function apiPath(path: string): string {
  return path.startsWith("/api/") ? path : `/api${path.startsWith("/") ? path : `/${path}`}`;
}

function requireObject(value: unknown, context: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${context}: expected JSON object`);
  }
  return value as Record<string, unknown>;
}

function numberOrUndefined(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

export class KnowledgeApiClient {
  private readonly baseUrl: string;
  private readonly token?: string;
  private readonly fetchImpl: typeof fetch;

  constructor(options: KnowledgeApiClientOptions = {}) {
    this.baseUrl = normalizeBaseUrl(options.baseUrl ?? process.env.KNOWLEDGE_API_BASE_URL);
    this.token = options.token ?? process.env.KNOWLEDGE_API_TOKEN;
    this.fetchImpl = options.fetchImpl ?? fetch;
  }

  async health(): Promise<KnowledgeHealth> {
    return this.request("/health", { auth: false }) as Promise<KnowledgeHealth>;
  }

  async projects(): Promise<{ projects: KnowledgeProject[] }> {
    const json = await this.request("/projects");
    const projects = Array.isArray(json.projects) ? json.projects.map(parseProject) : [];
    return { projects };
  }

  async files(
    projectId: string,
    options: { root?: "wiki" | "sources" | "all"; recursive?: boolean; maxFiles?: number } = {},
  ): Promise<KnowledgeFilesResponse> {
    const params = new URLSearchParams();
    params.set("root", options.root ?? "wiki");
    if (options.recursive !== undefined) params.set("recursive", String(options.recursive));
    if (options.maxFiles !== undefined) params.set("maxFiles", String(options.maxFiles));
    const json = await this.request(
      `/projects/${encodeURIComponent(projectId)}/files?${params.toString()}`,
    );
    return {
      files: Array.isArray(json.files) ? json.files.map(parseFileNode) : [],
      truncated: json.truncated === true,
    };
  }

  async fileContent(projectId: string, path: string): Promise<{ path: string; content: string }> {
    const params = new URLSearchParams({ path });
    const json = await this.request(
      `/projects/${encodeURIComponent(projectId)}/files/content?${params.toString()}`,
    );
    return {
      path: typeof json.path === "string" ? json.path : path,
      content: typeof json.content === "string" ? json.content : "",
    };
  }

  async reviews(
    projectId: string,
    options: { status?: KnowledgeReviewStatus; type?: string; limit?: number } = {},
  ): Promise<KnowledgeReviewsResponse> {
    const params = new URLSearchParams();
    if (options.status) params.set("status", options.status);
    if (options.type) params.set("type", options.type);
    if (options.limit !== undefined) params.set("limit", String(options.limit));
    const suffix = params.toString() ? `?${params.toString()}` : "";
    const json = await this.request(`/projects/${encodeURIComponent(projectId)}/reviews${suffix}`);
    const list = Array.isArray(json.items)
      ? json.items
      : Array.isArray(json.reviews)
        ? json.reviews
        : [];
    return { reviews: list.map(parseReviewItem) };
  }

  async search(
    projectId: string,
    query: string,
    options: { topK?: number; includeContent?: boolean } = {},
  ): Promise<KnowledgeSearchResponse> {
    const body: Record<string, unknown> = { query };
    if (options.topK !== undefined) body.topK = options.topK;
    if (options.includeContent !== undefined) body.includeContent = options.includeContent;
    const json = await this.request(`/projects/${encodeURIComponent(projectId)}/search`, {
      method: "POST",
      body,
    });
    return {
      results: Array.isArray(json.results) ? json.results.map(parseSearchResult) : [],
      mode: typeof json.mode === "string" ? json.mode : undefined,
      tokenHits: numberOrUndefined(json.tokenHits),
      vectorHits: numberOrUndefined(json.vectorHits),
    };
  }

  async graph(
    projectId: string,
    options: { q?: string; nodeType?: string; limit?: number } = {},
  ): Promise<{ nodes: KnowledgeGraphNode[]; edges: KnowledgeGraphEdge[] }> {
    const params = new URLSearchParams();
    if (options.q) params.set("q", options.q);
    if (options.nodeType) params.set("nodeType", options.nodeType);
    if (options.limit !== undefined) params.set("limit", String(options.limit));
    const suffix = params.toString() ? `?${params.toString()}` : "";
    const json = await this.request(`/projects/${encodeURIComponent(projectId)}/graph${suffix}`);
    return {
      nodes: Array.isArray(json.nodes) ? json.nodes.map(parseGraphNode) : [],
      edges: Array.isArray(json.edges) ? json.edges.map(parseGraphEdge) : [],
    };
  }

  async rescan(projectId: string): Promise<Record<string, unknown>> {
    return this.request(`/projects/${encodeURIComponent(projectId)}/sources:rescan`, {
      method: "POST",
    });
  }

  private async request(
    path: string,
    options: { method?: "GET" | "POST"; body?: unknown; auth?: boolean } = {},
  ): Promise<Record<string, unknown>> {
    const url = `${this.baseUrl}${apiPath(path)}`;
    const headers: Record<string, string> = { Accept: "application/json" };
    if (options.auth !== false && this.token?.trim()) {
      headers.Authorization = `Bearer ${this.token.trim()}`;
    }
    if (options.body !== undefined) headers["Content-Type"] = "application/json";

    let response: Response;
    try {
      response = await this.fetchImpl(url, {
        method: options.method ?? (options.body === undefined ? "GET" : "POST"),
        headers,
        body: options.body === undefined ? undefined : JSON.stringify(options.body),
      });
    } catch (err) {
      throw new Error(
        `knowledge-server request failed. Is it running at ${this.baseUrl}? ${err instanceof Error ? err.message : String(err)}`,
      );
    }

    const text = await response.text();
    let json: Record<string, unknown>;
    try {
      json = text ? requireObject(JSON.parse(text), "knowledge-server response") : {};
    } catch (err) {
      throw new Error(
        `knowledge-server returned non-JSON (${response.status}): ${text.slice(0, 300)}${err instanceof Error ? ` (${err.message})` : ""}`,
      );
    }

    if (!response.ok) {
      const message = typeof json.error === "string" ? json.error : response.statusText;
      throw new Error(`knowledge-server ${response.status}: ${message}`);
    }
    return json;
  }
}

function parseProject(value: unknown): KnowledgeProject {
  const obj = requireObject(value, "project");
  return {
    id: String(obj.id ?? ""),
    name: String(obj.name ?? ""),
    rootPath: String(obj.rootPath ?? obj.path ?? ""),
  };
}

function parseFileNode(value: unknown): KnowledgeFileNode {
  const obj = requireObject(value, "file node");
  const children = Array.isArray(obj.children) ? obj.children.map(parseFileNode) : undefined;
  return {
    name: String(obj.name ?? ""),
    path: String(obj.path ?? ""),
    isDir: obj.isDir === true || obj.is_dir === true,
    ...(children ? { children } : {}),
  };
}

function parseSearchResult(value: unknown): KnowledgeSearchResult {
  const obj = requireObject(value, "search result");
  return {
    path: String(obj.path ?? ""),
    title: String(obj.title ?? ""),
    snippet: String(obj.snippet ?? ""),
    score: numberOrUndefined(obj.score) ?? 0,
    titleMatch: obj.titleMatch === true,
    images: Array.isArray(obj.images)
      ? obj.images.map((image) => {
          const item = requireObject(image, "image");
          return { url: String(item.url ?? ""), alt: String(item.alt ?? "") };
        })
      : [],
    vectorScore: numberOrUndefined(obj.vectorScore) ?? null,
  };
}

function parseGraphNode(value: unknown): KnowledgeGraphNode {
  const obj = requireObject(value, "graph node");
  return {
    id: String(obj.id ?? ""),
    label: String(obj.label ?? ""),
    type: String(obj.type ?? obj.nodeType ?? "other"),
    path: typeof obj.path === "string" ? obj.path : undefined,
    linkCount: numberOrUndefined(obj.linkCount),
    weight: numberOrUndefined(obj.weight),
  };
}

function parseGraphEdge(value: unknown): KnowledgeGraphEdge {
  const obj = requireObject(value, "graph edge");
  return {
    source: String(obj.source ?? ""),
    target: String(obj.target ?? ""),
    weight: numberOrUndefined(obj.weight),
  };
}

function stringArray(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  return value.map((item) => String(item));
}

function parseReviewItem(value: unknown): KnowledgeReviewItem {
  const obj = requireObject(value, "review item");
  return {
    id: String(obj.id ?? ""),
    type: String(obj.type ?? ""),
    title: String(obj.title ?? ""),
    description: typeof obj.description === "string" ? obj.description : undefined,
    sourcePath: typeof obj.sourcePath === "string" ? obj.sourcePath : undefined,
    affectedPages: stringArray(obj.affectedPages),
    searchQueries: stringArray(obj.searchQueries),
    options: Array.isArray(obj.options)
      ? obj.options.map((option) => {
          const item = requireObject(option, "review option");
          return { label: String(item.label ?? ""), action: String(item.action ?? "") };
        })
      : [],
    status: String(obj.status ?? ""),
  };
}
```

Note the two adjustments from upstream:
- `apiPath` uses `/api/` prefix.
- `rescan` posts to `:rescan` suffix.
- `reviews` accepts both `items` (our shape) and `reviews` (upstream-compatible) array names. Check our server's `list_reviews_handler` JSON shape and use the matching key; the wrapper above handles either to be safe.

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: 6 tests PASS.

- [ ] **Step 5: Typecheck**

Run: `npm run typecheck --workspace @knowledge/mcp-server`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add packages/mcp-server/src/api-client.ts packages/mcp-server/test/api-client.test.ts
git commit -m "feat: add KnowledgeApiClient for mcp-server"
```

---

### Task 3: Formatters

**Files:**
- Create: `packages/mcp-server/src/formatters.ts`
- Create: `packages/mcp-server/test/formatters.test.ts`

Upstream reference: `upstream_llm_wiki/mcp-server/src/index.ts:269-366` (`formatFileTree`, `formatSearchResults`, `formatReviews`, `formatGraph`, `truncateText`). These are pure text — port verbatim, then drop the trailing emoji folder icons if we want the output paste-safe in environments without emoji fonts (upstream uses 📁 / 📄; keeping them is fine — agents render fine).

- [ ] **Step 1: Write the failing tests**

Create `packages/mcp-server/test/formatters.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import {
  formatFileTree,
  formatGraph,
  formatReviews,
  formatSearchResults,
  truncateText,
} from "../src/formatters.js";

describe("formatFileTree", () => {
  it("returns a placeholder when empty", () => {
    expect(formatFileTree([], false)).toBe("No files found.");
  });

  it("walks nested directories with indentation", () => {
    const tree = [
      {
        name: "wiki",
        path: "wiki",
        isDir: true,
        children: [{ name: "index.md", path: "wiki/index.md", isDir: false }],
      },
    ];
    const out = formatFileTree(tree, false);
    expect(out).toContain("📁 wiki");
    expect(out).toContain("  📄 wiki/index.md");
  });

  it("prepends a truncation warning when the API truncated", () => {
    const out = formatFileTree([{ name: "a.md", path: "a.md", isDir: false }], true);
    expect(out).toMatch(/\[warning\] File tree was truncated/);
  });
});

describe("formatSearchResults", () => {
  it("returns a no-results placeholder", () => {
    expect(formatSearchResults("foo", { results: [] })).toBe('No results for "foo".');
  });

  it("renders meta and entries", () => {
    const out = formatSearchResults("q", {
      results: [{ path: "wiki/a.md", title: "A", snippet: "hit", score: 0.5, vectorScore: 0.9 }],
      mode: "hybrid",
      tokenHits: 2,
      vectorHits: 1,
    });
    expect(out).toContain('# Search results for "q"');
    expect(out).toContain("Mode: hybrid");
    expect(out).toContain("## 1. A");
    expect(out).toContain("Snippet: hit");
  });
});

describe("formatReviews", () => {
  it("returns a placeholder when empty", () => {
    expect(formatReviews({ reviews: [] })).toBe("No review items found.");
  });

  it("renders review entries with options", () => {
    const out = formatReviews({
      reviews: [
        {
          id: "r1",
          type: "missing-page",
          title: "Missing Topic",
          description: "We should add this.",
          status: "unresolved",
          options: [{ label: "Confirm", action: "confirm" }],
        },
      ],
    });
    expect(out).toContain("## 1. Missing Topic");
    expect(out).toContain("Type: missing-page");
    expect(out).toContain("Options: Confirm (confirm)");
  });
});

describe("formatGraph", () => {
  it("counts node types and lists top nodes", () => {
    const out = formatGraph(
      [
        { id: "a", label: "A", type: "concept", linkCount: 3 },
        { id: "b", label: "B", type: "entity", linkCount: 1 },
      ],
      [{ source: "a", target: "b" }],
    );
    expect(out).toContain("Nodes: 2");
    expect(out).toContain("Edges: 1");
    expect(out).toContain("- concept: 1");
    expect(out).toContain("A (concept, 3 links)");
  });
});

describe("truncateText", () => {
  it("returns input unchanged under the limit", () => {
    expect(truncateText("short", 100)).toBe("short");
  });

  it("appends a truncation note when too large", () => {
    const long = "x".repeat(200);
    const out = truncateText(long, 50);
    expect(out.startsWith("x".repeat(50))).toBe(true);
    expect(out).toMatch(/\[truncated: \d+ bytes omitted\]/);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: FAIL — `Cannot find module '../src/formatters.js'`.

- [ ] **Step 3: Implement the formatters**

Create `packages/mcp-server/src/formatters.ts`:

```ts
import type {
  KnowledgeFileNode,
  KnowledgeGraphEdge,
  KnowledgeGraphNode,
  KnowledgeReviewItem,
  KnowledgeReviewsResponse,
  KnowledgeSearchResponse,
  KnowledgeSearchResult,
} from "./api-client.js";

export function truncateText(value: string, maxBytes: number): string {
  const bytes = Buffer.byteLength(value, "utf8");
  if (bytes <= maxBytes) return value;
  let out = "";
  let used = 0;
  for (const ch of value) {
    const size = Buffer.byteLength(ch, "utf8");
    if (used + size > maxBytes) break;
    out += ch;
    used += size;
  }
  return `${out}\n\n[truncated: ${bytes - used} bytes omitted]`;
}

export function formatFileTree(files: KnowledgeFileNode[], truncated = false): string {
  if (files.length === 0) return "No files found.";
  const lines: string[] = truncated
    ? ["[warning] File tree was truncated by the knowledge-server maxFiles limit.", ""]
    : [];
  const walk = (nodes: KnowledgeFileNode[], depth: number) => {
    for (const node of nodes) {
      const prefix = "  ".repeat(depth);
      lines.push(`${prefix}${node.isDir ? "📁" : "📄"} ${node.path}`);
      if (node.children) walk(node.children, depth + 1);
    }
  };
  walk(files, 0);
  return lines.join("\n");
}

export function formatSearchResults(
  query: string,
  search: { results: KnowledgeSearchResult[]; mode?: string; tokenHits?: number; vectorHits?: number },
): string {
  const { results } = search;
  if (results.length === 0) return `No results for "${query}".`;
  const meta = [
    search.mode ? `Mode: ${search.mode}` : null,
    typeof search.tokenHits === "number" ? `Token hits: ${search.tokenHits}` : null,
    typeof search.vectorHits === "number" ? `Vector hits: ${search.vectorHits}` : null,
  ].filter(Boolean) as string[];
  const lines: string[] = [
    `# Search results for "${query}"`,
    ...(meta.length > 0 ? [meta.join(" | ")] : []),
    "",
  ];
  results.forEach((result, index) => {
    lines.push(`## ${index + 1}. ${result.title}`);
    lines.push(`Path: ${result.path}`);
    lines.push(
      `Score: ${result.score.toFixed(6)}${typeof result.vectorScore === "number" ? ` | Vector score: ${result.vectorScore.toFixed(6)}` : ""}`,
    );
    if (result.snippet) lines.push(`Snippet: ${result.snippet}`);
    if (result.images && result.images.length > 0) {
      lines.push(`Images: ${result.images.map((image) => image.url).join(", ")}`);
    }
    lines.push("");
  });
  return lines.join("\n");
}

export function formatReviews(response: KnowledgeReviewsResponse): string {
  const { reviews } = response;
  if (reviews.length === 0) return "No review items found.";
  const lines: string[] = ["# Review items", "", `Count: ${reviews.length}`, ""];
  reviews.forEach((review, index) => {
    lines.push(`## ${index + 1}. ${review.title || review.id}`);
    lines.push(`ID: ${review.id}`);
    lines.push(`Type: ${review.type}`);
    lines.push(`Status: ${review.status}`);
    if (review.sourcePath) lines.push(`Source: ${review.sourcePath}`);
    if (review.affectedPages && review.affectedPages.length > 0) {
      lines.push(`Affected pages: ${review.affectedPages.join(", ")}`);
    }
    if (review.searchQueries && review.searchQueries.length > 0) {
      lines.push(`Search queries: ${review.searchQueries.join(", ")}`);
    }
    if (review.description) lines.push(`Description: ${review.description}`);
    const optionSummary = formatReviewOptions(review);
    if (optionSummary) lines.push(`Options: ${optionSummary}`);
    lines.push("");
  });
  return lines.join("\n");
}

function formatReviewOptions(review: KnowledgeReviewItem): string {
  if (!review.options || review.options.length === 0) return "";
  return review.options
    .map((option) => (option.label ? `${option.label} (${option.action})` : option.action))
    .join(", ");
}

export function formatGraph(
  nodes: KnowledgeGraphNode[],
  edges: KnowledgeGraphEdge[],
): string {
  const typeCounts = new Map<string, number>();
  for (const node of nodes) typeCounts.set(node.type, (typeCounts.get(node.type) ?? 0) + 1);
  const lines: string[] = [
    "# Knowledge graph",
    "",
    `Nodes: ${nodes.length}`,
    `Edges: ${edges.length}`,
    "",
    "## Node types",
    ...[...typeCounts.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([type, count]) => `- ${type}: ${count}`),
    "",
    "## Top nodes",
    ...nodes
      .slice()
      .sort((a, b) => (b.linkCount ?? 0) - (a.linkCount ?? 0))
      .slice(0, 30)
      .map(
        (node) =>
          `- ${node.label} (${node.type}, ${node.linkCount ?? 0} links)${node.path ? ` — ${node.path}` : ""}`,
      ),
  ];
  return lines.join("\n");
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: all unit tests PASS (api-client + formatters).

- [ ] **Step 5: Commit**

```bash
git add packages/mcp-server/src/formatters.ts packages/mcp-server/test/formatters.test.ts
git commit -m "feat: add mcp-server text formatters"
```

---

### Task 4: `/api/health` JSON body + integration test

**Files:**
- Modify: `crates/knowledge-server/src/http/router.rs`
- Create: `tests/rust-integration/tests/health_api.rs`

Today the health endpoint is `async fn health() -> StatusCode { StatusCode::OK }`. The MCP `status` tool wants `{ok, service}` so the agent has something to display. Adding a JSON body doesn't break any existing caller because every existing client either ignores the body or already accepts JSON.

- [ ] **Step 1: Write the failing test**

Create `tests/rust-integration/tests/health_api.rs`:

```rust
mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use knowledge_server::config::AppConfig;
use knowledge_server::{bootstrap_state, build_app};
use serde_json::Value;
use support::TestEnvironment;
use tower::util::ServiceExt;

#[tokio::test]
async fn health_returns_json_status_body() {
    let _env = TestEnvironment::start("health-status").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();

    let response = build_app(state)
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body.get("ok").and_then(Value::as_bool), Some(true));
    assert_eq!(
        body.get("service").and_then(Value::as_str),
        Some("knowledge-server")
    );
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p rust-integration --test health_api -- --test-threads=1`
Expected: FAIL — `expected value at line 1 column 1` (empty body cannot deserialize).

- [ ] **Step 3: Replace the health handler**

In `crates/knowledge-server/src/http/router.rs`, replace the `health` function and adjust the import:

```rust
use axum::Json;
use axum::routing::get;
use axum::Router;
use serde_json::json;

use crate::auth;
use crate::app::state::AppState;
use crate::chat;
use crate::projects;
use crate::settings;
use crate::users;

pub fn build_router(state: AppState) -> Router {
  Router::new()
    .route("/api/health", get(health))
    .merge(auth::routes::router())
    .merge(chat::routes::router())
    .merge(projects::routes::router())
    .merge(settings::routes::router())
    .merge(users::routes::router())
    .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
  Json(json!({ "ok": true, "service": "knowledge-server" }))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rust-integration --test health_api -- --test-threads=1`
Expected: PASS.

Also re-run the full integration suite to confirm no caller was relying on an empty body:

Run: `cargo test -p rust-integration -- --test-threads=1`
Expected: all PASS.

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/knowledge-server/src/http/router.rs tests/rust-integration/tests/health_api.rs
git commit -m "feat: return JSON status body from /api/health"
```

---

### Task 5: MCP server entry point with 8 tools

**Files:**
- Create: `packages/mcp-server/src/index.ts`

Upstream reference: `upstream_llm_wiki/mcp-server/src/index.ts` — port the `ListToolsRequestSchema` and `CallToolRequestSchema` handlers, the dispatch switch, and the helper functions (`asObject`, `stringArg`, `optionalStringArg`, `boolArg`, `numberArg`, `enumArg`, `textResult`). Skip `assertMcpEnabled()` entirely. Rename `llm_wiki_*` → `knowledge_*`. Make `project_id` **required** on every project-scoped tool.

- [ ] **Step 1: Create the entry point**

Create `packages/mcp-server/src/index.ts`:

```ts
#!/usr/bin/env node
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ErrorCode,
  ListToolsRequestSchema,
  McpError,
} from "@modelcontextprotocol/sdk/types.js";

import { KnowledgeApiClient } from "./api-client.js";
import {
  formatFileTree,
  formatGraph,
  formatReviews,
  formatSearchResults,
  truncateText,
} from "./formatters.js";

const VERSION = "0.1.0";
const MAX_TEXT_BYTES = 120_000;

const client = new KnowledgeApiClient();

const server = new Server(
  { name: "knowledge-server", version: VERSION },
  { capabilities: { tools: {} } },
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: "knowledge_status",
      description:
        "Check whether knowledge-server is reachable and whether the configured Bearer token can list projects.",
      inputSchema: { type: "object", properties: {}, additionalProperties: false },
    },
    {
      name: "knowledge_projects",
      description: "List projects visible to the configured Bearer token.",
      inputSchema: { type: "object", properties: {}, additionalProperties: false },
    },
    {
      name: "knowledge_files",
      description:
        "List files in a project. project_id is required (the multi-tenant server has no notion of a current project).",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
          root: {
            type: "string",
            enum: ["wiki", "sources", "all"],
            description: "Tree root to list. Defaults to wiki.",
          },
          recursive: { type: "boolean", description: "Whether to list recursively. Defaults to true." },
          max_files: { type: "number", description: "Maximum files returned." },
        },
        required: ["project_id"],
        additionalProperties: false,
      },
    },
    {
      name: "knowledge_read_file",
      description:
        "Read a text file from a project. Only public project paths such as wiki/ and raw/sources/ are allowed.",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
          path: { type: "string", description: "Project-relative file path, for example wiki/index.md." },
        },
        required: ["project_id", "path"],
        additionalProperties: false,
      },
    },
    {
      name: "knowledge_reviews",
      description:
        "List Review tab items from a project. Defaults to unresolved items so agents can help manage pending review work.",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
          status: {
            type: "string",
            enum: ["unresolved", "resolved", "all"],
            description: "Review status filter. Defaults to unresolved.",
          },
          type: { type: "string", description: "Optional review item type filter." },
          limit: { type: "number", description: "Maximum review items returned." },
        },
        required: ["project_id"],
        additionalProperties: false,
      },
    },
    {
      name: "knowledge_search",
      description: "Search a project using the same hybrid keyword + vector retrieval used by the UI.",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
          query: { type: "string", description: "Search query." },
          top_k: { type: "number", description: "Maximum results." },
          include_content: { type: "boolean", description: "Include full page content in results." },
        },
        required: ["project_id", "query"],
        additionalProperties: false,
      },
    },
    {
      name: "knowledge_graph",
      description: "Query the project knowledge graph.",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
          q: { type: "string", description: "Optional text filter." },
          node_type: { type: "string", description: "Optional node type filter." },
          limit: { type: "number", description: "Maximum nodes." },
        },
        required: ["project_id"],
        additionalProperties: false,
      },
    },
    {
      name: "knowledge_rescan_sources",
      description:
        "Trigger a project source folder rescan, using the project's Source Watch rules.",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
        },
        required: ["project_id"],
        additionalProperties: false,
      },
    },
  ],
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const args = asObject(request.params.arguments ?? {});
  try {
    switch (request.params.name) {
      case "knowledge_status": {
        const health = await client.health();
        let projects: Awaited<ReturnType<typeof client.projects>> | { error: string };
        try {
          projects = await client.projects();
        } catch (err) {
          projects = { error: err instanceof Error ? err.message : String(err) };
        }
        return textResult(JSON.stringify({ ...health, ...projects }, null, 2));
      }
      case "knowledge_projects": {
        return textResult(JSON.stringify(await client.projects(), null, 2));
      }
      case "knowledge_files": {
        const response = await client.files(requireProjectId(args), {
          root: enumArg(args.root, ["wiki", "sources", "all"] as const, "wiki"),
          recursive: boolArg(args.recursive, true),
          maxFiles: numberArg(args.max_files),
        });
        return textResult(formatFileTree(response.files, response.truncated));
      }
      case "knowledge_read_file": {
        const projectId = requireProjectId(args);
        const relPath = stringArg(args.path, "path");
        const { path, content } = await client.fileContent(projectId, relPath);
        return textResult(`# ${path}\n\n${truncateText(content, MAX_TEXT_BYTES)}`);
      }
      case "knowledge_reviews": {
        const reviews = await client.reviews(requireProjectId(args), {
          status: enumArg(args.status, ["unresolved", "resolved", "all"] as const, "unresolved"),
          type: optionalStringArg(args.type),
          limit: numberArg(args.limit),
        });
        return textResult(formatReviews(reviews));
      }
      case "knowledge_search": {
        const projectId = requireProjectId(args);
        const query = stringArg(args.query, "query");
        const search = await client.search(projectId, query, {
          topK: numberArg(args.top_k),
          includeContent: boolArg(args.include_content, false),
        });
        return textResult(formatSearchResults(query, search));
      }
      case "knowledge_graph": {
        const graph = await client.graph(requireProjectId(args), {
          q: optionalStringArg(args.q),
          nodeType: optionalStringArg(args.node_type),
          limit: numberArg(args.limit),
        });
        return textResult(formatGraph(graph.nodes, graph.edges));
      }
      case "knowledge_rescan_sources": {
        return textResult(JSON.stringify(await client.rescan(requireProjectId(args)), null, 2));
      }
      default:
        throw new McpError(ErrorCode.MethodNotFound, `Unknown tool: ${request.params.name}`);
    }
  } catch (err) {
    if (err instanceof McpError) throw err;
    throw new McpError(
      ErrorCode.InternalError,
      err instanceof Error ? err.message : String(err),
    );
  }
});

function textResult(text: string) {
  return { content: [{ type: "text" as const, text }] };
}

function asObject(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  return value as Record<string, unknown>;
}

function requireProjectId(args: Record<string, unknown>): string {
  const value = args.project_id;
  if (typeof value !== "string" || value.trim() === "") {
    throw new McpError(ErrorCode.InvalidParams, "project_id is required");
  }
  return value;
}

function stringArg(value: unknown, name: string): string {
  if (typeof value !== "string" || value.trim() === "") {
    throw new McpError(ErrorCode.InvalidParams, `${name} is required`);
  }
  return value;
}

function optionalStringArg(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() !== "" ? value : undefined;
}

function boolArg(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function numberArg(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function enumArg<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback;
}

async function main(): Promise<void> {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  const base = process.env.KNOWLEDGE_API_BASE_URL ?? "http://127.0.0.1:4001";
  console.error(`knowledge-mcp v${VERSION} connected to ${base}`);
}

main().catch((err) => {
  console.error("Failed to start knowledge-mcp:", err);
  process.exit(1);
});
```

- [ ] **Step 2: Build**

Run: `npm run build --workspace @knowledge/mcp-server`
Expected: `dist/src/index.js` written without errors.

- [ ] **Step 3: Verify the binary is executable on the platform**

```bash
node packages/mcp-server/dist/src/index.js < /dev/null
```

Expected: prints `knowledge-mcp v0.1.0 connected to http://127.0.0.1:4001` to stderr and exits cleanly when stdin EOFs (the SDK detects EOF and shuts down). Note: on Windows, `/dev/null` is `NUL` — bash on Windows accepts `/dev/null` too.

If the executable bit isn't set on Unix, add `chmod +x dist/src/index.js` to the `build` script.

- [ ] **Step 4: Commit**

```bash
git add packages/mcp-server/src/index.ts
git commit -m "feat: add mcp-server stdio entry with 8 knowledge_* tools"
```

---

### Task 6: Stdio integration test

**Files:**
- Create: `packages/mcp-server/test/server.test.ts`

Spawns the built `dist/src/index.js` as a child process, connects via the MCP SDK client over stdio, asserts `tools/list` and one round-trip `tools/call`. We point the child at an ephemeral HTTP server we boot inside the test that mimics the knowledge-server REST shape for one request. This catches dispatch bugs, schema regressions, and Bearer-header wiring without needing a live Postgres.

- [ ] **Step 1: Write the test**

Create `packages/mcp-server/test/server.test.ts`:

```ts
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { fileURLToPath } from "node:url";

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

const here = fileURLToPath(new URL("./", import.meta.url));
const entry = `${here}../dist/src/index.js`;

let httpServer: Server;
let port: number;
const seenAuthHeaders: string[] = [];

beforeAll(async () => {
  httpServer = createServer((req: IncomingMessage, res: ServerResponse) => {
    seenAuthHeaders.push(String(req.headers["authorization"] ?? ""));
    if (req.method === "GET" && req.url === "/api/health") {
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: true, service: "knowledge-server" }));
      return;
    }
    if (req.method === "GET" && req.url === "/api/projects") {
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ projects: [{ id: "p1", name: "Demo", rootPath: "/tmp/demo" }] }));
      return;
    }
    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "not found" }));
  });
  await new Promise<void>((resolve) => {
    httpServer.listen(0, "127.0.0.1", () => resolve());
  });
  const addr = httpServer.address();
  if (!addr || typeof addr === "string") throw new Error("no port");
  port = addr.port;
});

afterAll(async () => {
  await new Promise<void>((resolve) => httpServer.close(() => resolve()));
});

describe("mcp-server stdio", () => {
  it("lists 8 tools and calls knowledge_projects with Bearer auth", async () => {
    const transport = new StdioClientTransport({
      command: process.execPath,
      args: [entry],
      env: {
        ...process.env,
        KNOWLEDGE_API_BASE_URL: `http://127.0.0.1:${port}`,
        KNOWLEDGE_API_TOKEN: "test-token",
      },
    });
    const client = new Client({ name: "test-client", version: "0.0.0" }, { capabilities: {} });
    await client.connect(transport);

    try {
      const list = await client.listTools();
      const names = list.tools.map((tool) => tool.name).sort();
      expect(names).toEqual([
        "knowledge_files",
        "knowledge_graph",
        "knowledge_projects",
        "knowledge_read_file",
        "knowledge_rescan_sources",
        "knowledge_reviews",
        "knowledge_search",
        "knowledge_status",
      ]);

      const result = await client.callTool({ name: "knowledge_projects", arguments: {} });
      const text =
        Array.isArray(result.content) && result.content[0]?.type === "text"
          ? String(result.content[0].text)
          : "";
      expect(text).toContain("Demo");
      expect(seenAuthHeaders).toContain("Bearer test-token");
    } finally {
      await client.close();
    }
  }, 15_000);

  it("rejects knowledge_files without project_id", async () => {
    const transport = new StdioClientTransport({
      command: process.execPath,
      args: [entry],
      env: {
        ...process.env,
        KNOWLEDGE_API_BASE_URL: `http://127.0.0.1:${port}`,
        KNOWLEDGE_API_TOKEN: "test-token",
      },
    });
    const client = new Client({ name: "test-client", version: "0.0.0" }, { capabilities: {} });
    await client.connect(transport);
    try {
      await expect(
        client.callTool({ name: "knowledge_files", arguments: {} }),
      ).rejects.toThrow(/project_id is required/);
    } finally {
      await client.close();
    }
  }, 15_000);
});
```

(`StdioClientTransport` spawns the subprocess for us — no need to use `node:child_process` directly.)

- [ ] **Step 2: Make sure the built bundle exists before vitest runs**

Add a `pretest` hook to `packages/mcp-server/package.json`:

```json
"scripts": {
  "build": "tsc -p tsconfig.json",
  "typecheck": "tsc -p tsconfig.json --noEmit",
  "pretest": "npm run build",
  "test": "vitest run"
}
```

- [ ] **Step 3: Run**

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: PASS (api-client unit tests + formatters tests + 2 stdio integration tests).

- [ ] **Step 4: Commit**

```bash
git add packages/mcp-server/test/server.test.ts packages/mcp-server/package.json
git commit -m "test: add mcp-server stdio integration test"
```

---

### Task 7: README + Claude Code / Codex CLI snippets

**Files:**
- Create: `packages/mcp-server/README.md`

This is documentation, not code. The MCP ecosystem has settled on a couple of config formats — including both makes the package immediately usable.

- [ ] **Step 1: Write the README**

Create `packages/mcp-server/README.md`:

```markdown
# @knowledge/mcp-server

Standalone Model Context Protocol server that exposes a knowledge-server project
to MCP-compatible agent clients (Claude Code, Codex CLI, Cursor, etc.) using a
per-user API token minted from the admin UI.

## Tools

| Tool | Description |
|---|---|
| `knowledge_status` | Verify server reachability and that the token can list projects. |
| `knowledge_projects` | List projects the token can see. |
| `knowledge_files` | List files in a project (wiki / sources / all). |
| `knowledge_read_file` | Read a single text file from a project. |
| `knowledge_reviews` | List Review tab items. |
| `knowledge_search` | Hybrid keyword + vector search. |
| `knowledge_graph` | Knowledge-graph summary. |
| `knowledge_rescan_sources` | Trigger a Source Watch rescan. |

## Installation

From the repo root:

```bash
npm install
npm run build --workspace @knowledge/mcp-server
```

The built binary lands at `packages/mcp-server/dist/src/index.js` and is exposed
as `knowledge-mcp` once you `npm link` (optional).

## Configuration

The server reads two environment variables:

- `KNOWLEDGE_API_BASE_URL` — defaults to `http://127.0.0.1:4001`.
- `KNOWLEDGE_API_TOKEN` — required for any tool other than `knowledge_status`.
  Mint a token in the admin UI: **Admin Console → API Tokens → Mint Token**.

## Claude Code

Add to your Claude Code MCP config (`.mcp.json` or via `/mcp` settings):

```json
{
  "mcpServers": {
    "knowledge": {
      "command": "node",
      "args": ["/absolute/path/to/packages/mcp-server/dist/src/index.js"],
      "env": {
        "KNOWLEDGE_API_BASE_URL": "http://127.0.0.1:4001",
        "KNOWLEDGE_API_TOKEN": "<your-token>"
      }
    }
  }
}
```

## Codex CLI

```toml
[mcp_servers.knowledge]
command = "node"
args = ["/absolute/path/to/packages/mcp-server/dist/src/index.js"]
env = { KNOWLEDGE_API_BASE_URL = "http://127.0.0.1:4001", KNOWLEDGE_API_TOKEN = "<your-token>" }
```
```

- [ ] **Step 2: Commit**

```bash
git add packages/mcp-server/README.md
git commit -m "docs: add mcp-server README with Claude Code and Codex snippets"
```

---

### Task 8: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Rust lint + tests**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo test --workspace -- --test-threads=1`
Expected: PASS, including the new `health_api` integration test.

- [ ] **Step 2: Frontend tests + lint**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS (no changes here, just regression check).

Run: `npm run test --workspace @knowledge/api-client`
Expected: PASS.

Run: `npm run test --workspace @knowledge/mcp-server`
Expected: PASS (unit + stdio integration tests).

Run: `npm run lint`
Expected: tsc + clippy clean. (If `tsc --noEmit` in the admin lint script doesn't cover the mcp-server workspace, also run `npm run typecheck --workspace @knowledge/mcp-server`.)

- [ ] **Step 3: Playwright suite**

Run: `npm run test --workspace @knowledge/web`
Expected: PASS — no specs were changed; this confirms the `/api/health` JSON change didn't break the e2e harness.

- [ ] **Step 4: Final commit (only if fixes were needed)**

```bash
git status
```

If steps 1–3 required fixes, commit them. Otherwise the work is complete.
