import {
  apiFetch,
  spaceListSchema,
  projectListSchema,
  teamListSchema,
  orgMemberListSchema,
  teamMemberListSchema,
  projectMemberListSchema,
  grantSchema,
} from "@knowledge/api-client";
import { z } from "zod";

function csrfHeader(): Record<string, string> {
  return {
    "x-csrf-token": window.sessionStorage.getItem("knowledge.csrfToken") ?? "",
  };
}

export function fetchSpaces() {
  return apiFetch("/api/spaces", { method: "GET" }, spaceListSchema);
}

export async function fetchOrgProjects(spaceId: string) {
  const response = await apiFetch(
    `/api/projects?space_id=${encodeURIComponent(spaceId)}`,
    { method: "GET" },
    projectListSchema,
  );
  return response.projects;
}

export function fetchOrgTeams(orgId: string) {
  return apiFetch(`/api/orgs/${orgId}/teams`, { method: "GET" }, teamListSchema);
}

export function fetchOrgMembers(orgId: string) {
  return apiFetch(
    `/api/orgs/${orgId}/members`,
    { method: "GET" },
    orgMemberListSchema,
  );
}

export function fetchTeamMembers(orgId: string, teamId: string) {
  return apiFetch(
    `/api/orgs/${orgId}/teams/${teamId}/members`,
    { method: "GET" },
    teamMemberListSchema,
  );
}

export function fetchProjectMembers(projectId: string) {
  return apiFetch(
    `/api/projects/${projectId}/members`,
    { method: "GET" },
    projectMemberListSchema,
  );
}

const createdOrgSchema = z.object({
  id: z.string(),
  name: z.string(),
  slug: z.string(),
  spaceId: z.string(),
});

export function createOrg(input: { name: string; slug: string }) {
  return apiFetch(
    "/api/orgs",
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify({ name: input.name, slug: input.slug }),
    },
    createdOrgSchema,
  );
}

const createdProjectSchema = z.object({
  id: z.string(),
  name: z.string(),
  rootPath: z.string(),
  createdAt: z.string(),
});

export function createSpaceProject(input: { name: string; spaceId: string }) {
  return apiFetch(
    "/api/projects",
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify({ name: input.name, spaceId: input.spaceId }),
    },
    createdProjectSchema,
  );
}

const createdTeamSchema = z.object({
  id: z.string(),
  name: z.string(),
  slug: z.string(),
  orgId: z.string(),
  spaceId: z.string(),
});

export function createTeam(orgId: string, input: { name: string; slug: string }) {
  return apiFetch(
    `/api/orgs/${orgId}/teams`,
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify({ name: input.name, slug: input.slug }),
    },
    createdTeamSchema,
  );
}

export function addOrgMember(
  orgId: string,
  input: { usernameOrEmail: string; role: "org_admin" | "org_member" },
) {
  return apiFetch(
    `/api/orgs/${orgId}/members`,
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify(input),
    },
    z.unknown(),
  );
}

export function setOrgMemberRole(
  orgId: string,
  userId: string,
  role: "org_admin" | "org_member",
) {
  return apiFetch(
    `/api/orgs/${orgId}/members/${userId}`,
    {
      method: "PATCH",
      headers: { ...csrfHeader() },
      body: JSON.stringify({ role }),
    },
    z.unknown(),
  );
}

export function removeOrgMember(orgId: string, userId: string) {
  return apiFetch(
    `/api/orgs/${orgId}/members/${userId}`,
    { method: "DELETE", headers: { ...csrfHeader() } },
    z.void(),
  );
}

export function addTeamMember(
  orgId: string,
  teamId: string,
  input: { usernameOrEmail: string },
) {
  return apiFetch(
    `/api/orgs/${orgId}/teams/${teamId}/members`,
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify(input),
    },
    z.unknown(),
  );
}

export function removeTeamMember(orgId: string, teamId: string, userId: string) {
  return apiFetch(
    `/api/orgs/${orgId}/teams/${teamId}/members/${userId}`,
    { method: "DELETE", headers: { ...csrfHeader() } },
    z.void(),
  );
}

export function upsertGrant(
  projectId: string,
  input: { userId: string; role: "editor" | "viewer" },
) {
  return apiFetch(
    `/api/projects/${projectId}/grants`,
    {
      method: "POST",
      headers: { ...csrfHeader() },
      body: JSON.stringify(input),
    },
    grantSchema,
  );
}

export function removeGrant(projectId: string, userId: string) {
  return apiFetch(
    `/api/projects/${projectId}/grants/${userId}`,
    { method: "DELETE", headers: { ...csrfHeader() } },
    z.void(),
  );
}
