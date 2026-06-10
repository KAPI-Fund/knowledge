import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

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

describe("ProjectsPage", () => {
  it("renders projects returned by the API hook", () => {
    const queryClient = new QueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <ProjectsPage />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(screen.getByRole("heading", { name: "Projects" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Create Project" })).toBeVisible();
    expect(screen.getByText("demo-project")).toBeVisible();
  });

  it("opens a create dialog with name only", () => {
    const queryClient = new QueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <ProjectsPage />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Create Project" }));

    expect(screen.getByRole("dialog")).toBeVisible();
    expect(screen.getByLabelText("Name")).toBeVisible();
    expect(screen.queryByLabelText("Root Path")).toBeNull();
  });
});
