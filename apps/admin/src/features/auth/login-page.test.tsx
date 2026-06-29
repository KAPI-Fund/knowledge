import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { LoginPage } from "./login-page";

const mockLogin = vi.fn();

vi.mock("./api", () => ({
  useLoginMutation: () => ({
    mutateAsync: mockLogin,
    isPending: false,
  }),
}));

function renderLogin(queryClient: QueryClient) {
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <LoginPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("LoginPage", () => {
  beforeEach(() => {
    mockLogin.mockReset();
    mockLogin.mockResolvedValue({ csrfToken: "csrf-token" });
  });

  it("submits username and password to the login mutation", async () => {
    const user = userEvent.setup();
    renderLogin(new QueryClient());

    await user.type(screen.getByLabelText(/username/i), "admin");
    await user.type(screen.getByLabelText(/password/i), "secret-password");
    await user.click(screen.getByRole("button", { name: /sign in/i }));

    expect(mockLogin).toHaveBeenCalledWith({
      username: "admin",
      password: "secret-password",
    });
  });

  it("clears the stale session cache after a successful login", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
    queryClient.setQueryData(["session"], { user: null });
    renderLogin(queryClient);

    await user.type(screen.getByLabelText(/username/i), "admin");
    await user.type(screen.getByLabelText(/password/i), "secret-password");
    await user.click(screen.getByRole("button", { name: /sign in/i }));

    expect(queryClient.getQueryData(["session"])).toBeUndefined();
  });

  it("shows an error message when login fails", async () => {
    mockLogin.mockRejectedValueOnce(new Error("invalid credentials"));
    const user = userEvent.setup();
    renderLogin(new QueryClient());

    await user.type(screen.getByLabelText(/username/i), "admin");
    await user.type(screen.getByLabelText(/password/i), "wrong-password");
    await user.click(screen.getByRole("button", { name: /sign in/i }));

    expect(await screen.findByText(/invalid credentials/i)).toBeInTheDocument();
  });
});
