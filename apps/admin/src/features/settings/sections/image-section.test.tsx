import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { allowedSizesForModel, ImageSection } from "./image-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
}));

describe("allowedSizesForModel", () => {
  it("returns the gpt-image family sizes", () => {
    expect(allowedSizesForModel("gpt-image-1")).toEqual(["1024x1024", "1024x1536", "1536x1024"]);
  });

  it("returns the dall-e-3 family sizes", () => {
    expect(allowedSizesForModel("dall-e-3")).toEqual(["1024x1024", "1024x1792", "1792x1024"]);
  });

  it("returns the dall-e-2 family sizes", () => {
    expect(allowedSizesForModel("dall-e-2")).toEqual(["256x256", "512x512", "1024x1024"]);
  });

  it("falls back to all sizes for an unknown model", () => {
    expect(allowedSizesForModel("custom-model")).toEqual([
      "256x256",
      "512x512",
      "1024x1024",
      "1024x1536",
      "1536x1024",
      "1024x1792",
      "1792x1024",
    ]);
  });
});

describe("ImageSection", () => {
  it("hydrates from settings and saves an image-only block including size", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
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

    await user.selectOptions(screen.getByLabelText(/image size/i), "1536x1024");
    await user.click(screen.getByRole("button", { name: /save/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload).toEqual({
      image: {
        baseUrl: "https://img.example.com",
        model: "gpt-image-1",
        size: "1536x1024",
        timeoutSeconds: 60,
      },
    });
    expect(payload.image).not.toHaveProperty("apiKey");
  });

  it("updates the size options and resets an out-of-family size when the model changes", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      defaults: { language: "en", defaultQueryLimit: 8 },
      image: {
        baseUrl: "https://img.example.com",
        model: "dall-e-2",
        size: "512x512",
        timeoutSeconds: 60,
        apiKeyConfigured: false,
      },
    });

    render(<ImageSection />);

    // dall-e-2 allows 512x512.
    expect(screen.getByLabelText(/image size/i)).toHaveValue("512x512");

    // Switch to gpt-image-1, which does not allow 512x512 -> resets to its first size.
    await user.clear(screen.getByLabelText(/model/i));
    await user.type(screen.getByLabelText(/model/i), "gpt-image-1");

    expect(screen.getByLabelText(/image size/i)).toHaveValue("1024x1024");
    expect(screen.queryByRole("option", { name: "512x512" })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: "1536x1024" })).toBeInTheDocument();
  });
});
