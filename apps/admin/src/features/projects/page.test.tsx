import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
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
        <ProjectsPage />
      </QueryClientProvider>,
    );

    expect(screen.getByRole("heading", { name: "Projects" })).toBeVisible();
    expect(screen.getByText("demo-project")).toBeVisible();
  });
});
