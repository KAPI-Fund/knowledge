import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { QueryPage } from "./page";

const mockCreateQueryTask = vi.fn();
const mockTaskDetail = vi.fn();
const mockSaveQueryTask = vi.fn();
const mockRetryTask = vi.fn();
const mockCancelTask = vi.fn();

vi.mock("./queries", () => ({
  useCreateQueryTaskMutation: () => ({
    mutateAsync: mockCreateQueryTask,
  }),
  useQueryTaskDetailQuery: () => ({
    data: mockTaskDetail(),
  }),
  useSaveQueryTaskMutation: () => ({
    mutateAsync: mockSaveQueryTask,
  }),
}));

vi.mock("../tasks/queries", () => ({
  useRetryTaskMutation: () => ({
    mutateAsync: mockRetryTask,
  }),
  useCancelTaskMutation: () => ({
    mutateAsync: mockCancelTask,
  }),
}));

describe("query page", () => {
  it("submits a query task and renders the completed answer", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();

    mockCreateQueryTask.mockResolvedValue({ taskId: "task-1", status: "queued" });
    mockTaskDetail
      .mockReturnValueOnce(undefined)
      .mockReturnValueOnce({
        id: "task-1",
        status: "succeeded",
        result: {
          answer: "Attention focuses on relevant tokens.",
          citations: [
            {
              path: "wiki/concepts/attention.md",
              title: "Attention",
              snippet: "relevant tokens",
              score: 1,
            },
          ],
        },
      });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/query"]}>
          <Routes>
            <Route path="projects/:projectId/query" element={<QueryPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.type(screen.getByLabelText("Query"), "What is attention?");
    await user.click(screen.getByRole("button", { name: "Run Query" }));

    expect(mockCreateQueryTask).toHaveBeenCalledWith({
      projectId: "project-1",
      query: "What is attention?",
      topK: 3,
    });
  });

  it("saves a successful query answer back into the wiki", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();

    mockTaskDetail.mockReturnValue({
      id: "task-1",
      title: "Query: What is attention?",
      status: "succeeded",
      result: {
        answer: "Attention focuses computation on relevant tokens.",
        citations: [
          {
            path: "wiki/concepts/attention.md",
            title: "Attention",
            snippet: "relevant tokens",
            score: 1,
          },
        ],
      },
    });
    mockSaveQueryTask.mockResolvedValue({ taskId: "task-2", status: "queued" });

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/query"]}>
          <Routes>
            <Route path="projects/:projectId/query" element={<QueryPage />} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Save To Wiki" }));

    expect(mockSaveQueryTask).toHaveBeenCalledWith({
      projectId: "project-1",
      taskId: "task-1",
      title: "What is attention?",
    });
  });
});
