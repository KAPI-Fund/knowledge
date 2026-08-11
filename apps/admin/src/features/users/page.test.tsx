import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { UsersPage } from "./page";

vi.mock("./queries", () => ({
  useUsersQuery: () => ({
    data: [
      {
        id: "user-1",
        username: "admin",
        role: "admin",
      },
      {
        id: "user-2",
        username: "editor",
        role: "member",
      },
    ],
    isLoading: false,
  }),
}));

describe("users page", () => {
  it("shows the workspace users and their roles", () => {
    const queryClient = new QueryClient();

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <UsersPage />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    expect(screen.getByRole("heading", { name: "Users" })).toBeInTheDocument();
    expect(screen.getAllByText("admin").length).toBeGreaterThan(1);
    expect(screen.getByText("editor")).toBeInTheDocument();
    expect(screen.getByText("member")).toBeInTheDocument();
    expect(screen.getByText("user-1")).toBeInTheDocument();
  });
});
