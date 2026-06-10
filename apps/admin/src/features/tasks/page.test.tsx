import { ApiClientError } from "@knowledge/api-client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { AppShell } from "../../components/layout/app-shell";
import { ProjectWorkspaceLayout } from "../../components/layout/project-workspace-layout";

import { TasksPage } from "./page";

vi.mock("../projects/detail-queries", () => ({
  useProjectDetailQuery: () => ({
    data: {
      project: {
        id: "project-1",
        name: "demo-project",
        rootPath: "E:/demo-project",
        createdAt: "2026-06-04T00:00:00Z",
        sourceCount: 1,
        taskCount: 1,
        reviewCount: 0,
      },
    },
    isLoading: false,
  }),
}));

vi.mock("./queries", () => ({
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
      },
    ],
    isLoading: false,
  }),
  useTaskDetailQuery: () => ({
    error: new ApiClientError(400, null, "unknown task"),
  }),
  useRetryTaskMutation: () => ({
    mutateAsync: vi.fn(),
  }),
  useCancelTaskMutation: () => ({
    mutateAsync: vi.fn(),
  }),
}));

describe("tasks page", () => {
  it("shows a local not-found message when the selected task disappears", async () => {
    const queryClient = new QueryClient();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/tasks"]}>
          <Routes>
            <Route path="/" element={<AppShell />}>
              <Route path="projects/:projectId" element={<ProjectWorkspaceLayout />}>
                <Route path="tasks" element={<TasksPage />} />
              </Route>
            </Route>
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(await screen.findByRole("heading", { name: "demo-project" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Tasks" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Task no longer exists" })).toBeInTheDocument();
    expect(screen.getByText(/the selected task disappeared or no longer exists/i)).toBeInTheDocument();
  });
});
