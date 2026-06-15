import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { DeepResearchPage } from "./page";

const mockCreate = vi.fn();
const mockTasks = vi.fn();

vi.mock("./queries", () => ({
  useCreateDeepResearchTaskMutation: () => ({
    mutateAsync: mockCreate,
    isPending: false,
  }),
}));

vi.mock("../tasks/queries", () => ({
  useProjectTasksQuery: () => ({ data: mockTasks(), isLoading: false }),
}));

function renderPage() {
  const queryClient = new QueryClient();
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/projects/project-1/deep-research"]}>
        <Routes>
          <Route path="projects/:projectId/deep-research" element={<DeepResearchPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("deep research page", () => {
  it("submits topic and parsed queries", async () => {
    const user = userEvent.setup();
    mockTasks.mockReturnValue([]);
    mockCreate.mockResolvedValue({ taskId: "task-1", status: "queued" });

    renderPage();

    await user.type(screen.getByLabelText("Topic"), "Knowledge Graphs");
    await user.type(screen.getByLabelText("Search Queries"), "knowledge graphs, rag knowledge graph");
    await user.click(screen.getByRole("button", { name: "Start Research" }));

    expect(mockCreate).toHaveBeenCalledWith({
      projectId: "project-1",
      topic: "Knowledge Graphs",
      searchQueries: ["knowledge graphs", "rag knowledge graph"],
    });
  });

  it("lists recent deep research tasks", () => {
    mockTasks.mockReturnValue([
      {
        id: "task-1",
        taskType: "project.deep_research",
        title: "Deep research: KG",
        status: "succeeded",
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:01:00Z",
      },
      {
        id: "task-2",
        taskType: "project.ingest_source",
        title: "Ingest",
        status: "succeeded",
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:00:30Z",
      },
    ]);
    renderPage();
    expect(screen.getByText("Deep research: KG")).toBeInTheDocument();
    expect(screen.queryByText("Ingest")).not.toBeInTheDocument();
  });

  it("renders the saved path and source count for a succeeded task", () => {
    mockTasks.mockReturnValue([
      {
        id: "task-1",
        taskType: "project.deep_research",
        title: "Deep research: KG",
        status: "succeeded",
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:01:00Z",
        result: {
          savedPath: "wiki/queries/research-kg.md",
          sourceCount: 3,
          errors: [],
        },
      },
    ]);
    renderPage();
    expect(screen.getByText("wiki/queries/research-kg.md")).toBeInTheDocument();
    expect(screen.getByText("Sources used: 3")).toBeInTheDocument();
  });

  it("renders the error message for a failed task", () => {
    mockTasks.mockReturnValue([
      {
        id: "task-1",
        taskType: "project.deep_research",
        title: "Deep research: KG",
        status: "failed",
        createdAt: "2026-06-15T00:00:00Z",
        updatedAt: "2026-06-15T00:01:00Z",
        error: { message: "no sources found" },
      },
    ]);
    renderPage();
    expect(screen.getByText(/Error: no sources found/)).toBeInTheDocument();
  });
});
