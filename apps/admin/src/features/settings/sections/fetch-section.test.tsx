import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { FetchSection } from "./fetch-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
}));

function firecrawlActive() {
  return {
    fetch: {
      provider: "firecrawl",
      providers: {
        firecrawl: { apiKeyConfigured: true, baseUrl: "https://api.firecrawl.dev" },
      },
    },
  };
}

describe("FetchSection", () => {
  it("defaults to the active firecrawl provider and renders its fields", () => {
    settingsData.mockReturnValue(firecrawlActive());
    render(<FetchSection />);
    expect(screen.getByRole("combobox", { name: /Fetch Provider/i })).toHaveTextContent(
      "firecrawl",
    );
    expect(screen.getByLabelText(/Firecrawl Base URL/i)).toHaveValue("https://api.firecrawl.dev");
  });

  it("saves a fetch block; blank key sends empty string (keep) and edited baseUrl", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue(firecrawlActive());
    render(<FetchSection />);

    await user.clear(screen.getByLabelText(/Firecrawl Base URL/i));
    await user.type(screen.getByLabelText(/Firecrawl Base URL/i), "http://host:3002");
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.fetch.provider).toBe("firecrawl");
    expect(payload.fetch.providers.firecrawl.baseUrl).toBe("http://host:3002");
    // Blank key box → empty string means "keep the stored key" server-side.
    expect(payload.fetch.providers.firecrawl.apiKey).toBe("");
    expect(Object.keys(payload)).toEqual(["fetch"]);
  });

  it("toggling clear-key sends apiKey null to remove the stored key", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue(firecrawlActive());
    render(<FetchSection />);

    await user.click(screen.getByLabelText(/Clear stored Firecrawl key/i));
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.fetch.providers.firecrawl.apiKey).toBeNull();
  });

  it("blanking the base URL sends null to revert it to the default", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue(firecrawlActive());
    render(<FetchSection />);

    await user.clear(screen.getByLabelText(/Firecrawl Base URL/i));
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.fetch.providers.firecrawl.baseUrl).toBeNull();
  });

  it("selecting none omits the firecrawl credential fields", async () => {
    const user = userEvent.setup();
    settingsData.mockReturnValue(firecrawlActive());
    render(<FetchSection />);

    await user.click(screen.getByRole("combobox", { name: /Fetch Provider/i }));
    await user.click(await screen.findByRole("option", { name: "none" }));
    expect(screen.queryByLabelText(/Firecrawl Base URL/i)).not.toBeInTheDocument();
  });
});
