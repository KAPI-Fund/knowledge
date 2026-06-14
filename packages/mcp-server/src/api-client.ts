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
