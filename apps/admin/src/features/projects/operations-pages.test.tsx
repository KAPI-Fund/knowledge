import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { AppShell } from "../../components/layout/app-shell";
import { AuditPage } from "../audit/page";
import { GraphPage } from "../graph/page";
import { ReviewsPage } from "../reviews/page";
import { SearchPage } from "../search/page";
import { SettingsPage } from "../settings/page";
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

describe("project operation pages", () => {
  it("navigates to sources, search, graph, reviews, audit, and settings pages", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1"]}>
          <Routes>
            <Route path="/" element={<AppShell />}>
              <Route path="projects/:projectId" element={<ProjectDetailPage />} />
              <Route path="projects/:projectId/sources" element={<SourcesPage />} />
              <Route path="projects/:projectId/search" element={<SearchPage />} />
              <Route path="projects/:projectId/graph" element={<GraphPage />} />
              <Route path="projects/:projectId/reviews" element={<ReviewsPage />} />
              <Route path="projects/:projectId/audit" element={<AuditPage />} />
              <Route path="settings" element={<SettingsPage />} />
            </Route>
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("link", { name: "Sources" }));
    expect(await screen.findByRole("heading", { name: "Sources" })).toBeInTheDocument();
    expect(screen.getByText("raw/sources/demo.md")).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Search" }));
    expect(await screen.findByRole("heading", { name: "Search" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run Search" })).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Graph" }));
    expect(await screen.findByRole("heading", { name: "Graph" })).toBeInTheDocument();
    expect(screen.getByText("Demo")).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Reviews" }));
    expect(await screen.findByRole("heading", { name: "Reviews" })).toBeInTheDocument();
    expect(screen.getByText("Review demo.md")).toBeInTheDocument();
    expect(screen.getByText("missing-page")).toBeInTheDocument();
    expect(screen.getByText("Create a dedicated page for evaluation details.")).toBeInTheDocument();
    expect(screen.getByText("raw/sources/demo.md")).toBeInTheDocument();
    expect(screen.getByText("wiki/evaluation.md")).toBeInTheDocument();
    expect(screen.getByText("demo evaluation")).toBeInTheDocument();
    expect(screen.getByText("Approve")).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Audit" }));
    expect(await screen.findByRole("heading", { name: "Audit" })).toBeInTheDocument();
    expect(screen.getByText("Created project demo-project")).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Settings" }));
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByDisplayValue("deterministic")).toBeInTheDocument();
  });
});
