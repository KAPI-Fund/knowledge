import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { WebSearchSection } from "./web-search-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();
const runWebSearch = vi.fn();
const resetWebSearch = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
  useRunWebSearchMutation: () => ({
    mutateAsync: runWebSearch,
    reset: resetWebSearch,
    isPending: false,
    data: undefined,
  }),
}));

function tavilyActive() {
  return {
    providerMode: "openai-compatible",
    defaults: { language: "en", defaultQueryLimit: 8 },
    search: {
      provider: "tavily",
      providers: {
        tavily: { apiKeyConfigured: true, baseUrl: "https://api.tavily.com" },
        serpapi: { apiKeyConfigured: false, engine: "google", baseUrl: "https://serpapi.com" },
        searxng: { url: "", categories: ["general"] },
        ollama: { apiKeyConfigured: false, url: "https://ollama.com" },
      },
    },
  };
}

describe("WebSearchSection", () => {
  it("defaults to the active tavily provider and renders its fields", () => {
    settingsData.mockReturnValue(tavilyActive());
    render(<WebSearchSection />);
    expect(screen.getByLabelText(/Search Provider/i)).toHaveValue("tavily");
    expect(screen.getByLabelText(/Tavily Base URL/i)).toHaveValue("https://api.tavily.com");
  });

  it("saves a search block; blank tavily key sends empty string (keep) and edited baseUrl", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue(tavilyActive());
    render(<WebSearchSection />);

    await user.clear(screen.getByLabelText(/Tavily Base URL/i));
    await user.type(screen.getByLabelText(/Tavily Base URL/i), "https://tavily.local");
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.search.provider).toBe("tavily");
    expect(payload.search.providers.tavily.baseUrl).toBe("https://tavily.local");
    // Blank key box → empty string means "keep the stored key" server-side.
    expect(payload.search.providers.tavily.apiKey).toBe("");
    expect(payload).toMatchObject({ providerMode: "openai-compatible", language: "en", defaultQueryLimit: 8 });
  });

  it("switching provider keeps each provider's fields in the payload", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue(tavilyActive());
    render(<WebSearchSection />);

    await user.selectOptions(screen.getByLabelText(/Search Provider/i), "searxng");
    await user.type(screen.getByLabelText(/SearXNG Instance URL/i), "https://searx.local");
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.search.provider).toBe("searxng");
    expect(payload.search.providers.searxng.url).toBe("https://searx.local");
    // Tavily's baseUrl is still carried so switching does not drop it.
    expect(payload.search.providers.tavily.baseUrl).toBe("https://api.tavily.com");
  });

  it("toggling clear-key sends apiKey null to remove the stored key", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue(tavilyActive());
    render(<WebSearchSection />);

    await user.click(screen.getByLabelText(/Clear stored Tavily key/i));
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.search.providers.tavily.apiKey).toBeNull();
  });

  it("blanking a base URL sends null to revert it to the default", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue(tavilyActive());
    render(<WebSearchSection />);

    await user.clear(screen.getByLabelText(/Tavily Base URL/i));
    await user.click(screen.getByRole("button", { name: /^save$/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.search.providers.tavily.baseUrl).toBeNull();
  });

  it("runs a test search with the typed query", async () => {
    const user = userEvent.setup();
    runWebSearch.mockResolvedValue({ results: [] });
    settingsData.mockReturnValue(tavilyActive());
    render(<WebSearchSection />);

    await user.type(screen.getByLabelText(/Test Query/i), "hello world");
    await user.click(screen.getByRole("button", { name: /Test Search/i }));
    expect(runWebSearch).toHaveBeenCalledWith({ query: "hello world" });
  });
});
