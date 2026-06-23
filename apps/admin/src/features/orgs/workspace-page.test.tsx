import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

vi.mock("../spaces/use-spaces", () => ({
  useSpacesQuery: () => ({
    data: {
      personal: { spaceId: "personal-space" },
      orgs: [
        { id: "org-1", slug: "acme", name: "Acme", spaceId: "org-space-1", role: "org_admin" },
      ],
      teams: [
        { id: "team-1", orgId: "org-1", slug: "platform", name: "Platform", spaceId: "ts1", role: "leader" },
      ],
    },
    isLoading: false,
  }),
}));

vi.mock("./workspace-queries", () => ({
  useOrgProjectsQuery: () => ({
    data: [
      { id: "kb1", name: "Handbook", rootPath: "/h", createdAt: "t", spaceKind: "org", teamId: null, teamSlug: null, role: "owner" },
      { id: "kb2", name: "Runbook", rootPath: "/r", createdAt: "t", spaceKind: "team", teamId: "team-1", teamSlug: "platform", role: "editor" },
    ],
    isLoading: false,
  }),
  useOrgTeamsQuery: () => ({
    data: {
      teams: [
        { id: "team-1", name: "Platform", slug: "platform", orgId: "org-1", spaceId: "ts1", role: "leader" },
      ],
    },
    isLoading: false,
  }),
}));

import { OrgWorkspacePage } from "./workspace-page";

function renderPage() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/orgs/org-1"]}>
        <Routes>
          <Route path="/orgs/:orgId" element={<OrgWorkspacePage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("OrgWorkspacePage", () => {
  it("renders a Public projects section and a per-team section", () => {
    renderPage();
    expect(screen.getByText("Public projects")).toBeInTheDocument();
    expect(screen.getByText("Handbook")).toBeInTheDocument();
    expect(screen.getByText(/Platform/)).toBeInTheDocument();
    expect(screen.getByText("Runbook")).toBeInTheDocument();
  });

  it("shows admin actions for an org_admin", () => {
    renderPage();
    // Exact-string names: "New team" must not collide with the team section's
    // "New team KB" button (a /New team/i regex would match both and throw).
    expect(screen.getByRole("button", { name: "New public project" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New team" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New team KB" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Members" })).toBeInTheDocument();
  });
});
