import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { SidebarProvider } from "@/components/ui/sidebar";

import { AppSidebar } from "./app-sidebar";

const mockUseSession = vi.fn();
const mockUseProjectDetailQuery = vi.fn();
const mockUseSpacesQuery = vi.fn();

vi.mock("@/features/auth/use-session", () => ({
  useSession: () => mockUseSession(),
}));
vi.mock("@/features/projects/detail-queries", () => ({
  useProjectDetailQuery: (...args: unknown[]) => mockUseProjectDetailQuery(...args),
}));
vi.mock("@/features/spaces/use-spaces", () => ({
  useSpacesQuery: () => mockUseSpacesQuery(),
}));

function renderSidebar(path: string) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <SidebarProvider>
          <AppSidebar />
        </SidebarProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("AppSidebar", () => {
  it("renders global navigation outside a project", () => {
    mockUseSession.mockReturnValue({ user: { id: "u1", username: "admin", role: "admin" }, isLoading: false });
    mockUseProjectDetailQuery.mockReturnValue({ data: null, error: null, isLoading: false });
    mockUseSpacesQuery.mockReturnValue({ data: { orgs: [] }, isLoading: false });

    renderSidebar("/");

    expect(screen.getByRole("link", { name: "Dashboard" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Projects" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /admin/i })).toBeInTheDocument();
  });

  it("renders grouped project navigation inside a project", () => {
    mockUseSession.mockReturnValue({ user: { id: "u1", username: "admin", role: "admin" }, isLoading: false });
    mockUseProjectDetailQuery.mockReturnValue({
      data: { project: { id: "project-1", name: "demo-project", rootPath: "E:/demo" } },
      error: null,
      isLoading: false,
    });
    mockUseSpacesQuery.mockReturnValue({ data: { orgs: [] }, isLoading: false });

    renderSidebar("/projects/project-1/tasks");

    expect(screen.getByText("demo-project")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Files" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Tasks" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /all projects/i })).toBeInTheDocument();
  });
});

describe("AppSidebar create-org entry", () => {
  it("opens the create-organization dialog from the space switcher", async () => {
    mockUseSession.mockReturnValue({ user: { id: "u1", username: "admin", role: "operator" }, isLoading: false });
    mockUseProjectDetailQuery.mockReturnValue({ data: null, error: null, isLoading: false });
    mockUseSpacesQuery.mockReturnValue({ data: { orgs: [] }, isLoading: false });

    const user = userEvent.setup();
    renderSidebar("/projects");

    await user.click(screen.getByRole("button", { name: /knowledge/i }));
    await user.click(await screen.findByText("New organization"));

    expect(await screen.findByRole("heading", { name: "New organization" })).toBeInTheDocument();
  });
});
