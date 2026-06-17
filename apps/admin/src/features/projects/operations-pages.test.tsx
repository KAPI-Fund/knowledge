import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
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

import { ProjectDetailPage } from "./detail-page";

vi.mock("./detail-queries", () => ({
  useProjectDetailQuery: () => ({
    data: {
      project: {
        id: "project-1",
        name: "demo-project",
        rootPath: "E:/demo-project",
        createdAt: "2026-06-04T00:00:00Z",
        sourceCount: 1,
        taskCount: 1,
        reviewCount: 1,
      },
    },
    isLoading: false,
  }),
}));

vi.mock("../sources/queries", () => ({
  useProjectSourcesQuery: () => ({
    data: [{ relativePath: "raw/sources/demo.md", size: 42 }],
    isLoading: false,
  }),
  useImportSourceMutation: () => ({
    mutateAsync: vi.fn(),
    isPending: false,
  }),
  useIngestSourceMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useRescanSourcesMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useDeleteSourceMutation: () => ({
    mutateAsync: vi.fn(),
  }),
}));

vi.mock("../reviews/queries", () => ({
  useProjectReviewsQuery: () => ({
    data: [
      {
        id: "review-1",
        status: "open",
        title: "Review demo.md",
        type: "missing-page",
        description: "Create a dedicated page for evaluation details.",
        sourcePath: "raw/sources/demo.md",
        affectedPages: ["wiki/evaluation.md"],
        searchQueries: ["demo evaluation", "demo benchmarks"],
        options: [
          { label: "Approve", action: "Approve" },
          { label: "Skip", action: "Skip" },
        ],
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

vi.mock("../files/queries", () => ({
  useProjectFilesQuery: () => ({
    data: {
      root: "all",
      truncated: false,
      files: [
        {
          name: "wiki",
          path: "wiki",
          isDir: true,
          children: [
            {
              name: "evaluation.md",
              path: "wiki/evaluation.md",
              isDir: false,
              size: 64,
            },
          ],
        },
        {
          name: "raw",
          path: "raw",
          isDir: true,
          children: [
            {
              name: "sources",
              path: "raw/sources",
              isDir: true,
              children: [
                {
                  name: "demo.md",
                  path: "raw/sources/demo.md",
                  isDir: false,
                  size: 32,
                },
              ],
            },
          ],
        },
      ],
    },
    isLoading: false,
  }),
  useProjectFileContentQuery: (_projectId: string, path: string) => ({
    data: {
      path,
      content: `preview:${path}`,
    },
    isLoading: false,
  }),
  useSaveFileContentMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useDeleteWikiPagesMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

vi.mock("../audit/queries", () => ({
  useProjectAuditLogsQuery: () => ({
    data: [{ id: "audit-1", action: "project.created", summary: "Created project demo-project" }],
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
  useRunWebSearchMutation: () => ({
    mutateAsync: vi.fn(),
    isPending: false,
    data: undefined,
  }),
}));

vi.mock("../search/queries", () => ({
  useProjectSearchMutation: () => ({
    mutateAsync: vi.fn().mockResolvedValue([
      {
        path: "wiki/sources/demo.md",
        title: "Demo",
        snippet: "Demo content",
        score: 1,
      },
    ]),
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
          sources: [],
        },
      ],
      edges: [
        {
          source: "demo",
          target: "peer",
          weight: 1,
        },
      ],
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

function renderProjectRoute(initialEntry: string) {
  const queryClient = new QueryClient();

  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[initialEntry]}>
        <Routes>
          <Route path="/" element={<AppShell />}>
            <Route path="projects/:projectId" element={<ProjectWorkspaceLayout />}>
              <Route index element={<ProjectDetailPage />} />
              <Route path="files" element={<FilesPage />} />
              <Route path="sources" element={<SourcesPage />} />
              <Route path="source-watch" element={<SourceWatchPage />} />
              <Route path="search" element={<SearchPage />} />
              <Route path="graph" element={<GraphPage />} />
              <Route path="reviews" element={<ReviewsPage />} />
              <Route path="audit" element={<AuditPage />} />
            </Route>
            <Route path="settings" element={<SettingsPage />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function workspaceProjectNav() {
  return screen.getAllByRole("navigation", { name: "Project navigation" })[0];
}

describe("project operation pages", () => {
  it("navigates to files, sources, search, graph, reviews, audit, and settings pages", async () => {
    const user = userEvent.setup();

    renderProjectRoute("/projects/project-1");

    expect(await screen.findByRole("heading", { name: "demo-project" })).toBeInTheDocument();
    expect(within(workspaceProjectNav()).getByRole("tab", { name: "Files" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Overview" })).toBeInTheDocument();

    await user.click(within(workspaceProjectNav()).getByRole("tab", { name: "Files" }));
    expect(await screen.findByRole("heading", { name: "Files" })).toBeInTheDocument();
    expect(screen.getAllByText("demo.md").length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: "demo.md" }));
    expect(screen.getByText("preview:raw/sources/demo.md")).toBeInTheDocument();

    await user.click(within(workspaceProjectNav()).getByRole("tab", { name: "Sources" }));
    expect(await screen.findByRole("heading", { name: "Sources" })).toBeInTheDocument();
    expect(screen.getByText("raw/sources/demo.md")).toBeInTheDocument();

    await user.click(within(workspaceProjectNav()).getByRole("tab", { name: "Source Watch" }));
    expect(await screen.findByRole("heading", { name: "Source Watch" })).toBeInTheDocument();
    expect(screen.getByDisplayValue("E:/watched-sources")).toBeInTheDocument();
    expect(screen.getByLabelText(/automatically enqueue ingest tasks/i)).toBeInTheDocument();

    await user.click(within(workspaceProjectNav()).getByRole("tab", { name: "Search" }));
    expect(await screen.findByRole("heading", { name: "Search" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run Search" })).toBeInTheDocument();

    await user.click(within(workspaceProjectNav()).getByRole("tab", { name: "Graph" }));
    expect(await screen.findByRole("heading", { name: "Graph" })).toBeInTheDocument();
    expect(screen.getByText("Demo")).toBeInTheDocument();

    await user.click(within(workspaceProjectNav()).getByRole("tab", { name: "Reviews" }));
    expect(await screen.findByRole("heading", { name: "Reviews" })).toBeInTheDocument();
    expect(screen.getByText("Review demo.md")).toBeInTheDocument();
    expect(screen.getByText("missing-page")).toBeInTheDocument();
    expect(screen.getByText("Create a dedicated page for evaluation details.")).toBeInTheDocument();
    expect(screen.getByText("raw/sources/demo.md")).toBeInTheDocument();
    expect(screen.getByText("wiki/evaluation.md")).toBeInTheDocument();
    expect(screen.getByText("demo evaluation")).toBeInTheDocument();
    expect(screen.getByText("Approve")).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: "wiki/evaluation.md" }));
    expect(await screen.findByRole("heading", { name: "Files" })).toBeInTheDocument();
    expect(screen.getByText("preview:wiki/evaluation.md")).toBeInTheDocument();

    await user.click(within(workspaceProjectNav()).getByRole("tab", { name: "Audit" }));
    expect(await screen.findByRole("heading", { name: "Audit" })).toBeInTheDocument();
    expect(screen.getByText("Created project demo-project")).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Settings" }));
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByDisplayValue("deterministic")).toBeInTheDocument();
  });
});
