import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { EmbeddingSection } from "./embedding-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
}));

describe("EmbeddingSection", () => {
  it("hydrates fields from settings and saves an embedding block with kept key", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      defaults: { language: "en", defaultQueryLimit: 8 },
      embedding: {
        enabled: true,
        baseUrl: "https://emb.example.com",
        model: "text-embedding-3-small",
        timeoutSeconds: 60,
        apiKeyConfigured: true,
      },
    });

    render(<EmbeddingSection />);

    expect(screen.getByLabelText(/base url/i)).toHaveValue("https://emb.example.com");
    await user.clear(screen.getByLabelText(/model/i));
    await user.type(screen.getByLabelText(/model/i), "text-embedding-3-large");
    await user.click(screen.getByRole("button", { name: /save/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.embedding).toMatchObject({
      enabled: true,
      baseUrl: "https://emb.example.com",
      model: "text-embedding-3-large",
    });
    expect(payload.embedding).not.toHaveProperty("apiKey");
    expect(Object.keys(payload)).toEqual(["embedding"]);
  });

  it("toggles enabled off and includes it in the block", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      defaults: { language: "en", defaultQueryLimit: 8 },
      embedding: { enabled: true, baseUrl: "", model: "", timeoutSeconds: null, apiKeyConfigured: false },
    });

    render(<EmbeddingSection />);
    await user.click(screen.getByRole("switch", { name: /enable embedding/i }));
    await user.click(screen.getByRole("button", { name: /save/i }));

    expect(updateSettings.mock.calls[0][0].embedding.enabled).toBe(false);
  });
});
