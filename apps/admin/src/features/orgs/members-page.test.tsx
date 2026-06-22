import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

let callerRole = "org_admin";
vi.mock("../spaces/use-spaces", () => ({
  useSpacesQuery: () => ({
    data: {
      personal: { spaceId: "p" },
      orgs: [{ id: "org-1", slug: "acme", name: "Acme", spaceId: "s1", role: callerRole }],
      teams: [],
    },
    isLoading: false,
  }),
}));

vi.mock("./members-queries", () => ({
  useOrgMembersQuery: () => ({
    data: {
      members: [
        { userId: "u1", username: "alice", role: "org_admin" },
        { userId: "u2", username: "bob", role: "org_member" },
      ],
    },
    isLoading: false,
  }),
}));

vi.mock("./members-mutations", () => ({
  useAddOrgMemberMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useSetOrgMemberRoleMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useRemoveOrgMemberMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

import { OrgMembersPage } from "./members-page";

function renderPage() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/orgs/org-1/members"]}>
        <Routes>
          <Route path="/orgs/:orgId/members" element={<OrgMembersPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("OrgMembersPage", () => {
  it("renders members for any member", () => {
    callerRole = "org_member";
    renderPage();
    expect(screen.getByText("alice")).toBeInTheDocument();
    expect(screen.getByText("bob")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Add member/i })).toBeNull();
  });

  it("shows admin controls for an org_admin", () => {
    callerRole = "org_admin";
    renderPage();
    expect(screen.getByRole("button", { name: /Add member/i })).toBeInTheDocument();
  });
});
