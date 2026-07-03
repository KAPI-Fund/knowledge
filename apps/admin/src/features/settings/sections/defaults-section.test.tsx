import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { DefaultsSection } from "./defaults-section";

const settingsData = vi.fn();
const updateSettings = vi.fn();

vi.mock("../queries", () => ({
  useSystemSettingsQuery: () => ({ data: settingsData(), isLoading: false }),
  useUpdateSystemSettingsMutation: () => ({ mutateAsync: updateSettings, isPending: false }),
}));

describe("DefaultsSection", () => {
  it("hydrates from defaults block and saves both defaults + top-level", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      providerMode: "openai-compatible",
      defaults: { language: "en", defaultQueryLimit: 5 },
    });

    render(<DefaultsSection />);

    expect(screen.getByLabelText(/language/i)).toHaveValue("en");
    await user.clear(screen.getByLabelText(/default query limit/i));
    await user.type(screen.getByLabelText(/default query limit/i), "12");
    await user.click(screen.getByRole("button", { name: /save/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload.defaults).toMatchObject({ language: "en", defaultQueryLimit: 12 });
    expect(payload).toMatchObject({ language: "en", defaultQueryLimit: 12, providerMode: "openai-compatible" });
  });
});
