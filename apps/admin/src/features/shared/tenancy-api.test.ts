import { describe, expect, it, vi, afterEach } from "vitest";
import { fetchOrgProjects } from "./tenancy-api";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("fetchOrgProjects", () => {
  it("requests /api/projects with the camelCase spaceId query param", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ projects: [] }), { status: 200 }),
    );

    await fetchOrgProjects("s1");

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][0]).toBe("/api/projects?spaceId=s1");
  });
});
