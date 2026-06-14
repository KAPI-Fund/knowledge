import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { ApiTokensPage } from "./page";

const mockTokensList = vi.fn();
const mockCreateToken = vi.fn();
const mockRevokeToken = vi.fn();

vi.mock("./queries", () => ({
  useApiTokensQuery: () => ({ data: mockTokensList() }),
  useCreateApiTokenMutation: () => ({ mutateAsync: mockCreateToken, isPending: false }),
  useRevokeApiTokenMutation: () => ({ mutateAsync: mockRevokeToken, isPending: false }),
}));

function renderPage() {
  const queryClient = new QueryClient();
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/api-tokens"]}>
        <ApiTokensPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("api tokens page", () => {
  it("mints a token and shows the plaintext once", async () => {
    const user = userEvent.setup();
    mockTokensList.mockReturnValue({ tokens: [] });
    mockCreateToken.mockResolvedValue({
      id: "token-1",
      name: "test",
      projectId: null,
      prefix: "abcd1234",
      createdAt: "2026-06-13T00:00:00Z",
      token: "plaintext-token-once",
    });

    renderPage();

    await user.type(screen.getByLabelText("Token Name"), "test");
    await user.click(screen.getByRole("button", { name: "Mint Token" }));

    expect(mockCreateToken).toHaveBeenCalledWith({ name: "test", projectId: null });
    expect(await screen.findByText("plaintext-token-once")).toBeInTheDocument();
    expect(
      screen.getByText(/Copy this token now — it will not be shown again/i),
    ).toBeInTheDocument();
  });

  it("lists existing tokens and revokes them", async () => {
    const user = userEvent.setup();
    mockTokensList.mockReturnValue({
      tokens: [
        {
          id: "token-1",
          name: "ci-bot",
          projectId: null,
          prefix: "abcd1234",
          lastUsedAt: "2026-06-13T00:00:00Z",
          revokedAt: null,
          createdAt: "2026-06-12T00:00:00Z",
        },
      ],
    });
    mockRevokeToken.mockResolvedValue({ revoked: true });

    renderPage();

    expect(screen.getByText("ci-bot")).toBeInTheDocument();
    expect(screen.getByText("abcd1234…")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Revoke" }));

    expect(mockRevokeToken).toHaveBeenCalledWith({ tokenId: "token-1" });
  });

  it("displays revoked tokens as revoked and hides their revoke button", () => {
    mockTokensList.mockReturnValue({
      tokens: [
        {
          id: "token-1",
          name: "old",
          projectId: null,
          prefix: "abcd1234",
          lastUsedAt: null,
          revokedAt: "2026-06-13T00:00:00Z",
          createdAt: "2026-06-12T00:00:00Z",
        },
      ],
    });

    renderPage();

    expect(screen.getByText("revoked")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Revoke" })).not.toBeInTheDocument();
  });
});
