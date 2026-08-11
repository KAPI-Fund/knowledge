import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { LintPage } from "./page";

const mockCreateLintTask = vi.fn();
const mockTaskDetail = vi.fn();
const mockLintItems = vi.fn();
const mockFixLintItem = vi.fn();
const mockDeleteLintOrphan = vi.fn();
const mockDismissLintItems = vi.fn();
const mockSendLintItemsToReview = vi.fn();

vi.mock("./queries", () => ({
  useCreateLintTaskMutation: () => ({
    mutateAsync: mockCreateLintTask,
    isPending: false,
  }),
  useLintTaskDetailQuery: () => ({
    data: mockTaskDetail(),
  }),
  useLintItemsQuery: () => ({
    data: mockLintItems(),
    isLoading: false,
  }),
  useFixLintItemMutation: () => ({
    mutateAsync: mockFixLintItem,
    isPending: false,
  }),
  useDeleteLintOrphanMutation: () => ({
    mutateAsync: mockDeleteLintOrphan,
    isPending: false,
  }),
  useDismissLintItemsMutation: () => ({
    mutateAsync: mockDismissLintItems,
    isPending: false,
  }),
  useSendLintItemsToReviewMutation: () => ({
    mutateAsync: mockSendLintItemsToReview,
    isPending: false,
  }),
}));

const brokenLinkItem = {
  id: "item-1",
  issueType: "broken-link",
  severity: "warning",
  page: "concepts/attention.md",
  detail: "Broken link: [[attention-mechanisms]] - target page not found.",
  brokenTarget: "attention-mechanisms",
  suggestedTarget: "concepts/attention-mechanism.md",
  mode: "structural",
  createdAt: "2026-07-11T00:00:00Z",
};

const orphanItem = {
  id: "item-2",
  issueType: "orphan",
  severity: "warning",
  page: "notes/loose.md",
  detail: "Page has no incoming wikilinks.",
  mode: "structural",
  createdAt: "2026-07-11T00:00:00Z",
};

function renderPage() {
  const queryClient = new QueryClient();
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/projects/project-1/lint"]}>
        <Routes>
          <Route path="projects/:projectId/lint" element={<LintPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("lint page", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockTaskDetail.mockReturnValue(undefined);
    mockLintItems.mockReturnValue([brokenLinkItem, orphanItem]);
  });

  it("queues a structural lint task", async () => {
    const user = userEvent.setup();
    mockCreateLintTask.mockResolvedValue({ taskId: "task-1", status: "queued" });

    renderPage();

    await user.click(screen.getByRole("button", { name: "Run Structural Lint" }));

    expect(mockCreateLintTask).toHaveBeenCalledWith({
      projectId: "project-1",
      mode: "structural",
    });
  });

  it("renders suggestion badges and review fallback", () => {
    renderPage();

    expect(screen.getByText("concepts/attention-mechanism.md")).toBeInTheDocument();
    expect(screen.getByText("→ Review")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Broken Link" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Orphan Page" })).toBeInTheDocument();
  });

  it("fixes an item from the detail card", async () => {
    const user = userEvent.setup();
    mockFixLintItem.mockResolvedValue({ action: "fixed", changedPaths: ["wiki/concepts/attention.md"] });

    renderPage();

    await user.click(screen.getByRole("button", { name: "Broken Link" }));
    expect(screen.getByText("Suggested target: concepts/attention-mechanism.md")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Fix" }));

    expect(mockFixLintItem).toHaveBeenCalledWith({
      projectId: "project-1",
      itemId: "item-1",
    });
  });

  it("dismisses selected items from the batch toolbar", async () => {
    const user = userEvent.setup();
    mockDismissLintItems.mockResolvedValue({ dismissedIds: ["item-1", "item-2"] });

    renderPage();

    await user.click(screen.getByRole("checkbox", { name: "Select all" }));
    expect(screen.getByText("2 selected")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Ignore selected" }));

    expect(mockDismissLintItems).toHaveBeenCalledWith({
      projectId: "project-1",
      ids: ["item-1", "item-2"],
    });
  });

  it("sends selected items to review", async () => {
    const user = userEvent.setup();
    mockSendLintItemsToReview.mockResolvedValue({ reviewIds: ["lint-1"] });

    renderPage();

    await user.click(screen.getByRole("checkbox", { name: "Select concepts/attention.md" }));
    await user.click(screen.getByRole("button", { name: "Send selected to Review" }));

    expect(mockSendLintItemsToReview).toHaveBeenCalledWith({
      projectId: "project-1",
      ids: ["item-1"],
    });
  });

  it("deletes an orphan page through the confirm dialog", async () => {
    const user = userEvent.setup();
    mockDeleteLintOrphan.mockResolvedValue({ deletedPaths: ["wiki/notes/loose.md"], rewrittenFiles: 1 });

    renderPage();

    await user.click(screen.getByRole("button", { name: "Orphan Page" }));
    await user.click(screen.getByRole("button", { name: "Delete" }));

    const dialog = await screen.findByRole("alertdialog");
    expect(
      within(dialog).getByText('Delete orphan page "notes/loose.md"?'),
    ).toBeInTheDocument();

    await user.click(within(dialog).getByRole("button", { name: "Delete" }));

    expect(mockDeleteLintOrphan).toHaveBeenCalledWith({
      projectId: "project-1",
      itemId: "item-2",
    });
  });
});
