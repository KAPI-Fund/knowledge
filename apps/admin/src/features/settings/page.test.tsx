import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SettingsPage } from "./page";

// Sections are exercised in their own tests; here we only assert the shell wires
// the nav to section switching. Stub each section with a marker.
vi.mock("./sections/llm-connections-section", () => ({
  LlmConnectionsSection: () => <div>LLM_SECTION</div>,
}));
vi.mock("./sections/embedding-section", () => ({
  EmbeddingSection: () => <div>EMBEDDING_SECTION</div>,
}));
vi.mock("./sections/image-section", () => ({
  ImageSection: () => <div>IMAGE_SECTION</div>,
}));
vi.mock("./sections/web-search-section", () => ({
  WebSearchSection: () => <div>WEB_SEARCH_SECTION</div>,
}));
vi.mock("./sections/defaults-section", () => ({
  DefaultsSection: () => <div>DEFAULTS_SECTION</div>,
}));

function renderPage() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <SettingsPage />
    </QueryClientProvider>,
  );
}

describe("SettingsPage shell", () => {
  it("shows the LLM section by default", () => {
    renderPage();
    expect(screen.getByText("LLM_SECTION")).toBeInTheDocument();
  });

  it("switches sections when a nav item is clicked", async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(screen.getByRole("button", { name: /image/i }));
    expect(screen.getByText("IMAGE_SECTION")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /web search/i }));
    expect(screen.getByText("WEB_SEARCH_SECTION")).toBeInTheDocument();
  });
});
