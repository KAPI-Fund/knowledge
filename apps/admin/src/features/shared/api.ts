import { apiFetch } from "@knowledge/api-client";
import { z } from "zod";

const loginSchema = z.object({
  csrfToken: z.string(),
});

const projectsSchema = z.object({
  projects: z.array(
    z.object({
      id: z.string(),
      name: z.string(),
      rootPath: z.string(),
      createdAt: z.string(),
    }),
  ),
});

const usersSchema = z.object({
  users: z.array(
    z.object({
      id: z.string(),
      username: z.string(),
      role: z.string(),
    }),
  ),
});

const projectDetailSchema = z.object({
  project: z.object({
    id: z.string(),
    name: z.string(),
    rootPath: z.string(),
    createdAt: z.string(),
    sourceCount: z.number(),
    taskCount: z.number(),
    reviewCount: z.number(),
  }),
});

const tasksSchema = z.object({
  tasks: z.array(
    z.object({
      id: z.string(),
      projectId: z.string().optional(),
      taskType: z.string(),
      status: z.string(),
      title: z.string(),
      relativePath: z.string().nullable().optional(),
      detail: z.unknown().optional(),
      payload: z.unknown().optional(),
      result: z.unknown().nullable().optional(),
      error: z.unknown().nullable().optional(),
      attemptCount: z.number().optional(),
      maxAttempts: z.number().optional(),
      createdAt: z.string().optional(),
      updatedAt: z.string().optional(),
      startedAt: z.string().nullable().optional(),
      finishedAt: z.string().nullable().optional(),
    }),
  ),
});

const taskSchema = z.object({
  id: z.string(),
  projectId: z.string().optional(),
  taskType: z.string(),
  status: z.string(),
  title: z.string(),
  relativePath: z.string().nullable().optional(),
  detail: z.unknown(),
  payload: z.unknown().optional(),
  result: z.unknown().nullable().optional(),
  error: z.unknown().nullable().optional(),
  attemptCount: z.number().optional(),
  maxAttempts: z.number().optional(),
  createdAt: z.string(),
  updatedAt: z.string(),
  startedAt: z.string().nullable().optional(),
  finishedAt: z.string().nullable().optional(),
});

const sourcesSchema = z.object({
  sources: z.array(
    z.object({
      relativePath: z.string(),
      size: z.number(),
    }),
  ),
});

const sourceWatchSettingsSchema = z.object({
  enabled: z.boolean(),
  autoIngest: z.boolean(),
  path: z.string(),
  includeExtensions: z.array(z.string()),
  excludeExtensions: z.array(z.string()),
  excludeDirs: z.array(z.string()),
  excludeGlobs: z.array(z.string()),
  maxFileSizeMb: z.number(),
  intervalMinutes: z.number(),
  lastScanAt: z.string().nullable().optional(),
});

const sourceWatchScanResultSchema = z.object({
  watchedCount: z.number(),
  copiedCount: z.number(),
  deletedCount: z.number(),
  queuedIngestCount: z.number(),
  queuedDeleteCount: z.number(),
});

const reviewsSchema = z.object({
  reviews: z.array(
    z.object({
      id: z.string(),
      status: z.string(),
      type: z.string().optional(),
      title: z.string(),
      description: z.string().optional(),
      sourcePath: z.string().optional(),
      affectedPages: z.array(z.string()).optional(),
      searchQueries: z.array(z.string()).optional(),
      options: z
        .array(
          z.object({
            label: z.string(),
            action: z.string(),
          }),
        )
        .optional(),
    }),
  ),
});

const projectFileNodeSchema: z.ZodType<any> = z.lazy(() =>
  z.object({
    name: z.string(),
    path: z.string(),
    isDir: z.boolean(),
    size: z.number().nullable().optional(),
    children: z.array(projectFileNodeSchema).nullable().optional(),
  }),
);

const projectFilesSchema = z.object({
  root: z.string(),
  files: z.array(projectFileNodeSchema),
  truncated: z.boolean(),
});

const projectFileContentSchema = z.object({
  path: z.string(),
  content: z.string(),
});

const saveFileContentSchema = z.object({
  path: z.string(),
  created: z.boolean(),
});

const deleteWikiPagesSchema = z.object({
  deletedPaths: z.array(z.string()),
  rewrittenFiles: z.number(),
});

const auditSchema = z.object({
  items: z.array(
    z.object({
      id: z.string(),
      action: z.string(),
      summary: z.string(),
      targetType: z.string().optional(),
      targetId: z.string().optional(),
    }),
  ),
});

