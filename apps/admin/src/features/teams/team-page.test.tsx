import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

vi.mock("../spaces/use-spaces", () => ({
  useSpacesQuery: () => ({
    data: {
      personal: { spaceId: "p" },
      orgs: [{ id: "org-1", slug: "acme", name: "Acme", spaceId: "s1", role: "org_member" }],
      teams: [
        { id: "team-1", orgId: "org-1", slug: "platform", name: "Platform", spaceId: "ts1", role: "leader" },
      ],
    },
    isLoading: false,
  }),
}));

vi.mock("./team-queries", () => ({
  useTeamMembersQuery: () => ({
    data: {
      members: [
        { userId: "u1", username: "lead", role: "leader" },
        { userId: "u2", username: "dev", role: "member" },
      ],
    },
    isLoading: false,
  }),
  useTeamProjectsQuery: () => ({
    data: [
      { id: "kb1", name: "Runbook", rootPath: "/r", createdAt: "t", spaceKind: "team", teamId: "team-1", teamSlug: "platform", role: "editor" },
    ],
    isLoading: false,
  }),
}));

vi.mock("./team-mutations", () => ({
  useAddTeamMemberMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useRemoveTeamMemberMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useCreateTeamKbMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

import { TeamPage } from "./team-page";

function renderPage() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/orgs/org-1/teams/team-1"]}>
        <Routes>
          <Route path="/orgs/:orgId/teams/:teamId" element={<TeamPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("TeamPage", () => {
  it("renders members and team KBs", () => {
    renderPage();
    expect(screen.getByText("lead")).toBeInTheDocument();
    expect(screen.getByText("dev")).toBeInTheDocument();
    expect(screen.getByText("Runbook")).toBeInTheDocument();
  });

  it("leader sees add controls but cannot remove the leader row", () => {
    renderPage();
    expect(screen.getByRole("button", { name: /Add member/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Remove dev/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Remove lead/i })).toBeNull();
  });
});
