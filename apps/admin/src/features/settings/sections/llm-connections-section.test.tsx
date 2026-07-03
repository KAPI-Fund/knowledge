import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { LlmConnectionsSection } from "./llm-connections-section";

const settingsData = vi.fn();
const createConnection = vi.fn();
const updateConnection = vi.fn();
const deleteConnection = vi.fn();
const activateConnection = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useCreateConnectionMutation: () => ({ mutateAsync: createConnection, isPending: false }),
  useUpdateConnectionMutation: () => ({ mutateAsync: updateConnection, isPending: false }),
  useDeleteConnectionMutation: () => ({ mutateAsync: deleteConnection, isPending: false }),
  useActivateConnectionMutation: () => ({ mutateAsync: activateConnection, isPending: false }),
}));

function twoConnections() {
  return {
    connections: [
      { id: "c1", label: "OpenAI", baseUrl: "https://api.openai.com", model: "gpt-4o", timeoutSeconds: 30, isActive: true, apiKeyConfigured: true },
      { id: "c2", label: "Local vLLM", baseUrl: "http://localhost:8000", model: "qwen2", timeoutSeconds: null, isActive: false, apiKeyConfigured: false },
    ],
  };
}

describe("LlmConnectionsSection", () => {
  it("lists each connection with its label, model, and active badge", () => {
    settingsData.mockReturnValue(twoConnections());
    render(<LlmConnectionsSection />);
    expect(screen.getByText("OpenAI")).toBeInTheDocument();
    expect(screen.getByText("Local vLLM")).toBeInTheDocument();
    expect(screen.getByText(/gpt-4o/)).toBeInTheDocument();
    expect(screen.getByText("Active")).toBeInTheDocument();
  });

  it("activates an inactive connection", async () => {
    const user = userEvent.setup();
    activateConnection.mockResolvedValue({});
    settingsData.mockReturnValue(twoConnections());
    render(<LlmConnectionsSection />);
    const row = screen.getByText("Local vLLM").closest("li") as HTMLElement;
    await user.click(within(row).getByRole("button", { name: /activate/i }));
    expect(activateConnection).toHaveBeenCalledWith("c2");
  });

  it("expands a row and updates the connection with a kept key", async () => {
    const user = userEvent.setup();
    updateConnection.mockResolvedValue({});
    settingsData.mockReturnValue(twoConnections());
    render(<LlmConnectionsSection />);
    const row = screen.getByText("OpenAI").closest("li") as HTMLElement;
    await user.click(within(row).getByRole("button", { name: /edit/i }));
    await user.clear(within(row).getByLabelText(/model/i));
    await user.type(within(row).getByLabelText(/model/i), "gpt-4o-mini");
    await user.click(within(row).getByRole("button", { name: /^save$/i }));
    expect(updateConnection).toHaveBeenCalledWith(
      expect.objectContaining({ id: "c1", model: "gpt-4o-mini" }),
    );
    // Blank key field → no apiKey sent.
    expect(updateConnection.mock.calls[0][0]).not.toHaveProperty("apiKey");
  });

  it("adds a draft connection and creates it", async () => {
    const user = userEvent.setup();
    createConnection.mockResolvedValue({});
    settingsData.mockReturnValue({ connections: [] });
    render(<LlmConnectionsSection />);
    await user.click(screen.getByRole("button", { name: /add/i }));
    await user.type(screen.getByLabelText(/label/i), "New");
    await user.type(screen.getByLabelText(/base url/i), "https://x");
    await user.type(screen.getByLabelText(/model/i), "m");
    await user.click(screen.getByRole("button", { name: /^create$/i }));
    expect(createConnection).toHaveBeenCalledWith(
      expect.objectContaining({ label: "New", baseUrl: "https://x", model: "m" }),
    );
  });

  it("deletes a connection after confirming", async () => {
    const user = userEvent.setup();
    deleteConnection.mockResolvedValue({});
    settingsData.mockReturnValue(twoConnections());
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<LlmConnectionsSection />);
    const row = screen.getByText("Local vLLM").closest("li") as HTMLElement;
    await user.click(within(row).getByRole("button", { name: /delete/i }));
    expect(deleteConnection).toHaveBeenCalledWith("c2");
  });
});
