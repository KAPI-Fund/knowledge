import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { AppShell } from "../../components/layout/app-shell";
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
              <Route path="projects/:projectId" element={<ProjectDetailPage />} />
              <Route path="projects/:projectId/tasks" element={<TasksPage />} />
            </Route>
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(await screen.findByRole("link", { name: "demo-project" }));

    expect(await screen.findByRole("heading", { name: "demo-project" })).toBeInTheDocument();
    expect(screen.getByText("2 sources")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Tasks" })).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Tasks" }));
    expect(await screen.findByRole("heading", { name: "Tasks" })).toBeInTheDocument();
    expect(screen.getAllByText("Imported note.md").length).toBeGreaterThan(0);
  });
});
