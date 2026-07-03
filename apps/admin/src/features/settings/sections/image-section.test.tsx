import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ImageSection } from "./image-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
}));

describe("ImageSection", () => {
  it("hydrates from settings and saves an image block including size", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      defaults: { language: "en", defaultQueryLimit: 8 },
      image: {
        baseUrl: "https://img.example.com",
        model: "gpt-image-1",
        size: "1024x1024",
        timeoutSeconds: 60,
        apiKeyConfigured: false,
      },
    });

    render(<ImageSection />);

    expect(screen.getByLabelText(/base url/i)).toHaveValue("https://img.example.com");
    expect(screen.getByLabelText(/model/i)).toHaveValue("gpt-image-1");
    expect(screen.getByLabelText(/image size/i)).toHaveValue("1024x1024");

    await user.selectOptions(screen.getByLabelText(/image size/i), "512x512");
    await user.click(screen.getByRole("button", { name: /save/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.image).toMatchObject({
      baseUrl: "https://img.example.com",
      model: "gpt-image-1",
      size: "512x512",
    });
    expect(payload.image).not.toHaveProperty("apiKey");
    expect(payload).toMatchObject({ providerMode: "openai-compatible", language: "en", defaultQueryLimit: 8 });
  });
});
