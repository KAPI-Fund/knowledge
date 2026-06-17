import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { AppShell } from "../../components/layout/app-shell";
import { ProjectWorkspaceLayout } from "../../components/layout/project-workspace-layout";
import { AuditPage } from "../audit/page";
import { FilesPage } from "../files/page";
import { GraphPage } from "../graph/page";
import { ReviewsPage } from "../reviews/page";
import { SearchPage } from "../search/page";
import { SettingsPage } from "../settings/page";
import { SourceWatchPage } from "../source-watch/page";
import { SourcesPage } from "../sources/page";
import { TasksPage } from "../tasks/page";

import { ProjectDetailPage } from "./detail-page";
import { ProjectsPage } from "./page";

vi.mock("./queries", () => ({
  useProjectsQuery: () => ({
    data: [
      {
        id: "project-1",
        name: "demo-project",
        rootPath: "E:/demo-project",
        createdAt: "2026-06-04T00:00:00Z",
      },
    ],
    isLoading: false,
  }),
}));

vi.mock("./detail-queries", () => ({
  useProjectDetailQuery: () => ({
    data: {
      project: {
        id: "project-1",
        name: "demo-project",
        rootPath: "E:/demo-project",
        createdAt: "2026-06-04T00:00:00Z",
        sourceCount: 2,
        taskCount: 3,
        reviewCount: 1,
      },
    },
    isLoading: false,
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
}));

vi.mock("../tasks/queries", () => ({
  useProjectTasksQuery: () => ({
    data: [
      {
        id: "task-1",
        title: "Imported note.md",
        taskType: "source_import",
        status: "completed",
        relativePath: "raw/sources/note.md",
        detail: {},
        createdAt: "2026-06-04T00:00:00Z",
        updatedAt: "2026-06-04T00:00:00Z",
      },
    ],
    isLoading: false,
  }),
  useTaskDetailQuery: () => ({
    data: {
      id: "task-1",
      title: "Imported note.md",
      taskType: "source_import",
      status: "completed",
      relativePath: "raw/sources/note.md",
      detail: {},
      createdAt: "2026-06-04T00:00:00Z",
      updatedAt: "2026-06-04T00:00:00Z",
    },
    isLoading: false,
  }),
  useRetryTaskMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useCancelTaskMutation: () => ({
    mutateAsync: vi.fn(),
  }),
}));

vi.mock("../reviews/queries", () => ({
  useProjectReviewsQuery: () => ({
    data: [
      {
        id: "review-1",
        status: "open",
        type: "missing-page",
        title: "Review demo.md",
        description: "Create a dedicated page for evaluation details.",
        sourcePath: "raw/sources/demo.md",
        affectedPages: ["wiki/evaluation.md"],
        searchQueries: ["demo evaluation"],
        options: [{ label: "Approve", action: "approve" }],
      },
    ],
    isLoading: false,
  }),
  useUpdateReviewMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useSweepReviewsMutation: () => ({
    mutateAsync: vi.fn(),
  }),
}));

vi.mock("../audit/queries", () => ({
  useProjectAuditLogsQuery: () => ({
    data: [
      {
        id: "audit-1",
        action: "project.created",
        summary: "Created project demo-project",
      },
    ],
    isLoading: false,
  }),
}));

vi.mock("../source-watch/queries", () => ({
  useProjectSourceWatchQuery: () => ({
    data: {
      enabled: true,
      autoIngest: true,
      path: "E:/watched-sources",
      includeExtensions: ["md"],
      excludeExtensions: ["tmp"],
      excludeDirs: [".git"],
      excludeGlobs: ["~$*"],
      maxFileSizeMb: 100,
      intervalMinutes: 5,
      lastScanAt: "2026-06-09T00:00:00Z",
    },
    isLoading: false,
  }),
  useUpdateProjectSourceWatchMutation: () => ({
    mutateAsync: vi.fn(),
    isPending: false,
  }),
  useScanProjectSourceWatchMutation: () => ({
    mutateAsync: vi.fn(),
    isPending: false,
  }),
}));

vi.mock("../search/queries", () => ({
  useProjectSearchMutation: () => ({
    mutateAsync: vi.fn(),
  }),
}));

vi.mock("../graph/graph-canvas", () => ({
  GraphCanvas: ({ nodes }: { nodes: { id: string; label: string }[] }) => (
    <div data-testid="graph-canvas">
      {nodes.map((node) => (
        <span key={node.id}>{node.label}</span>
      ))}
    </div>
  ),
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
          linkCount: 1,
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
        linkCount: 1,
      },
      neighbors: [],
    },
    isLoading: false,
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
    mutateAsync: vi.fn(),
  }),
}));

describe("project operations routing", () => {
  it("navigates from the project list into project operations pages", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects"]}>
          <Routes>
            <Route path="/" element={<AppShell />}>
              <Route path="projects" element={<ProjectsPage />} />
              <Route path="projects/:projectId" element={<ProjectWorkspaceLayout />}>
                <Route index element={<ProjectDetailPage />} />
                <Route path="files" element={<FilesPage />} />
                <Route path="sources" element={<SourcesPage />} />
                <Route path="source-watch" element={<SourceWatchPage />} />
                <Route path="search" element={<SearchPage />} />
                <Route path="graph" element={<GraphPage />} />
                <Route path="tasks" element={<TasksPage />} />
                <Route path="reviews" element={<ReviewsPage />} />
                <Route path="audit" element={<AuditPage />} />
              </Route>
              <Route path="settings" element={<SettingsPage />} />
            </Route>
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(await screen.findByRole("link", { name: "demo-project" }));

    expect(await screen.findByRole("heading", { name: "demo-project" })).toBeInTheDocument();
    expect(screen.getByText("2 sources")).toBeInTheDocument();
    expect(screen.getByText("3 tasks")).toBeInTheDocument();
    expect(screen.getByText("1 reviews")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Overview" })).toBeInTheDocument();
    expect(screen.getByText("Recent Sources")).toBeInTheDocument();
    expect(screen.getByText("raw/sources/demo.md")).toBeInTheDocument();
    expect(screen.getByText("Recent Tasks")).toBeInTheDocument();
    expect(screen.getByText("Imported note.md")).toBeInTheDocument();
    expect(screen.getByText("Recent Reviews")).toBeInTheDocument();
    expect(screen.getByText("Review demo.md")).toBeInTheDocument();
    expect(screen.getByText("Recent Audit")).toBeInTheDocument();
    expect(screen.getByText("Created project demo-project")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Tasks" })).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Tasks" }));
    expect(await screen.findByRole("heading", { name: "Tasks" })).toBeInTheDocument();
    expect(screen.getAllByText("Imported note.md").length).toBeGreaterThan(0);
  });
});
