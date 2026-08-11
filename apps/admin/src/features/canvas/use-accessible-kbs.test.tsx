import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../shared/tenancy-api", () => ({
  fetchSpaces: vi.fn(),
  fetchOrgProjects: vi.fn(),
}));

import { fetchOrgProjects, fetchSpaces } from "../shared/tenancy-api";
import { useAccessibleKnowledgeBases } from "./use-accessible-kbs";

type ScopedProject = Awaited<ReturnType<typeof fetchOrgProjects>>[number];

function project(overrides: Partial<ScopedProject> & Pick<ScopedProject, "id" | "name">): ScopedProject {
  return {
    rootPath: "",
    createdAt: "2026-01-01T00:00:00Z",
    spaceKind: "personal",
    teamId: null,
    teamSlug: null,
    role: "viewer",
    ...overrides,
  };
}

function makeWrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

describe("useAccessibleKnowledgeBases", () => {
  it("aggregates personal, org, and team knowledge bases across spaces", async () => {
    vi.mocked(fetchSpaces).mockResolvedValue({
      personal: { spaceId: "p1" },
      orgs: [{ id: "o1", slug: "acme", name: "Acme", spaceId: "os1", role: "org_admin" }],
      teams: [{ id: "t1", orgId: "o1", slug: "eng", name: "Engineering", spaceId: "ts1", role: "leader" }],
    });
    vi.mocked(fetchOrgProjects).mockImplementation(async (spaceId: string) => {
      if (spaceId === "p1") {
        return [project({ id: "kp", name: "My Notes", spaceKind: "personal", role: "owner" })];
      }
      if (spaceId === "os1") {
        return [
          project({ id: "ko", name: "Acme KB", spaceKind: "org", role: "editor" }),
          project({
            id: "kt",
            name: "Eng KB",
            spaceKind: "team",
            teamId: "t1",
            teamSlug: "eng",
            role: "viewer",
          }),
        ];
      }
      return [];
    });

    const { result } = renderHook(() => useAccessibleKnowledgeBases(), { wrapper: makeWrapper() });

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    await waitFor(() => expect(result.current.items).toHaveLength(3));

    const byId = new Map(result.current.items.map((i) => [i.id, i]));
    expect(byId.get("kp")).toMatchObject({ groupKey: "personal", groupLabel: "Personal", groupOrder: 0, role: "owner" });
    expect(byId.get("ko")).toMatchObject({ groupKey: "org:o1", groupLabel: "Acme", groupOrder: 1, role: "editor" });
    expect(byId.get("kt")).toMatchObject({
      groupKey: "team:t1",
      groupLabel: "Engineering",
      groupOrder: 1.5,
      role: "viewer",
    });
  });

  it("skips the personal space fetch when there is no personal space", async () => {
    vi.mocked(fetchSpaces).mockResolvedValue({
      personal: { spaceId: null },
      orgs: [],
      teams: [],
    });

    const { result } = renderHook(() => useAccessibleKnowledgeBases(), { wrapper: makeWrapper() });

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.items).toEqual([]);
    expect(fetchOrgProjects).not.toHaveBeenCalled();
  });

  it("falls back to the team slug when the team name is not in the spaces list", async () => {
    vi.mocked(fetchSpaces).mockResolvedValue({
      personal: { spaceId: null },
      orgs: [{ id: "o1", slug: "acme", name: "Acme", spaceId: "os1", role: "org_member" }],
      teams: [],
    });
    vi.mocked(fetchOrgProjects).mockResolvedValue([
      project({ id: "kt", name: "Orphan KB", spaceKind: "team", teamId: "t9", teamSlug: "ghost", role: "viewer" }),
    ]);

    const { result } = renderHook(() => useAccessibleKnowledgeBases(), { wrapper: makeWrapper() });

    await waitFor(() => expect(result.current.items).toHaveLength(1));
    expect(result.current.items[0]).toMatchObject({ groupKey: "team:t9", groupLabel: "ghost" });
  });
});
