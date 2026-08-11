import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { DedupPage } from "./page";

const mockDedupOverview = vi.fn();
const mockDetectDuplicates = vi.fn();
const mockMergeGroup = vi.fn();
const mockDismissGroup = vi.fn();

vi.mock("./queries", () => ({
  useProjectDedupQuery: () => ({
    data: mockDedupOverview(),
  }),
  useDetectDuplicatesMutation: () => ({
    mutateAsync: mockDetectDuplicates,
    isPending: false,
  }),
  useMergeDuplicateGroupMutation: () => ({
    mutateAsync: mockMergeGroup,
    isPending: false,
  }),
  useDismissDuplicateGroupMutation: () => ({
    mutateAsync: mockDismissGroup,
    isPending: false,
  }),
}));

function renderDedupPage() {
  const queryClient = new QueryClient();
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/projects/project-1/dedup"]}>
        <Routes>
          <Route path="projects/:projectId/dedup" element={<DedupPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("dedup page", () => {
  it("triggers duplicate detection", async () => {
    const user = userEvent.setup();
    mockDedupOverview.mockReturnValue({ groups: [], notDuplicates: [] });
    mockDetectDuplicates.mockResolvedValue({ taskId: "task-1", status: "queued" });

    renderDedupPage();

    expect(screen.getByText("No duplicate candidates")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Detect Duplicates" }));

    expect(mockDetectDuplicates).toHaveBeenCalledWith({ projectId: "project-1" });
  });

  it("merges a group with the selected canonical slug", async () => {
    const user = userEvent.setup();
    mockDedupOverview.mockReturnValue({
      groups: [
        {
          id: "group-1",
          slugs: ["attention", "attention-mechanism"],
          reason: "Both describe the attention mechanism.",
          confidence: "high",
          status: "candidate",
          createdAt: "2026-06-12T00:00:00Z",
        },
      ],
      notDuplicates: [],
    });
    mockMergeGroup.mockResolvedValue({ taskId: "task-2", status: "queued" });

    renderDedupPage();

    expect(screen.getByText("attention / attention-mechanism")).toBeInTheDocument();
    expect(screen.getByText("Both describe the attention mechanism.")).toBeInTheDocument();

    await user.click(screen.getByRole("combobox", { name: "Canonical Slug" }));
    await user.click(await screen.findByRole("option", { name: "attention-mechanism" }));
    await user.click(screen.getByRole("button", { name: "Merge" }));
    await user.click(await screen.findByRole("button", { name: "Merge pages" }));

    expect(mockMergeGroup).toHaveBeenCalledWith({
      projectId: "project-1",
      groupId: "group-1",
      canonicalSlug: "attention-mechanism",
    });
  });

  it("dismisses a group as not duplicates", async () => {
    const user = userEvent.setup();
    mockDedupOverview.mockReturnValue({
      groups: [
        {
          id: "group-1",
          slugs: ["attention", "attention-mechanism"],
          reason: "Both describe the attention mechanism.",
          confidence: "high",
          status: "candidate",
          createdAt: "2026-06-12T00:00:00Z",
        },
      ],
      notDuplicates: [["rope", "rotary-embedding"]],
    });
    mockDismissGroup.mockResolvedValue({ dismissed: true });

    renderDedupPage();

    expect(screen.getByText("rope, rotary-embedding")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Not Duplicates" }));

    expect(mockDismissGroup).toHaveBeenCalledWith({
      projectId: "project-1",
      groupId: "group-1",
    });
  });
});
