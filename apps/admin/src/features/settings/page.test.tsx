import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SettingsPage } from "./page";

const updateSettings = vi.fn();
const runWebSearch = vi.fn();
const resetWebSearch = vi.fn();
const settingsData = vi.fn();

vi.mock("./queries", () => ({
  useSystemSettingsQuery: () => ({
    data: settingsData(),
    isLoading: false,
  }),
  useUpdateSystemSettingsMutation: () => ({
    mutateAsync: updateSettings,
  }),
  useRunWebSearchMutation: () => ({
    mutateAsync: runWebSearch,
    reset: resetWebSearch,
    isPending: false,
    data: undefined,
  }),
}));

describe("SettingsPage", () => {
  it("omits providerApiKey when the field is blank", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 5,
      providerBaseUrl: "https://backend.intelalloc.com",
      providerApiKeyConfigured: true,
      providerModel: "gpt-5.4",
      providerEmbeddingModel: "",
      providerTimeoutSeconds: 60,
      searchProvider: "none",
      searchApiKeyConfigured: false,
      serpapiEngine: "google",
      searxngUrl: null,
      searxngCategories: ["general"],
      ollamaSearchUrl: null,
    });
    updateSettings.mockResolvedValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 5,
      providerBaseUrl: "https://backend.intelalloc.com",
      providerApiKeyConfigured: true,
      providerModel: "gpt-5.4",
      providerEmbeddingModel: "",
      providerTimeoutSeconds: 60,
    });

    render(
      <QueryClientProvider client={queryClient}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Save Settings" }));

    expect(updateSettings).toHaveBeenCalledWith(
      expect.not.objectContaining({
        providerApiKey: "",
      }),
    );
  });

  it("sends clearProviderApiKey when remove key is clicked then saved", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 25,
      providerBaseUrl: null,
      providerApiKeyConfigured: true,
      providerModel: null,
      providerEmbeddingModel: null,
      providerTimeoutSeconds: 30,
      searchProvider: "none",
      searchApiKeyConfigured: false,
      serpapiEngine: "google",
      searxngUrl: null,
      searxngCategories: ["general"],
      ollamaSearchUrl: null,
      tavilyBaseUrl: null,
      serpapiBaseUrl: null,
    });
    updateSettings.mockResolvedValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 25,
      providerApiKeyConfigured: false,
    });

    render(
      <QueryClientProvider client={queryClient}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: /remove configured key/i }));
    expect(screen.getByText("Key will be removed on save")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Save Settings" }));
    expect(updateSettings).toHaveBeenCalledWith(
      expect.objectContaining({ clearProviderApiKey: true }),
    );
  });

  it("a typed replacement key cancels a pending clear", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 25,
      providerBaseUrl: null,
      providerApiKeyConfigured: true,
      providerModel: null,
      providerEmbeddingModel: null,
      providerTimeoutSeconds: 30,
      searchProvider: "none",
      searchApiKeyConfigured: false,
      serpapiEngine: "google",
      searxngUrl: null,
      searxngCategories: ["general"],
      ollamaSearchUrl: null,
      tavilyBaseUrl: null,
      serpapiBaseUrl: null,
    });
    updateSettings.mockClear();
    updateSettings.mockResolvedValue({ providerApiKeyConfigured: true });

    render(
      <QueryClientProvider client={queryClient}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: /remove configured key/i }));
    expect(screen.getByText("Key will be removed on save")).toBeInTheDocument();

    await user.type(
      screen.getByPlaceholderText("Leave blank to keep the current key"),
      "new-key",
    );
    await user.click(screen.getByRole("button", { name: "Save Settings" }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload).toMatchObject({ providerApiKey: "new-key" });
    expect(payload).not.toHaveProperty("clearProviderApiKey");
  });

  it("undo cancels a pending clear without sending the flag", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 25,
      providerBaseUrl: null,
      providerApiKeyConfigured: true,
      providerModel: null,
      providerEmbeddingModel: null,
      providerTimeoutSeconds: 30,
      searchProvider: "none",
      searchApiKeyConfigured: false,
      serpapiEngine: "google",
      searxngUrl: null,
      searxngCategories: ["general"],
      ollamaSearchUrl: null,
      tavilyBaseUrl: null,
      serpapiBaseUrl: null,
    });
    updateSettings.mockClear();
    updateSettings.mockResolvedValue({ providerApiKeyConfigured: true });

    render(
      <QueryClientProvider client={queryClient}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: /remove configured key/i }));
    expect(screen.getByText("Key will be removed on save")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^undo$/i }));
    expect(
      screen.getByRole("button", { name: /remove configured key/i }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Save Settings" }));
    expect(updateSettings.mock.calls[0][0]).not.toHaveProperty("clearProviderApiKey");
  });

  it("sends clearSearchApiKey when the search remove key is clicked then saved", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 25,
      providerBaseUrl: null,
      providerApiKeyConfigured: false,
      providerModel: null,
      providerEmbeddingModel: null,
      providerTimeoutSeconds: 30,
      searchProvider: "tavily",
      searchApiKeyConfigured: true,
      serpapiEngine: "google",
      searxngUrl: null,
      searxngCategories: ["general"],
      ollamaSearchUrl: null,
      tavilyBaseUrl: null,
      serpapiBaseUrl: null,
    });
    updateSettings.mockClear();
    updateSettings.mockResolvedValue({ searchApiKeyConfigured: false });

    render(
      <QueryClientProvider client={queryClient}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button", { name: /remove configured key/i }));
    expect(screen.getByText("Key will be removed on save")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Save Settings" }));
    expect(updateSettings).toHaveBeenCalledWith(
      expect.objectContaining({ clearSearchApiKey: true }),
    );
  });
});

