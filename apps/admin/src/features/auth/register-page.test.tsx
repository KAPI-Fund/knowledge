import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { RegisterPage } from "./register-page";

const mockRegister = vi.fn();

vi.mock("./api", () => ({
  useRegisterMutation: () => ({
    mutateAsync: mockRegister,
    isPending: false,
  }),
}));

function renderRegister(queryClient: QueryClient) {
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <RegisterPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("RegisterPage", () => {
  beforeEach(() => {
    mockRegister.mockReset();
    mockRegister.mockResolvedValue({ csrfToken: "csrf-token" });
  });

  it("submits username and password when the confirmation matches", async () => {
    const user = userEvent.setup();
    renderRegister(new QueryClient());

    await user.type(screen.getByLabelText(/username/i), "newcomer");
    await user.type(screen.getByLabelText(/^password$/i), "hunter2-pass");
    await user.type(screen.getByLabelText(/confirm password/i), "hunter2-pass");
    await user.click(screen.getByRole("button", { name: /create account/i }));

    expect(mockRegister).toHaveBeenCalledWith({
      username: "newcomer",
      password: "hunter2-pass",
    });
  });

  it("blocks submission and shows an error when passwords do not match", async () => {
    const user = userEvent.setup();
    renderRegister(new QueryClient());

    await user.type(screen.getByLabelText(/username/i), "newcomer");
    await user.type(screen.getByLabelText(/^password$/i), "hunter2-pass");
    await user.type(screen.getByLabelText(/confirm password/i), "different-pass");
    await user.click(screen.getByRole("button", { name: /create account/i }));

    expect(await screen.findByText(/passwords do not match/i)).toBeInTheDocument();
    expect(mockRegister).not.toHaveBeenCalled();
  });

  it("clears the stale session cache after a successful registration", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
    queryClient.setQueryData(["session"], { user: null });
    renderRegister(queryClient);

    await user.type(screen.getByLabelText(/username/i), "newcomer");
    await user.type(screen.getByLabelText(/^password$/i), "hunter2-pass");
    await user.type(screen.getByLabelText(/confirm password/i), "hunter2-pass");
    await user.click(screen.getByRole("button", { name: /create account/i }));

    expect(queryClient.getQueryData(["session"])).toBeUndefined();
  });

  it("shows an error message when registration fails", async () => {
    mockRegister.mockRejectedValueOnce(new Error("username already taken"));
    const user = userEvent.setup();
    renderRegister(new QueryClient());

    await user.type(screen.getByLabelText(/username/i), "admin");
    await user.type(screen.getByLabelText(/^password$/i), "hunter2-pass");
    await user.type(screen.getByLabelText(/confirm password/i), "hunter2-pass");
    await user.click(screen.getByRole("button", { name: /create account/i }));

    expect(await screen.findByText(/username already taken/i)).toBeInTheDocument();
  });
});
