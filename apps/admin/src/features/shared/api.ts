import { apiFetch } from "@knowledge/api-client";
import { z } from "zod";

import { getCsrfToken } from "../auth/csrf";

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

const fileHistoryEntrySchema = z.object({
  id: z.string(),
  path: z.string(),
  author: z.string(),
  tool: z.string(),
  createdAt: z.string(),
});

const fileHistoryListSchema = z.object({
  entries: z.array(fileHistoryEntrySchema),
});

const fileHistoryDetailSchema = fileHistoryEntrySchema.extend({
  content: z.string(),
});

export type FileHistoryEntry = z.infer<typeof fileHistoryEntrySchema>;
export type FileHistoryDetail = z.infer<typeof fileHistoryDetailSchema>;

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

const connectionSchema = z.object({
  id: z.string(),
  label: z.string(),
  baseUrl: z.string(),
  model: z.string(),
  timeoutSeconds: z.number().nullable().optional(),
  isActive: z.boolean(),
  apiKeyConfigured: z.boolean(),
});

const searchProviderConfigsSchema = z.object({
  tavily: z
    .object({ apiKeyConfigured: z.boolean().optional(), baseUrl: z.string().optional() })
    .partial()
    .optional(),
  serpapi: z
    .object({
      apiKeyConfigured: z.boolean().optional(),
      engine: z.string().optional(),
      baseUrl: z.string().optional(),
    })
    .partial()
    .optional(),
  searxng: z
    .object({ url: z.string().optional(), categories: z.array(z.string()).optional() })
    .partial()
    .optional(),
  ollama: z
    .object({ apiKeyConfigured: z.boolean().optional(), url: z.string().optional() })
    .partial()
    .optional(),
});

const fetchProviderConfigsSchema = z.object({
  firecrawl: z
    .object({ apiKeyConfigured: z.boolean().optional(), baseUrl: z.string().optional() })
    .partial()
    .optional(),
});

const settingsSchema = z.object({
  connections: z.array(connectionSchema).optional(),
  embedding: z
    .object({
      enabled: z.boolean(),
      baseUrl: z.string().nullable().optional(),
      model: z.string().nullable().optional(),
      timeoutSeconds: z.number().nullable().optional(),
      apiKeyConfigured: z.boolean(),
    })
    .optional(),
  image: z
    .object({
      baseUrl: z.string().nullable().optional(),
      model: z.string().nullable().optional(),
      size: z.string().nullable().optional(),
      timeoutSeconds: z.number().nullable().optional(),
      apiKeyConfigured: z.boolean(),
    })
    .optional(),
  search: z
    .object({
      provider: z.string().nullable().optional(),
      providers: searchProviderConfigsSchema,
    })
    .optional(),
  fetch: z
    .object({
      provider: z.string().nullable().optional(),
      providers: fetchProviderConfigsSchema,
    })
    .optional(),
  defaults: z
    .object({
      language: z.string(),
      defaultQueryLimit: z.number(),
    })
    .optional(),
});

export type SystemSettings = z.infer<typeof settingsSchema>;
export type ProviderConnection = z.infer<typeof connectionSchema>;

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
      sources: z.array(z.string()).default([]),
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
    sources: z.array(z.string()).default([]),
  }),
  neighbors: z.array(
    z.object({
      id: z.string(),
      label: z.string(),
      nodeType: z.string(),
      path: z.string(),
      linkCount: z.number(),
      sources: z.array(z.string()).default([]),
    }),
  ),
});

function csrfHeader() {
  return {
    "x-csrf-token": getCsrfToken(),
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
      agentMode: z.string().nullable().optional(),
      agentEvents: z.unknown().nullable().optional(),
      createdAt: z.string(),
    }),
  ),
});

const agentSkillsSchema = z.object({
  skills: z.array(
    z.object({
      id: z.string(),
      name: z.string(),
      description: z.string(),
      source: z.enum(["project", "global"]),
    }),
  ),
});

const agentSkillDetailSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  instructions: z.string(),
  source: z.enum(["project", "global"]),
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

export async function register(input: { username: string; password: string }) {
  return apiFetch(
    "/api/auth/register",
    {
      method: "POST",
      body: JSON.stringify(input),
    },
    loginSchema,
  );
}

