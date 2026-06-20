import { describe, expect, it } from "vitest";
import {
  parseCurrentUser,
  parseSpaceList,
  parseProjectList,
  parseTeamList,
  parseGrant,
  parseOrgMemberList,
  parseTeamMemberList,
  parseProjectMemberList,
} from "./schemas";

describe("parseCurrentUser", () => {
  it("parses a valid current-user payload", () => {
    expect(parseCurrentUser({ id: "u1", username: "admin", role: "admin" }).username).toBe(
      "admin",
    );
  });
});

describe("parseSpaceList", () => {
  it("parses personal + orgs + teams", () => {
    const result = parseSpaceList({
      personal: { spaceId: "sp1" },
      orgs: [{ id: "o1", slug: "acme", name: "Acme", spaceId: "os1", role: "org_admin" }],
      teams: [
        { id: "t1", orgId: "o1", slug: "plat", name: "Platform", spaceId: "ts1", role: "leader" },
      ],
    });
    expect(result.personal.spaceId).toBe("sp1");
    expect(result.orgs[0].role).toBe("org_admin");
    expect(result.teams[0].orgId).toBe("o1");
  });

  it("allows a null personal spaceId", () => {
    const result = parseSpaceList({ personal: { spaceId: null }, orgs: [], teams: [] });
    expect(result.personal.spaceId).toBeNull();
  });
});

describe("parseProjectList", () => {
  it("parses space-scoped projects", () => {
    const result = parseProjectList({
      projects: [
        {
          id: "p1",
          name: "KB",
          rootPath: "/tmp/p1",
          createdAt: "2026-06-19T00:00:00Z",
          spaceKind: "team",
          teamId: "t1",
          teamSlug: "plat",
          role: "editor",
        },
      ],
    });
    expect(result.projects[0].spaceKind).toBe("team");
    expect(result.projects[0].role).toBe("editor");
  });

  it("allows null team fields for non-team KBs", () => {
    const result = parseProjectList({
      projects: [
        {
          id: "p1",
          name: "KB",
          rootPath: "/tmp/p1",
          createdAt: "2026-06-19T00:00:00Z",
          spaceKind: "org",
          teamId: null,
          teamSlug: null,
          role: "viewer",
        },
      ],
    });
    expect(result.projects[0].teamId).toBeNull();
  });
});

describe("parseTeamList", () => {
  it("parses teams", () => {
    const result = parseTeamList({
      teams: [{ id: "t1", name: "Platform", slug: "plat", orgId: "o1" }],
    });
    expect(result.teams[0].slug).toBe("plat");
  });
});

describe("parseGrant", () => {
  it("parses a grant response", () => {
    const result = parseGrant({
      projectId: "p1",
      userId: "u1",
      role: "editor",
      canImport: true,
    });
    expect(result.role).toBe("editor");
    expect(result.canImport).toBe(true);
  });

  it("rejects an invalid role", () => {
    expect(() => parseGrant({ projectId: "p1", userId: "u1", role: "owner", canImport: true }))
      .toThrow();
  });
});

describe("parseOrgMemberList", () => {
  it("parses org members with username and role", () => {
    const result = parseOrgMemberList({
      members: [
        { userId: "u1", username: "alice", role: "org_admin" },
        { userId: "u2", username: "bob", role: "org_member" },
      ],
    });
    expect(result.members).toHaveLength(2);
    expect(result.members[0]).toEqual({
      userId: "u1",
      username: "alice",
      role: "org_admin",
    });
  });

  it("rejects an unknown role", () => {
    expect(() =>
      parseOrgMemberList({
        members: [{ userId: "u1", username: "alice", role: "wizard" }],
      }),
    ).toThrow();
  });
});

describe("parseTeamMemberList", () => {
  it("parses team members with leader/member roles", () => {
    const result = parseTeamMemberList({
      members: [
        { userId: "u1", username: "alice", role: "leader" },
        { userId: "u2", username: "bob", role: "member" },
      ],
    });
    expect(result.members[1]).toEqual({
      userId: "u2",
      username: "bob",
      role: "member",
    });
  });
});

describe("parseProjectMemberList", () => {
  it("parses project members with canImport", () => {
    const result = parseProjectMemberList({
      members: [
        { userId: "u1", username: "alice", role: "owner", canImport: true },
        { userId: "u2", username: "bob", role: "viewer", canImport: false },
      ],
    });
    expect(result.members[0].canImport).toBe(true);
    expect(result.members[1].role).toBe("viewer");
  });
});