const settingsSchema = z.object({
  providerMode: z.string(),
  language: z.string(),
  defaultQueryLimit: z.number(),
  providerBaseUrl: z.string().nullable().optional(),
  providerApiKeyConfigured: z.boolean().optional(),
  providerModel: z.string().nullable().optional(),
  providerEmbeddingModel: z.string().nullable().optional(),
  providerTimeoutSeconds: z.number().nullable().optional(),
  searchProvider: z.string().nullable().optional(),
  searchApiKeyConfigured: z.boolean().optional(),
  serpapiEngine: z.string().nullable().optional(),
  searxngUrl: z.string().nullable().optional(),
  searxngCategories: z.array(z.string()).nullable().optional(),
  ollamaSearchUrl: z.string().nullable().optional(),
});

const webSearchResultSchema = z.object({
  title: z.string(),
  url: z.string(),
  snippet: z.string(),
  source: z.string(),
});

const webSearchResponseSchema = z.object({
  results: z.array(webSearchResultSchema),
});

export type WebSearchResult = z.infer<typeof webSearchResultSchema>;

const searchResultsSchema = z.object({
  mode: z.string(),
  tokenHits: z.number(),
  vectorHits: z.number(),
  results: z.array(
    z.object({
      path: z.string(),
      title: z.string(),
      snippet: z.string(),
      score: z.number(),
      titleMatch: z.boolean().optional(),
      images: z
        .array(
          z.object({
            url: z.string(),
            alt: z.string(),
          }),
        )
        .optional(),
      content: z.string().optional(),
    }),
  ),
});

const graphSchema = z.object({
  nodes: z.array(
    z.object({
      id: z.string(),
      label: z.string(),
      nodeType: z.string(),
      path: z.string(),
      linkCount: z.number(),
    }),
  ),
  edges: z.array(
    z.object({
      source: z.string(),
      target: z.string(),
      weight: z.number(),
    }),
  ),
});

const graphNeighborsSchema = z.object({
  node: z.object({
    id: z.string(),
    label: z.string(),
    nodeType: z.string(),
    path: z.string(),
    linkCount: z.number(),
  }),
  neighbors: z.array(
    z.object({
      id: z.string(),
      label: z.string(),
      nodeType: z.string(),
      path: z.string(),
      linkCount: z.number(),
    }),
  ),
});

function csrfHeader() {
  return {
    "x-csrf-token": window.sessionStorage.getItem("knowledge.csrfToken") ?? "",
  };
}

const projectSchema = z.object({
  id: z.string(),
  name: z.string(),
  rootPath: z.string(),
  createdAt: z.string(),
});