export async function logout() {
  return apiFetch(
    "/api/auth/logout",
    {
      method: "POST",
      headers: { "x-csrf-token": getCsrfToken() },
    },
    z.unknown(),
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

export function rawFileUrl(projectId: string, path: string) {
  return `/api/projects/${projectId}/files/raw?path=${encodeURIComponent(path)}`;
}

export async function listFileHistory(input: { projectId: string; path: string }) {
  const searchParams = new URLSearchParams({ path: input.path });
  const response = await apiFetch(
    `/api/projects/${input.projectId}/files/history?${searchParams.toString()}`,
    { method: "GET" },
    fileHistoryListSchema,
  );
  return response.entries;
}

export async function getFileHistoryEntry(input: { projectId: string; entryId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/files/history/${input.entryId}`,
    { method: "GET" },
    fileHistoryDetailSchema,
  );
}

export async function restoreFileHistoryEntry(input: { projectId: string; entryId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/files/history/${input.entryId}/restore`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    z.object({ path: z.string(), content: z.string() }),
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

export async function saveMessageToWiki(input: {
  projectId: string;
  conversationId: string;
  messageId: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/conversations/${input.conversationId}/messages/${input.messageId}/save-to-wiki`,
    {
      method: "POST",
      headers: csrfHeader(),
    },
    z.object({ path: z.string(), title: z.string() }),
  );
}

export async function listAgentSkills(projectId: string) {
  const response = await apiFetch(
    `/api/projects/${projectId}/agent/skills`,
    { method: "GET" },
    agentSkillsSchema,
  );
  return response.skills;
}

export async function getAgentSkill(input: { projectId: string; skillId: string }) {
  return apiFetch(
    `/api/projects/${input.projectId}/agent/skills/${encodeURIComponent(input.skillId)}`,
    { method: "GET" },
    agentSkillDetailSchema,
  );
}

export async function cancelActiveAgentRun(input: { projectId: string; conversationId: string }) {
  const response = await fetch(
    `/api/projects/${input.projectId}/conversations/${input.conversationId}/messages/active`,
    {
      method: "DELETE",
      credentials: "include",
      headers: csrfHeader(),
    },
  );
  if (!response.ok && response.status !== 404) {
    throw new Error(`failed to cancel agent run (status ${response.status})`);
  }
  return response.ok;
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

export async function createProviderConnection(input: {
  label: string;
  baseUrl: string;
  apiKey?: string;
  model: string;
  timeoutSeconds?: number;
}) {
  return apiFetch(
    "/api/system/provider-connections",
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({
        label: input.label,
        baseUrl: input.baseUrl,
        model: input.model,
        timeoutSeconds: input.timeoutSeconds,
        ...(input.apiKey?.trim() ? { apiKey: input.apiKey.trim() } : {}),
      }),
    },
    settingsSchema,
  );
}

export async function updateProviderConnection(
  id: string,
  input: {
    label: string;
    baseUrl: string;
    model: string;
    apiKey?: string;
    clearApiKey?: boolean;
    timeoutSeconds?: number;
  },
) {
  return apiFetch(
    `/api/system/provider-connections/${id}`,
    {
      method: "PATCH",
      headers: csrfHeader(),
      body: JSON.stringify({
        label: input.label,
        baseUrl: input.baseUrl,
        model: input.model,
        timeoutSeconds: input.timeoutSeconds,
        ...(input.apiKey?.trim() ? { apiKey: input.apiKey.trim() } : {}),
        ...(input.clearApiKey && !input.apiKey?.trim() ? { clearApiKey: true } : {}),
      }),
    },
    settingsSchema,
  );
}

export async function deleteProviderConnection(id: string) {
  return apiFetch(
    `/api/system/provider-connections/${id}`,
    { method: "DELETE", headers: csrfHeader() },
    settingsSchema,
  );
}

export async function activateProviderConnection(id: string) {
  return apiFetch(
    `/api/system/provider-connections/${id}/activate`,
    { method: "POST", headers: csrfHeader() },
    settingsSchema,
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
  image?: {
    baseUrl?: string;
    model?: string;
    size?: string;
    timeoutSeconds?: number;
    apiKey?: string;
    clearApiKey?: boolean;
  };
  embedding?: {
    enabled?: boolean;
    baseUrl?: string;
    model?: string;
    timeoutSeconds?: number;
    apiKey?: string;
    clearApiKey?: boolean;
  };
  search?: {
    provider?: string;
    providers?: Record<string, Record<string, unknown>>;
  };
  fetch?: {
    provider?: string;
    providers?: Record<string, Record<string, unknown>>;
  };
  defaults?: {
    language?: string;
    defaultQueryLimit?: number;
  };
}) {
  const payload = {
    ...(input.image
      ? {
          image: {
            baseUrl: input.image.baseUrl,
            model: input.image.model,
            size: input.image.size,
            timeoutSeconds: input.image.timeoutSeconds,
            ...(input.image.apiKey?.trim() ? { apiKey: input.image.apiKey.trim() } : {}),
            ...(input.image.clearApiKey && !input.image.apiKey?.trim()
              ? { clearApiKey: true }
              : {}),
          },
        }
      : {}),
    ...(input.embedding
      ? {
          embedding: {
            enabled: input.embedding.enabled,
            baseUrl: input.embedding.baseUrl,
            model: input.embedding.model,
            timeoutSeconds: input.embedding.timeoutSeconds,
            ...(input.embedding.apiKey?.trim() ? { apiKey: input.embedding.apiKey.trim() } : {}),
            ...(input.embedding.clearApiKey && !input.embedding.apiKey?.trim()
              ? { clearApiKey: true }
              : {}),
          },
        }
      : {}),
    ...(input.search
      ? {
          search: {
            provider: input.search.provider,
            ...(input.search.providers
              ? {
                  providers: Object.fromEntries(
                    Object.entries(input.search.providers).map(([name, fields]) => [
                      name,
                      Object.fromEntries(
                        Object.entries(fields).filter(([, v]) => v !== ""),
                      ),
                    ]),
                  ),
                }
              : {}),
          },
        }
      : {}),
    ...(input.fetch
      ? {
          fetch: {
            provider: input.fetch.provider,
            ...(input.fetch.providers
              ? {
                  providers: Object.fromEntries(
                    Object.entries(input.fetch.providers).map(([name, fields]) => [
                      name,
                      Object.fromEntries(
                        Object.entries(fields).filter(([, v]) => v !== ""),
                      ),
                    ]),
                  ),
                }
              : {}),
          },
        }
      : {}),
    ...(input.defaults ? { defaults: input.defaults } : {}),
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

const skillMetadataSchema = z.object({
  command: z.string(),
  name: z.string(),
  description: z.string(),
  requiresSelection: z.boolean(),
  argumentHint: z.string().nullable().optional(),
  outputNodeType: z.string(),
});

export type SkillMetadata = z.infer<typeof skillMetadataSchema>;

export async function fetchSkills() {
  return apiFetch("/api/skills", { method: "GET" }, z.array(skillMetadataSchema));
}

const skillJobStatusSchema = z.object({
  status: z.enum(["queued", "running", "done", "error"]),
  result: z
    .object({ assetId: z.string(), url: z.string(), title: z.string().optional() })
    .nullish(),
  error: z.object({ message: z.string() }).nullish(),
  progress: z
    .object({ stage: z.string(), message: z.string(), elapsedS: z.number().nullish() })
    .partial()
    .nullish(),
});

export type SkillJobStatus = z.infer<typeof skillJobStatusSchema>;
export type SkillJobProgress = NonNullable<SkillJobStatus["progress"]>;

export async function fetchSkillJob(id: string) {
  return apiFetch(`/api/canvas-skill-jobs/${id}`, { method: "GET" }, skillJobStatusSchema);
}

// A retry issues a brand-new job id (the poller dedupes terminal jobs by id,
// so the old id would never be polled again); callers must rebind the node.
export async function retrySkillJob(id: string) {
  return apiFetch(
    `/api/canvas-skill-jobs/${id}/retry`,
    { method: "POST", headers: csrfHeader() },
    z.object({ jobId: z.string() }),
  );
}
