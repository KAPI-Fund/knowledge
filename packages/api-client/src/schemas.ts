import { z } from "zod";

export const currentUserSchema = z.object({
  id: z.string(),
  username: z.string(),
  role: z.string(),
});

export function parseCurrentUser(input: unknown) {
  return currentUserSchema.parse(input);
}

export const spaceListSchema = z.object({
  personal: z.object({ spaceId: z.string().nullable() }),
  orgs: z.array(
    z.object({
      id: z.string(),
      slug: z.string(),
      name: z.string(),
      spaceId: z.string(),
      role: z.enum(["org_admin", "org_member"]),
    }),
  ),
  teams: z.array(
    z.object({
      id: z.string(),
      orgId: z.string(),
      slug: z.string(),
      name: z.string(),
      spaceId: z.string(),
      role: z.enum(["leader", "member"]),
    }),
  ),
});

export function parseSpaceList(input: unknown) {
  return spaceListSchema.parse(input);
}

export const scopedProjectSchema = z.object({
  id: z.string(),
  name: z.string(),
  rootPath: z.string(),
  createdAt: z.string(),
  spaceKind: z.enum(["personal", "org", "team"]),
  teamId: z.string().nullable(),
  teamSlug: z.string().nullable(),
  role: z.enum(["owner", "editor", "viewer"]),
});

export const projectListSchema = z.object({
  projects: z.array(scopedProjectSchema),
});

export function parseProjectList(input: unknown) {
  return projectListSchema.parse(input);
}

export const teamListSchema = z.object({
  teams: z.array(
    z.object({
      id: z.string(),
      name: z.string(),
      slug: z.string(),
      orgId: z.string(),
    }),
  ),
});

export function parseTeamList(input: unknown) {
  return teamListSchema.parse(input);
}

export const grantSchema = z.object({
  projectId: z.string(),
  userId: z.string(),
  role: z.enum(["editor", "viewer"]),
  canImport: z.boolean(),
});

export function parseGrant(input: unknown) {
  return grantSchema.parse(input);
}
