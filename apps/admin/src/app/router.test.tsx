import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { AppRoutes } from "./router";

const mockUseSession = vi.fn();
const mockUseProjectDetailQuery = vi.fn();

vi.mock("../features/auth/use-session", () => ({
  useSession: () => mockUseSession(),
}));

vi.mock("../features/projects/detail-queries", () => ({
  useProjectDetailQuery: (...args: unknown[]) => mockUseProjectDetailQuery(...args),
}));

vi.mock("../features/auth/login-page", () => ({
  LoginPage: () => <h1>Sign in</h1>,
}));

vi.mock("../features/dashboard/page", () => ({
  DashboardPage: () => <h1>Dashboard</h1>,
}));

vi.mock("../features/projects/page", () => ({
  ProjectsPage: () => <h1>Projects</h1>,
}));

vi.mock("../features/projects/detail-page", () => ({
  ProjectDetailPage: () => <h1>Project overview</h1>,
}));

vi.mock("../features/files/page", () => ({
  FilesPage: () => <h1>Files Page</h1>,
}));

vi.mock("../features/sources/page", () => ({
  SourcesPage: () => <h1>Sources Page</h1>,
}));

vi.mock("../features/source-watch/page", () => ({
  SourceWatchPage: () => <h1>Source Watch Page</h1>,
}));

vi.mock("../features/search/page", () => ({
  SearchPage: () => <h1>Search Page</h1>,
}));

vi.mock("../features/lint/page", () => ({
  LintPage: () => <h1>Lint Page</h1>,
}));

vi.mock("../features/graph/page", () => ({
  GraphPage: () => <h1>Graph Page</h1>,
}));

vi.mock("../features/tasks/page", () => ({
  TasksPage: () => <h1>Tasks Page</h1>,
}));

vi.mock("../features/reviews/page", () => ({
  ReviewsPage: () => <h1>Reviews Page</h1>,
}));

vi.mock("../features/audit/page", () => ({
  AuditPage: () => <h1>Audit Page</h1>,
}));

vi.mock("../features/users/page", () => ({
  UsersPage: () => <h1>Users</h1>,
}));

vi.mock("../features/settings/page", () => ({
  SettingsPage: () => <h1>Settings</h1>,
}));

describe("AppRoutes", () => {
  it("redirects unauthenticated users to /login", async () => {
    mockUseSession.mockReturnValue({ user: null, isLoading: false });
    mockUseProjectDetailQuery.mockReturnValue({
      data: null,
      error: null,
      isLoading: false,
    });

    const queryClient = new QueryClient();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/"]}>
          <AppRoutes />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(await screen.findByRole("heading", { name: /sign in/i })).toBeInTheDocument();
  });

  it("shows project workspace chrome on /projects/:projectId routes", async () => {
    const user = userEvent.setup();
    mockUseSession.mockReturnValue({
      user: { id: "user-1", username: "admin", role: "admin" },
      isLoading: false,
    });
    mockUseProjectDetailQuery.mockReturnValue({
      data: {
        project: {
          id: "project-1",
          name: "demo-project",
          rootPath: "E:/demo-project",
          createdAt: "2026-06-10T00:00:00Z",
          sourceCount: 2,
          taskCount: 3,
          reviewCount: 1,
        },
      },
      error: null,
      isLoading: false,
    });

    const queryClient = new QueryClient();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/projects/project-1/files"]}>
          <AppRoutes />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(await screen.findByRole("link", { name: /files/i })).toBeInTheDocument();
    expect(screen.getByText("demo-project")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Files Page" })).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Reviews" }));
    expect(await screen.findByRole("heading", { name: "Reviews Page" })).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Audit" }));
    expect(await screen.findByRole("heading", { name: "Audit Page" })).toBeInTheDocument();
  });
});
