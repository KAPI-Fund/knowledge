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
  it("hydrates from defaults block and saves a defaults-only payload", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      defaults: { language: "en", defaultQueryLimit: 5 },
    });

    render(<DefaultsSection />);

    expect(screen.getByLabelText(/language/i)).toHaveValue("en");
    await user.clear(screen.getByLabelText(/default query limit/i));
    await user.type(screen.getByLabelText(/default query limit/i), "12");
    await user.click(screen.getByRole("button", { name: /save/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload).toEqual({ defaults: { language: "en", defaultQueryLimit: 12 } });
  });

  it("keeps the saved query limit when the box is cleared before saving", async () => {
    const user = userEvent.setup();
    updateSettings.mockResolvedValue({});
    settingsData.mockReturnValue({
      defaults: { language: "en", defaultQueryLimit: 25 },
    });

    render(<DefaultsSection />);

    // Clearing the box would previously send Number("") === 0; it must snap back to
    // the currently-saved value (25, not a hardcoded 5) and echo that into the field.
    await user.clear(screen.getByLabelText(/default query limit/i));
    await user.click(screen.getByRole("button", { name: /save/i }));

    const payload = updateSettings.mock.calls[0][0];
    expect(payload).toEqual({ defaults: { language: "en", defaultQueryLimit: 25 } });
    expect(screen.getByLabelText(/default query limit/i)).toHaveValue(25);
  });
});
