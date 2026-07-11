import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

import { McpSection } from "./mcp-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
}));

function renderSection() {
  render(
    <MemoryRouter>
      <McpSection />
    </MemoryRouter>,
  );
}

describe("McpSection", () => {
  it("enables MCP access via the switch", async () => {
    const user = userEvent.setup();
    settingsData.mockReturnValue({ mcp: { enabled: false } });
    updateSettings.mockResolvedValue({});
    renderSection();

    expect(screen.queryByText(/Claude Desktop configuration/i)).not.toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: /Enable MCP access/i }));
    expect(updateSettings).toHaveBeenCalledWith({ mcp: { enabled: true } });
  });

  it("shows endpoint and config snippets when enabled, and can disable", async () => {
    const user = userEvent.setup();
    settingsData.mockReturnValue({ mcp: { enabled: true } });
    updateSettings.mockResolvedValue({});
    renderSection();

    const endpoint = `${window.location.origin}/api/mcp`;
    expect(screen.getByText(endpoint)).toBeInTheDocument();
    expect(screen.getByText(/Claude Desktop configuration/i)).toBeInTheDocument();
    expect(screen.getByText(new RegExp(`claude mcp add --transport http knowledge`))).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /API tokens/i })).toHaveAttribute(
      "href",
      "/api-tokens",
    );

    await user.click(screen.getByRole("switch", { name: /Enable MCP access/i }));
    expect(updateSettings).toHaveBeenCalledWith({ mcp: { enabled: false } });
  });
});