describe("settings page web search section", () => {
  it("renders the search provider fields and Test Search button", () => {
    const queryClient = new QueryClient();
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 25,
      providerBaseUrl: null,
      providerApiKeyConfigured: false,
      providerModel: null,
      providerEmbeddingModel: null,
      providerTimeoutSeconds: 30,
      searchProvider: "searxng",
      searchApiKeyConfigured: false,
      serpapiEngine: "google",
      searxngUrl: null,
      searxngCategories: ["general"],
      ollamaSearchUrl: null,
    });

    render(
      <QueryClientProvider client={queryClient}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    expect(screen.getByLabelText(/Search Provider/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/SearXNG Instance URL/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Test Search/i })).toBeInTheDocument();
  });

  it("calls runWebSearch with the typed query when Test Search is clicked", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 25,
      providerBaseUrl: null,
      providerApiKeyConfigured: false,
      providerModel: null,
      providerEmbeddingModel: null,
      providerTimeoutSeconds: 30,
      searchProvider: "searxng",
      searchApiKeyConfigured: false,
      serpapiEngine: "google",
      searxngUrl: "http://127.0.0.1:18080",
      searxngCategories: ["general"],
      ollamaSearchUrl: null,
    });
    runWebSearch.mockResolvedValue({ results: [] });

    render(
      <QueryClientProvider client={queryClient}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    await user.type(screen.getByLabelText(/Test Query/i), "hello world");
    await user.click(screen.getByRole("button", { name: /Test Search/i }));

    expect(runWebSearch).toHaveBeenCalledWith({ query: "hello world" });
  });

  it("calls reset when the form becomes dirty", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 25,
      providerBaseUrl: null,
      providerApiKeyConfigured: false,
      providerModel: null,
      providerEmbeddingModel: null,
      providerTimeoutSeconds: 30,
      searchProvider: "none",
      searchApiKeyConfigured: false,
      serpapiEngine: "google",
      searxngUrl: null,
      searxngCategories: ["general"],
      ollamaSearchUrl: null,
      tavilyBaseUrl: null,
      serpapiBaseUrl: null,
    });

    render(
      <QueryClientProvider client={queryClient}>
        <SettingsPage />
      </QueryClientProvider>,
    );

    resetWebSearch.mockClear();

    await user.type(
      screen.getByPlaceholderText("Leave blank to keep the current key"),
      "x",
    );

    expect(resetWebSearch).toHaveBeenCalled();
  });
});
