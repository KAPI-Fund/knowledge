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
      taskType: z.string(),
      status: z.string(),
      title: z.string(),
      relativePath: z.string().nullable().optional(),
      detail: z.unknown().optional(),
      createdAt: z.string().optional(),
      updatedAt: z.string().optional(),
    }),
  ),
});

const taskSchema = z.object({
  id: z.string(),
  taskType: z.string(),
  status: z.string(),
  title: z.string(),
  relativePath: z.string().nullable().optional(),
  detail: z.unknown(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

const sourcesSchema = z.object({
  sources: z.array(
    z.object({
      relativePath: z.string(),
      size: z.number(),
    }),
  ),
});

const reviewsSchema = z.object({
  reviews: z.array(
    z.object({
      id: z.string(),
      status: z.string(),
      title: z.string(),
      description: z.string().optional(),
    }),
  ),
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
});

const searchResultsSchema = z.object({
  results: z.array(
    z.object({
      path: z.string(),
      title: z.string(),
      snippet: z.string(),
      score: z.number(),
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
    }),
  ),
  edges: z.array(
    z.object({
      source: z.string(),
      target: z.string(),
    }),
  ),
});

const graphNeighborsSchema = z.object({
  node: z.object({
    id: z.string(),
    label: z.string(),
    nodeType: z.string(),
    path: z.string(),
  }),
  neighbors: z.array(
    z.object({
      id: z.string(),
      label: z.string(),
      nodeType: z.string(),
      path: z.string(),
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

export async function createProject(input: { name: string; rootPath: string; csrfToken: string }) {
  return apiFetch(
    "/api/projects",
    {
      method: "POST",
      headers: {
        "x-csrf-token": input.csrfToken,
      },
      body: JSON.stringify({
        name: input.name,
        rootPath: input.rootPath,
      }),
    },
    projectSchema,
  );
}

export async function listProjectSources(projectId: string) {
  const response = await apiFetch(`/api/projects/${projectId}/sources`, { method: "GET" }, sourcesSchema);
  return response.sources;
}

function encodeContentBase64(content: string) {
  const bytes = new TextEncoder().encode(content);
  const binary = Array.from(bytes, (byte) => String.fromCharCode(byte)).join("");
  return window.btoa(binary);
}

export async function importProjectSource(input: {
  projectId: string;
  fileName: string;
  content: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/sources:import`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({
        fileName: input.fileName,
        contentBase64: encodeContentBase64(input.content),
      }),
    },
    z.object({
      relativePath: z.string(),
      size: z.number(),
    }),
  );
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
      summaryPath: z.string(),
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
      discoveredCount: z.number(),
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
    z.any(),
  );
}

export async function searchProject(input: { projectId: string; query: string }) {
  const response = await apiFetch(
    `/api/projects/${input.projectId}/search`,
    {
      method: "POST",
      body: JSON.stringify({ query: input.query }),
    },
    searchResultsSchema,
  );
  return response.results;
}

export async function getProjectGraph(projectId: string) {
  return apiFetch(`/api/projects/${projectId}/graph`, { method: "GET" }, graphSchema);
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

export async function listProjectReviews(projectId: string) {
  const response = await apiFetch(`/api/projects/${projectId}/reviews`, { method: "GET" }, reviewsSchema);
  return response.reviews;
}

export async function listProjectAuditLogs(projectId: string) {
  const response = await apiFetch(`/api/projects/${projectId}/audit-logs`, { method: "GET" }, auditSchema);
  return response.items;
}

export async function getSystemSettings() {
  return apiFetch("/api/system/settings", { method: "GET" }, settingsSchema);
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
      id: z.string(),
      status: z.string(),
      title: z.string(),
      description: z.string().optional(),
    }),
  );
}

export async function updateSystemSettings(input: {
  providerMode: string;
  language: string;
  defaultQueryLimit: number;
}) {
  return apiFetch(
    "/api/system/settings",
    {
      method: "PATCH",
      headers: csrfHeader(),
      body: JSON.stringify(input),
    },
    settingsSchema,
  );
}
