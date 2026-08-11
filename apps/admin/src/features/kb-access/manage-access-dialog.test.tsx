import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const removeGrantMock = vi.fn();
const upsertGrantMock = vi.fn();

vi.mock("./manage-access-queries", () => ({
  useProjectGranteesQuery: () => ({
    data: {
      members: [
        { userId: "u1", username: "alice", role: "owner", canImport: true },
        { userId: "u2", username: "bob", role: "viewer", canImport: false },
      ],
    },
    isLoading: false,
  }),
  useGrantCandidatesQuery: () => ({
    data: [
      { userId: "u2", username: "bob" },
      { userId: "u3", username: "carol" },
    ],
    isLoading: false,
  }),
}));

vi.mock("./manage-access-mutations", () => ({
  useUpsertGrantMutation: () => ({ mutateAsync: upsertGrantMock, isPending: false }),
  useRemoveGrantMutation: () => ({ mutateAsync: removeGrantMock, isPending: false }),
}));

import { ManageAccessDialog } from "./manage-access-dialog";

function renderDialog() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <ManageAccessDialog
        projectId="p1"
        spaceKind="org"
        orgId="org-1"
        teamId={null}
        open
        onOpenChange={() => {}}
      />
    </QueryClientProvider>,
  );
}

describe("ManageAccessDialog", () => {
  it("lists current grantees by username", () => {
    renderDialog();
    expect(screen.getByText("alice")).toBeInTheDocument();
    expect(screen.getByText("bob")).toBeInTheDocument();
  });

  it("removes a grantee", () => {
    renderDialog();
    fireEvent.click(screen.getByRole("button", { name: /remove bob/i }));
    expect(removeGrantMock).toHaveBeenCalledWith({ userId: "u2" });
  });

  it("adds a grant for a chosen candidate", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(screen.getByRole("combobox", { name: "Add user" }));
    await user.click(await screen.findByRole("option", { name: "carol" }));
    await user.click(screen.getByRole("combobox", { name: "Grant role" }));
    await user.click(await screen.findByRole("option", { name: "editor" }));
    await user.click(screen.getByRole("button", { name: /add grant/i }));
    expect(upsertGrantMock).toHaveBeenCalledWith({ userId: "u3", role: "editor" });
  });
});
