import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { LintPage } from "./page";

const mockCreateLintTask = vi.fn();
const mockTaskDetail = vi.fn();

vi.mock("./queries", () => ({
  useCreateLintTaskMutation: () => ({
    mutateAsync: mockCreateLintTask,
  }),
  useLintTaskDetailQuery: () => ({
    data: mockTaskDetail(),
  }),
}));

describe("lint page", () => {
  it("runs structural lint and renders returned issues", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();

    mockCreateLintTask.mockResolvedValue({ taskId: "task-1", status: "queued" });
    mockTaskDetail
      .mockReturnValueOnce(undefined)
      .mockReturnValueOnce({
        id: "task-1",
        status: "succeeded",
        result: {
          mode: "structural",
          issues: [
            {
              issueType: "broken-link",
              severity: "warning",
              page: "concepts/attention.md",
              detail: "Broken link: [[missing-page]] - target page not found.",
            },
          ],
        },
      });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/lint"]}>
          <Routes>
            <Route path="projects/:projectId/lint" element={<LintPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Run Structural Lint" }));

    expect(mockCreateLintTask).toHaveBeenCalledWith({
      projectId: "project-1",
      mode: "structural",
    });
    expect(await screen.findByText("broken-link")).toBeInTheDocument();
    expect(screen.getByText("wiki/concepts/attention.md")).toBeInTheDocument();
    expect(screen.getByText("Broken link: [[missing-page]] - target page not found.")).toBeInTheDocument();
  });

  it("runs semantic lint and renders semantic issues", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();

    mockCreateLintTask.mockResolvedValue({ taskId: "task-2", status: "queued" });
    mockTaskDetail
      .mockReturnValueOnce(undefined)
      .mockReturnValueOnce({
        id: "task-2",
        status: "succeeded",
        result: {
          mode: "semantic",
          issues: [
            {
              issueType: "semantic",
              severity: "warning",
              page: "Conflicting attention claims",
              detail: "[contradiction] Two pages describe attention with conflicting scope.",
              affectedPages: ["concepts/attention.md", "concepts/attention-mechanism.md"],
            },
          ],
        },
      });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/lint"]}>
          <Routes>
            <Route path="projects/:projectId/lint" element={<LintPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Run Semantic Lint" }));

    expect(mockCreateLintTask).toHaveBeenCalledWith({
      projectId: "project-1",
      mode: "semantic",
    });
    expect(await screen.findByRole("heading", { name: "semantic" })).toBeInTheDocument();
    expect(screen.getByText("[contradiction] Two pages describe attention with conflicting scope.")).toBeInTheDocument();
    expect(screen.getByText("wiki/concepts/attention.md")).toBeInTheDocument();
    expect(screen.getByText("wiki/concepts/attention-mechanism.md")).toBeInTheDocument();
  });
});
