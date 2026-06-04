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
