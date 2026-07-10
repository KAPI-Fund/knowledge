import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";

import { GraphPage } from "../graph/page";
import { ReviewsPage } from "../reviews/page";
import { SearchPage } from "../search/page";
import { DefaultsSection } from "../settings/sections/defaults-section";
import { SourcesPage } from "../sources/page";
import { TasksPage } from "../tasks/page";

const retryTask = vi.fn();
const cancelTask = vi.fn();
const updateReview = vi.fn();
const sweepReviews = vi.fn();
const updateSettings = vi.fn();
const importSource = vi.fn();
const ingestSource = vi.fn();
const deleteSource = vi.fn();
const rescanSources = vi.fn();
const runSearch = vi.fn().mockResolvedValue({
  mode: "keyword",
  tokenHits: 1,
  vectorHits: 0,
  results: [
    {
      path: "wiki/sources/demo.md",
      title: "Demo",
      snippet: "Demo content",
      score: 1,
      images: [
        {
          url: "wiki/media/demo.png",
          alt: "Demo Diagram",
        },
      ],
      content: "# Demo",
    },
  ],
});
const graphQuerySpy = vi.fn();

vi.mock("../tasks/queries", () => ({
  useProjectTasksQuery: () => ({
    data: [
      {
        id: "task-1",
        title: "Imported note.md",
        taskType: "source_import",
        status: "queued",
        relativePath: "raw/sources/demo.md",
        attemptCount: 0,
        maxAttempts: 3,
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
        attemptCount: 0,
        maxAttempts: 3,
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
  useSweepReviewsMutation: () => ({
    mutateAsync: sweepReviews,
  }),
}));

vi.mock("../settings/queries", () => ({
  useSystemSettingsQuery: () => ({
    data: {
      providerMode: "deterministic",
      language: "en",
      defaultQueryLimit: 3,
      providerBaseUrl: "",
      providerApiKeyConfigured: false,
      providerModel: "",
      providerEmbeddingModel: "",
      providerTimeoutSeconds: 60,
      defaults: { language: "en", defaultQueryLimit: 3 },
    },
    isLoading: false,
  }),
  useUpdateSystemSettingsMutation: () => ({
    mutateAsync: updateSettings,
  }),
  useRunWebSearchMutation: () => ({
    mutateAsync: vi.fn(),
    reset: vi.fn(),
    isPending: false,
    data: undefined,
  }),
  useCreateConnectionMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useUpdateConnectionMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useDeleteConnectionMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useActivateConnectionMutation: () => ({ mutateAsync: vi.fn(), isPending: false }),
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

vi.mock("../graph/graph-canvas", () => ({
  GraphCanvas: ({ nodes }: { nodes: { id: string; label: string }[] }) => (
    <div data-testid="graph-canvas">
      {nodes.map((node) => (
        <span key={node.id}>{node.label}</span>
      ))}
    </div>
  ),
}));

vi.mock("../graph/starfield-canvas", () => ({
  StarfieldCanvas: ({ nodes }: { nodes: { id: string; label: string }[] }) => (
    <div data-testid="starfield-canvas">
      {nodes.map((node) => (
        <span key={node.id}>{node.label}</span>
      ))}
    </div>
  ),
}));

vi.mock("../graph/queries", () => ({
  useProjectGraphQuery: (...args: unknown[]) => {
    graphQuerySpy(...args);
    return {
      data: {
        nodes: [
          {
            id: "demo",
            label: "Demo",
            nodeType: "source",
            path: "wiki/sources/demo.md",
            linkCount: 2,
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
    };
  },
  useProjectGraphNeighborsQuery: () => ({
    data: {
      node: {
        id: "demo",
        label: "Demo",
        nodeType: "source",
        path: "wiki/sources/demo.md",
        linkCount: 2,
      },
      neighbors: [],
    },
    isLoading: false,
  }),
}));

afterEach(() => {
  vi.clearAllMocks();
});

describe("operations actions", () => {
  it("imports, rescans, deletes, ingests, searches, retries, cancels, sweeps reviews, resolves, and saves settings", async () => {
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

    await user.click(screen.getByRole("tab", { name: "Text Import" }));
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
    await user.click(await screen.findByRole("button", { name: "Delete source" }));
    expect(deleteSource).toHaveBeenCalledWith({
      projectId: "project-1",
      relativePath: "demo.md",
    });

    cleanup();
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
      topK: 10,
      includeContent: false,
    });
    expect(await screen.findByText("wiki/sources/demo.md")).toBeInTheDocument();
    expect(screen.getByText("Mode: keyword")).toBeInTheDocument();
    expect(screen.getByText("Demo Diagram")).toBeInTheDocument();
    expect(screen.getByText("wiki/media/demo.png")).toBeInTheDocument();

    cleanup();
    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/graph"]}>
          <Routes>
            <Route path="projects/:projectId/graph" element={<GraphPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(await screen.findByTestId("starfield-canvas")).toBeInTheDocument();
    expect(screen.getByText("Demo")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Search graph"), "demo");
    expect(graphQuerySpy).toHaveBeenLastCalledWith("project-1");

    cleanup();
    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/tasks"]}>
          <Routes>
            <Route path="projects/:projectId/tasks" element={<TasksPage />} />
            <Route path="projects/:projectId/reviews" element={<ReviewsPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(retryTask).toHaveBeenCalledWith({ projectId: "project-1", taskId: "task-1" });
    expect(screen.getAllByText("raw/sources/demo.md").length).toBeGreaterThan(0);
    expect(screen.getByText(/"size": 42/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await user.click(await screen.findByRole("button", { name: "Cancel task" }));
    expect(cancelTask).toHaveBeenCalledWith({ projectId: "project-1", taskId: "task-1" });

    cleanup();
    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/reviews"]}>
          <Routes>
            <Route path="projects/:projectId/reviews" element={<ReviewsPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Sweep Reviews" }));
    expect(sweepReviews).toHaveBeenCalledWith({
      projectId: "project-1",
    });

    await user.click(screen.getByRole("button", { name: "Resolve" }));
    expect(updateReview).toHaveBeenCalledWith({
      projectId: "project-1",
      reviewId: "review-1",
      status: "resolved",
    });

    cleanup();
    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/settings/defaults"]}>
          <Routes>
            <Route path="settings/defaults" element={<DefaultsSection />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.clear(screen.getByLabelText("Default Query Limit"));
    await user.type(screen.getByLabelText("Default Query Limit"), "5");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(updateSettings).toHaveBeenCalledWith({
      defaults: { language: "en", defaultQueryLimit: 5 },
    });
  });

  it("uploads binary files and preserves nested folder paths", async () => {
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

    await user.click(screen.getByRole("tab", { name: "File Upload" }));
    const binaryFile = new File([new Uint8Array([0, 1, 2, 255])], "slides.docx", {
      type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    });
    await user.upload(screen.getByLabelText("Files to Upload"), binaryFile);
    await user.click(screen.getByRole("button", { name: "Upload Files" }));

    expect(importSource).toHaveBeenCalledWith({
      projectId: "project-1",
      fileName: "slides.docx",
      contentBase64: "AAEC/w==",
    });

    await user.click(screen.getByRole("tab", { name: "Folder Import" }));
    const nestedFile = new File(["# Child\n"], "child.md", { type: "text/markdown" });
    const peerFile = new File(["# Peer\n"], "peer.md", { type: "text/markdown" });
    Object.defineProperty(nestedFile, "webkitRelativePath", {
      configurable: true,
      value: "team-a/child.md",
    });
    Object.defineProperty(peerFile, "webkitRelativePath", {
      configurable: true,
      value: "team-a/docs/peer.md",
    });

    await user.upload(screen.getByLabelText("Folder to Import"), [nestedFile, peerFile]);
    await user.click(screen.getByRole("button", { name: "Import Folder" }));

    expect(importSource).toHaveBeenNthCalledWith(2, {
      projectId: "project-1",
      fileName: "team-a/child.md",
      contentBase64: "IyBDaGlsZAo=",
    });
    expect(importSource).toHaveBeenNthCalledWith(3, {
      projectId: "project-1",
      fileName: "team-a/docs/peer.md",
      contentBase64: "IyBQZWVyCg==",
    });
  });
});
