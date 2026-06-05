import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { LoginPage } from "./login-page";

const mockLogin = vi.fn();

vi.mock("./api", () => ({
  useLoginMutation: () => ({
    mutateAsync: mockLogin.mockResolvedValue({ csrfToken: "csrf-token" }),
  }),
}));

describe("LoginPage", () => {
  it("submits username and password to the login mutation", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <LoginPage />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    await user.type(screen.getByLabelText(/username/i), "admin");
    await user.type(screen.getByLabelText(/password/i), "secret-password");
    await user.click(screen.getByRole("button", { name: /sign in/i }));

    expect(mockLogin).toHaveBeenCalledWith({
      username: "admin",
      password: "secret-password",
    });
  });
});
