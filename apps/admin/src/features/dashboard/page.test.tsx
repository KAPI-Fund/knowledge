import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { DashboardPage } from "./page";

vi.mock("../projects/queries", () => ({
  useProjectsQuery: () => ({
    data: [
      {
        id: "project-1",
        name: "demo-project",
        rootPath: "E:/demo-project",
        createdAt: "2026-06-09T00:00:00Z",
      },
      {
        id: "project-2",
        name: "research-notes",
        rootPath: "E:/research-notes",
        createdAt: "2026-06-08T00:00:00Z",
      },
    ],
    isLoading: false,
  }),
}));

vi.mock("../settings/queries", () => ({
  useSystemSettingsQuery: () => ({
    data: {
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 5,
    },
    isLoading: false,
  }),
}));

describe("dashboard page", () => {
  it("summarizes the workspace and exposes project shortcuts", () => {
    const queryClient = new QueryClient();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <DashboardPage />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(screen.getByRole("heading", { name: "Dashboard" })).toBeInTheDocument();
    expect(screen.getByText("2 projects")).toBeInTheDocument();
    expect(screen.getByText("openai-compatible")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "demo-project" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "research-notes" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Projects" })).toBeInTheDocument();
  });
});
