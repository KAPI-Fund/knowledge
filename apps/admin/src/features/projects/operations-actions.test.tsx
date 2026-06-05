import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { GraphPage } from "../graph/page";
import { ReviewsPage } from "../reviews/page";
import { SearchPage } from "../search/page";
import { SettingsPage } from "../settings/page";
import { SourcesPage } from "../sources/page";
import { TasksPage } from "../tasks/page";

const retryTask = vi.fn();
const cancelTask = vi.fn();
const updateReview = vi.fn();
const updateSettings = vi.fn();
const importSource = vi.fn();
const ingestSource = vi.fn();
const deleteSource = vi.fn();
const rescanSources = vi.fn();
const runSearch = vi.fn().mockResolvedValue([
  {
    path: "wiki/sources/demo.md",
    title: "Demo",
    snippet: "Demo content",
    score: 1,
  },
]);

vi.mock("../tasks/queries", () => ({
  useProjectTasksQuery: () => ({
    data: [
      {
        id: "task-1",
        title: "Imported note.md",
        taskType: "source_import",
        status: "queued",
        relativePath: "raw/sources/demo.md",
        detail: {
          size: 42,
        },
        createdAt: "2026-06-05T00:00:00Z",
        updatedAt: "2026-06-05T00:00:01Z",
      },
    ],
    isLoading: false,
  }),
  useTaskDetailQuery: () => ({
    data: {
      id: "task-1",
      title: "Imported note.md",
      taskType: "source_import",
      status: "queued",
      relativePath: "raw/sources/demo.md",
      detail: {
        size: 42,
      },
      createdAt: "2026-06-05T00:00:00Z",
      updatedAt: "2026-06-05T00:00:01Z",
    },
    isLoading: false,
  }),
  useRetryTaskMutation: () => ({
    mutateAsync: retryTask,
  }),
  useCancelTaskMutation: () => ({
    mutateAsync: cancelTask,
  }),
}));

vi.mock("../reviews/queries", () => ({
  useProjectReviewsQuery: () => ({
    data: [
      {
        id: "review-1",
        title: "Review note.md",
        status: "open",
      },
    ],
    isLoading: false,
  }),
  useUpdateReviewMutation: () => ({
    mutateAsync: updateReview,
  }),
}));

vi.mock("../settings/queries", () => ({
  useSystemSettingsQuery: () => ({
    data: {
      providerMode: "deterministic",
      language: "en",
      defaultQueryLimit: 3,
    },
    isLoading: false,
  }),
  useUpdateSystemSettingsMutation: () => ({
    mutateAsync: updateSettings,
  }),
}));

vi.mock("../sources/queries", () => ({
  useProjectSourcesQuery: () => ({
    data: [
      {
        relativePath: "raw/sources/demo.md",
        size: 42,
      },
    ],
    isLoading: false,
  }),
  useImportSourceMutation: () => ({
    mutateAsync: importSource,
  }),
  useIngestSourceMutation: () => ({
    mutateAsync: ingestSource,
  }),
  useDeleteSourceMutation: () => ({
    mutateAsync: deleteSource,
  }),
  useRescanSourcesMutation: () => ({
    mutateAsync: rescanSources,
  }),
}));

vi.mock("../search/queries", () => ({
  useProjectSearchMutation: () => ({
    mutateAsync: runSearch,
  }),
}));

vi.mock("../graph/queries", () => ({
  useProjectGraphQuery: () => ({
    data: {
      nodes: [
        {
          id: "demo",
          label: "Demo",
          nodeType: "source",
          path: "wiki/sources/demo.md",
        },
      ],
      edges: [],
    },
    isLoading: false,
  }),
  useProjectGraphNeighborsQuery: () => ({
    data: {
      node: {
        id: "demo",
        label: "Demo",
        nodeType: "source",
        path: "wiki/sources/demo.md",
      },
      neighbors: [],
    },
    isLoading: false,
  }),
}));

describe("operations actions", () => {
  it("imports, rescans, deletes, ingests, searches, retries, cancels, resolves, and saves settings", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/sources"]}>
          <Routes>
            <Route path="projects/:projectId/sources" element={<SourcesPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.type(screen.getByLabelText("File Name"), "new-note.md");
    await user.type(screen.getByLabelText("Markdown Content"), "# New Note");
    await user.click(screen.getByRole("button", { name: "Import Source" }));
    expect(importSource).toHaveBeenCalledWith({
      projectId: "project-1",
      fileName: "new-note.md",
      content: "# New Note",
    });

    await user.click(screen.getByRole("button", { name: "Rescan Sources" }));
    expect(rescanSources).toHaveBeenCalledWith({ projectId: "project-1" });

    await user.click(screen.getByRole("button", { name: "Ingest" }));
    expect(ingestSource).toHaveBeenCalledWith({
      projectId: "project-1",
      relativePath: "raw/sources/demo.md",
    });

    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(deleteSource).toHaveBeenCalledWith({
      projectId: "project-1",
      relativePath: "demo.md",
    });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/search"]}>
          <Routes>
            <Route path="projects/:projectId/search" element={<SearchPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.type(screen.getByLabelText("Search Query"), "demo");
    await user.click(screen.getByRole("button", { name: "Run Search" }));
    expect(runSearch).toHaveBeenCalledWith({
      projectId: "project-1",
      query: "demo",
    });
    expect(await screen.findByText("wiki/sources/demo.md")).toBeInTheDocument();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/graph"]}>
          <Routes>
            <Route path="projects/:projectId/graph" element={<GraphPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(screen.getByText("Nodes: 1")).toBeInTheDocument();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/tasks"]}>
          <Routes>
            <Route path="projects/:projectId/tasks" element={<TasksPage />} />
            <Route path="projects/:projectId/reviews" element={<ReviewsPage />} />
            <Route path="settings" element={<SettingsPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(retryTask).toHaveBeenCalledWith({ projectId: "project-1", taskId: "task-1" });
    expect(screen.getAllByText("raw/sources/demo.md").length).toBeGreaterThan(0);
    expect(screen.getByText(/"size": 42/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(cancelTask).toHaveBeenCalledWith({ projectId: "project-1", taskId: "task-1" });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/reviews"]}>
          <Routes>
            <Route path="projects/:projectId/reviews" element={<ReviewsPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Resolve" }));
    expect(updateReview).toHaveBeenCalledWith({
      projectId: "project-1",
      reviewId: "review-1",
      status: "resolved",
    });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/settings"]}>
          <Routes>
            <Route path="settings" element={<SettingsPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.clear(screen.getByLabelText("Default Query Limit"));
    await user.type(screen.getByLabelText("Default Query Limit"), "5");
    await user.click(screen.getByRole("button", { name: "Save Settings" }));
    expect(updateSettings).toHaveBeenCalledWith({
      providerMode: "deterministic",
      language: "en",
      defaultQueryLimit: 5,
    });
  });
});
