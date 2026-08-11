import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { SettingsLayout } from "./layout";

function renderLayout(initialEntry = "/settings/llm") {
  return render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <Routes>
        <Route path="settings" element={<SettingsLayout />}>
          <Route path="llm" element={<div>LLM_SECTION</div>} />
          <Route path="image" element={<div>IMAGE_SECTION</div>} />
          <Route path="search" element={<div>WEB_SEARCH_SECTION</div>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe("SettingsLayout", () => {
  it("renders the header, section nav, and the active section outlet", () => {
    renderLayout();
    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Settings sections" })).toBeInTheDocument();
    expect(screen.getByText("LLM_SECTION")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Defaults" })).toBeInTheDocument();
  });

  it("switches sections when a nav link is clicked", async () => {
    const user = userEvent.setup();
    renderLayout();
    await user.click(screen.getByRole("link", { name: "Image" }));
    expect(screen.getByText("IMAGE_SECTION")).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: "Web Search" }));
    expect(screen.getByText("WEB_SEARCH_SECTION")).toBeInTheDocument();
  });
});