const conversationSchema = z.object({
  id: z.string(),
  projectId: z.string(),
  title: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

const conversationsSchema = z.object({
  conversations: z.array(conversationSchema),
});

const conversationMessagesSchema = z.object({
  messages: z.array(
    z.object({
      id: z.string(),
      role: z.string(),
      content: z.string(),
      contextSummary: z.string().nullable().optional(),
      createdAt: z.string(),
    }),
  ),
});

export async function login(input: { username: string; password: string }) {
  return apiFetch(
    "/api/auth/login",
    {
      method: "POST",
      body: JSON.stringify(input),
    },
    loginSchema,
  );
}

export async function listProjects() {
  const response = await apiFetch("/api/projects", { method: "GET" }, projectsSchema);
  return response.projects;
}

export async function listUsers() {
  const response = await apiFetch("/api/users", { method: "GET" }, usersSchema);
  return response.users;
}

export async function getProjectDetail(projectId: string) {
  return apiFetch(`/api/projects/${projectId}`, { method: "GET" }, projectDetailSchema);
}

export async function listProjectTasks(projectId: string) {
  const response = await apiFetch(`/api/projects/${projectId}/tasks`, { method: "GET" }, tasksSchema);
  return response.tasks;
}

export async function createProject(input: { name: string; csrfToken: string }) {
  return apiFetch(
    "/api/projects",
    {
      method: "POST",
      headers: {
        "x-csrf-token": input.csrfToken,
      },
      body: JSON.stringify({
        name: input.name,
      }),
    },
    projectSchema,
  );
}

export async function listProjectSources(projectId: string) {
  const response = await apiFetch(`/api/projects/${projectId}/sources`, { method: "GET" }, sourcesSchema);
  return response.sources;
}

export async function getProjectSourceWatchSettings(projectId: string) {
  return apiFetch(
    `/api/projects/${projectId}/source-watch`,
    { method: "GET" },
    sourceWatchSettingsSchema,
  );
}

export async function updateProjectSourceWatchSettings(input: {
  projectId: string;
  enabled: boolean;
  autoIngest: boolean;
  path: string;
  includeExtensions: string[];
  excludeExtensions: string[];
  excludeDirs: string[];
  excludeGlobs: string[];
  maxFileSizeMb: number;
  intervalMinutes: number;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/source-watch`,
    {
      method: "PATCH",
      headers: csrfHeader(),
      body: JSON.stringify({
        enabled: input.enabled,
        autoIngest: input.autoIngest,
        path: input.path,
        includeExtensions: input.includeExtensions,
        excludeExtensions: input.excludeExtensions,
        excludeDirs: input.excludeDirs,
        excludeGlobs: input.excludeGlobs,
        maxFileSizeMb: input.maxFileSizeMb,
        intervalMinutes: input.intervalMinutes,
      }),
    },
    sourceWatchSettingsSchema,
  );
}

export async function scanProjectSourceWatch(projectId: string) {
  return apiFetch(
    `/api/projects/${projectId}/source-watch:scan`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    sourceWatchScanResultSchema,
  );
}

export async function listProjectFiles(input: {
  projectId: string;
  root?: string;
  recursive?: boolean;
  maxFiles?: number;
}) {
  const searchParams = new URLSearchParams();
  if (input.root?.trim()) {
    searchParams.set("root", input.root.trim());
  }
  if (input.recursive === false) {
    searchParams.set("recursive", "false");
  }
  if (input.maxFiles && input.maxFiles > 0) {
    searchParams.set("maxFiles", String(input.maxFiles));
  }
  const query = searchParams.toString();
  const path = query
    ? `/api/projects/${input.projectId}/files?${query}`
    : `/api/projects/${input.projectId}/files`;
  return apiFetch(path, { method: "GET" }, projectFilesSchema);
}

export async function getProjectFileContent(input: { projectId: string; path: string }) {
  const searchParams = new URLSearchParams({ path: input.path });
  return apiFetch(
    `/api/projects/${input.projectId}/files/content?${searchParams.toString()}`,
    { method: "GET" },
    projectFileContentSchema,
  );
}

export async function saveProjectFileContent(input: {
  projectId: string;
  path: string;
  content: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/files/content`,
    {
      method: "PUT",
      headers: csrfHeader(),
      body: JSON.stringify({ path: input.path, content: input.content }),
    },
    saveFileContentSchema,
  );
}

export async function listConversations(projectId: string) {
  const response = await apiFetch(
    `/api/projects/${projectId}/conversations`,
    { method: "GET" },
    conversationsSchema,
  );
  return response.conversations;
}

export async function createConversation(input: { projectId: string; title?: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/conversations`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({ title: input.title }),
    },
    conversationSchema,
  );
}

export async function renameConversation(input: {
  projectId: string;
  conversationId: string;
  title: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/conversations/${input.conversationId}`,
    {
      method: "PATCH",
      headers: csrfHeader(),
      body: JSON.stringify({ title: input.title }),
    },
    conversationSchema,
  );
}

export async function deleteConversation(input: { projectId: string; conversationId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/conversations/${input.conversationId}`,
    {
      method: "DELETE",
      headers: csrfHeader(),
    },
    z.object({ deleted: z.boolean() }),
  );
}

export async function listConversationMessages(input: {
  projectId: string;
  conversationId: string;
}) {
  const response = await apiFetch(
    `/api/projects/${input.projectId}/conversations/${input.conversationId}/messages`,
    { method: "GET" },
    conversationMessagesSchema,
  );
  return response.messages;
}

export async function deleteProjectWikiPages(input: { projectId: string; paths: string[] }) {
  return apiFetch(
    `/api/projects/${input.projectId}/wiki-pages:delete`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({ paths: input.paths }),
    },
    deleteWikiPagesSchema,
  );
}

function encodeBytesBase64(bytes: Uint8Array) {
  let binary = "";
  const chunkSize = 0x8000;
  for (let index = 0; index < bytes.length; index += chunkSize) {
    const chunk = bytes.subarray(index, index + chunkSize);
    binary += String.fromCharCode(...chunk);
  }
  return window.btoa(binary);
}

function encodeContentBase64(content: string) {
  const bytes = new TextEncoder().encode(content);
  return encodeBytesBase64(bytes);
}

export async function importProjectSource(input: {
  projectId: string;
  fileName: string;
  content?: string;
  contentBase64?: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/sources:import`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({
        fileName: input.fileName,
        contentBase64: input.contentBase64 ?? encodeContentBase64(input.content ?? ""),
      }),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function fileToBase64(file: File) {
  const buffer = await readFileArrayBuffer(file);
  return encodeBytesBase64(new Uint8Array(buffer));
}

async function readFileArrayBuffer(file: File) {
  if (typeof file.arrayBuffer === "function") {
    return file.arrayBuffer();
  }

  return new Promise<ArrayBuffer>((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener("load", () => {
      if (reader.result instanceof ArrayBuffer) {
        resolve(reader.result);
        return;
      }

      reject(new Error("File reader did not produce binary content."));
    });
    reader.addEventListener("error", () => {
      reject(reader.error ?? new Error("Failed to read file."));
    });
    reader.readAsArrayBuffer(file);
  });
}

export async function ingestProjectSource(input: {
  projectId: string;
  relativePath: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/ingest`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({
        relativePath: input.relativePath,
      }),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function rescanProjectSources(input: { projectId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/sources:rescan`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function deleteProjectSource(input: {
  projectId: string;
  relativePath: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/sources/${input.relativePath}`,
    {
      method: "DELETE",
      headers: csrfHeader(),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function searchProject(input: {
  projectId: string;
  query: string;
  topK?: number;
  includeContent?: boolean;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/search`,
    {
      method: "POST",
      body: JSON.stringify({
        query: input.query,
        topK: input.topK,
        includeContent: input.includeContent,
      }),
    },
    searchResultsSchema,
  );
}

export async function getProjectGraph(input: {
  projectId: string;
  query?: string;
  nodeType?: string;
  limit?: number;
}) {
  const searchParams = new URLSearchParams();
  if (input.query?.trim()) {
    searchParams.set("q", input.query.trim());
  }
  if (input.nodeType?.trim()) {
    searchParams.set("nodeType", input.nodeType.trim());
  }
  if (input.limit && input.limit > 0) {
    searchParams.set("limit", String(input.limit));
  }
  const queryString = searchParams.toString();
  const path = queryString
    ? `/api/projects/${input.projectId}/graph?${queryString}`
    : `/api/projects/${input.projectId}/graph`;
  return apiFetch(path, { method: "GET" }, graphSchema);
}

export async function getProjectGraphNeighbors(input: {
  projectId: string;
  nodeId: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/graph/${encodeURIComponent(input.nodeId)}/neighbors`,
    { method: "GET" },
    graphNeighborsSchema,
  );
}

export async function listProjectReviews(input: {
  projectId: string;
  status?: string;
  itemType?: string;
  limit?: number;
}) {
  const searchParams = new URLSearchParams();
  if (input.status?.trim()) {
    searchParams.set("status", input.status.trim());
  }
  if (input.itemType?.trim()) {
    searchParams.set("type", input.itemType.trim());
  }
  if (input.limit && input.limit > 0) {
    searchParams.set("limit", String(input.limit));
  }
  const query = searchParams.toString();
  const path = query
    ? `/api/projects/${input.projectId}/reviews?${query}`
    : `/api/projects/${input.projectId}/reviews`;
  const response = await apiFetch(path, { method: "GET" }, reviewsSchema);
  return response.reviews;
}

export async function listProjectAuditLogs(projectId: string) {
  const response = await apiFetch(`/api/projects/${projectId}/audit-logs`, { method: "GET" }, auditSchema);
  return response.items;
}

export async function getSystemSettings() {
  return apiFetch("/api/system/settings", { method: "GET" }, settingsSchema);
}

export async function createQueryTask(input: {
  projectId: string;
  query: string;
  topK: number;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/query-tasks`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({
        query: input.query,
        topK: input.topK,
      }),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function createLintTask(input: {
  projectId: string;
  mode: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/lint-tasks`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({
        mode: input.mode,
      }),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function getQueryTaskDetail(input: { projectId: string; taskId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/query-tasks/${input.taskId}`,
    { method: "GET" },
    taskSchema,
  );
}

export async function saveQueryTaskResult(input: {
  projectId: string;
  taskId: string;
  title: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/query-tasks/${input.taskId}/save`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({
        title: input.title,
      }),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function retryProjectTask(input: { projectId: string; taskId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/tasks/${input.taskId}/retry`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    taskSchema,
  );
}

export async function getProjectTaskDetail(input: { projectId: string; taskId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/tasks/${input.taskId}`,
    { method: "GET" },
    taskSchema,
  );
}

export async function cancelProjectTask(input: { projectId: string; taskId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/tasks/${input.taskId}/cancel`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    taskSchema,
  );
}

export async function updateProjectReview(input: {
  projectId: string;
  reviewId: string;
  status: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/reviews/${input.reviewId}`,
    {
      method: "PATCH",
      headers: csrfHeader(),
      body: JSON.stringify({ status: input.status }),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function sweepProjectReviews(input: { projectId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/reviews:sweep`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

const dedupGroupSchema = z.object({
  id: z.string(),
  slugs: z.array(z.string()),
  reason: z.string(),
  confidence: z.string(),
  status: z.string(),
  createdAt: z.string(),
});

const dedupOverviewSchema = z.object({
  groups: z.array(dedupGroupSchema),
  notDuplicates: z.array(z.array(z.string())),
});

export type DedupGroup = z.infer<typeof dedupGroupSchema>;

export async function getProjectDedup(projectId: string) {
  return apiFetch(`/api/projects/${projectId}/dedup`, { method: "GET" }, dedupOverviewSchema);
}

export async function detectProjectDuplicates(input: { projectId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/dedup:detect`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function mergeProjectDuplicateGroup(input: {
  projectId: string;
  groupId: string;
  canonicalSlug: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/dedup/groups/${input.groupId}/merge`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({ canonicalSlug: input.canonicalSlug }),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

export async function dismissProjectDuplicateGroup(input: {
  projectId: string;
  groupId: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/dedup/groups/${input.groupId}/dismiss`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    z.object({ dismissed: z.boolean() }),
  );
}

export async function createDeepResearchTask(input: {
  projectId: string;
  topic: string;
  searchQueries?: string[];
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/deep-research`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({
        topic: input.topic,
        searchQueries: input.searchQueries,
      }),
    },
    z.object({
      taskId: z.string(),
      status: z.string(),
    }),
  );
}

const apiTokenSchema = z.object({
  id: z.string(),
  name: z.string(),
  projectId: z.string().nullable(),
  prefix: z.string(),
  lastUsedAt: z.string().nullable(),
  revokedAt: z.string().nullable(),
  createdAt: z.string(),
});

const apiTokensListSchema = z.object({
  tokens: z.array(apiTokenSchema),
});

const apiTokenCreateResponseSchema = z.object({
  id: z.string(),
  name: z.string(),
  projectId: z.string().nullable(),
  prefix: z.string(),
  createdAt: z.string(),
  token: z.string(),
});

export type ApiTokenSummary = z.infer<typeof apiTokenSchema>;
export type ApiTokenCreateResponse = z.infer<typeof apiTokenCreateResponseSchema>;

export async function listApiTokens() {
  return apiFetch(`/api/users/me/api-tokens`, { method: "GET" }, apiTokensListSchema);
}

export async function createApiToken(input: { name: string; projectId: string | null }) {
  return apiFetch(
    `/api/users/me/api-tokens`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({ name: input.name, projectId: input.projectId }),
    },
    apiTokenCreateResponseSchema,
  );
}

export async function revokeApiToken(input: { tokenId: string }) {
  return apiFetch(
    `/api/users/me/api-tokens/${input.tokenId}/revoke`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    z.object({ revoked: z.boolean() }),
  );
}

export async function updateSystemSettings(input: {
  providerMode: string;
  language: string;
  defaultQueryLimit: number;
  providerBaseUrl?: string;
  providerApiKey?: string;
  providerModel?: string;
  providerEmbeddingModel?: string;
  providerTimeoutSeconds?: number;
  searchProvider?: string;
  searchApiKey?: string;
  serpapiEngine?: string;
  searxngUrl?: string;
  searxngCategories?: string[];
  ollamaSearchUrl?: string;
}) {
  const payload = {
    providerMode: input.providerMode,
    language: input.language,
    defaultQueryLimit: input.defaultQueryLimit,
    providerBaseUrl: input.providerBaseUrl,
    providerModel: input.providerModel,
    providerEmbeddingModel: input.providerEmbeddingModel,
    providerTimeoutSeconds: input.providerTimeoutSeconds,
    searchProvider: input.searchProvider,
    serpapiEngine: input.serpapiEngine,
    searxngUrl: input.searxngUrl,
    searxngCategories: input.searxngCategories,
    ollamaSearchUrl: input.ollamaSearchUrl,
    ...(input.providerApiKey?.trim()
      ? {
          providerApiKey: input.providerApiKey.trim(),
        }
      : {}),
    ...(input.searchApiKey?.trim()
      ? {
          searchApiKey: input.searchApiKey.trim(),
        }
      : {}),
  };

  return apiFetch(
    "/api/system/settings",
    {
      method: "PATCH",
      headers: csrfHeader(),
      body: JSON.stringify(payload),
    },
    settingsSchema,
  );
}

export async function runWebSearch(input: { query: string; maxResults?: number }) {
  return apiFetch(
    `/api/web-search`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({ query: input.query, maxResults: input.maxResults }),
    },
    webSearchResponseSchema,
  );
}
