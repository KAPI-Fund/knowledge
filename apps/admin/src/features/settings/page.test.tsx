import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SettingsPage } from "./page";

const updateSettings = vi.fn();

vi.mock("./queries", () => ({
  useSystemSettingsQuery: () => ({
    data: {
      providerMode: "openai-compatible",
      language: "en",
      defaultQueryLimit: 5,
      providerBaseUrl: "https://backend.intelalloc.com",
      providerApiKeyConfigured: true,
      providerModel: "gpt-5.4",
      providerEmbeddingModel: "",
      providerTimeoutSeconds: 60,
    },
    isLoading: false,
  }),
  useUpdateSystemSettingsMutation: () => ({
    mutateAsync: updateSettings,
  }),
}));

describe("SettingsPage", () => {
  it("omits providerApiKey when the field is blank", async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient();
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
});
