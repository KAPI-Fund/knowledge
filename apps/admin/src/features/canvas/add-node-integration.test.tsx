import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

// Full integration for the user's exact action: real page + real toolbar +
// real board. Mocks only the data/network layer. Reproduces "click Note on a
// fresh empty canvas" and asserts a rendered node appears.

beforeAll(() => {
  class RO {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  (globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver = RO;
});

function emptyCanvas() {
  return {
    id: "c1",
    title: "Fresh",
    document: { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } },
  };
}

let currentParams: { canvasId?: string } = { canvasId: "c1" };
let canvasResult: { data: ReturnType<typeof emptyCanvas> | undefined } = { data: emptyCanvas() };

beforeEach(() => {
  currentParams = { canvasId: "c1" };
  canvasResult = { data: emptyCanvas() };
});

vi.mock("react-router-dom", () => ({ useParams: () => currentParams }));
vi.mock("./api", () => ({ extractUrl: vi.fn(), saveCanvas: vi.fn() }));
vi.mock("./stream", () => ({ runCanvasNode: vi.fn(), streamCanvasChat: vi.fn() }));
vi.mock("./history-sidebar", () => ({ HistorySidebar: () => <div>history</div> }));
vi.mock("./chat-panel", () => ({ ChatPanel: () => <div>chat</div> }));
vi.mock("./kb-picker", () => ({ KbProjectPicker: () => <div>kb-picker</div> }));
vi.mock("./queries", () => ({
  useCanvas: () => canvasResult,
  useSaveCanvas: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
  useCanvasCacheSave: () => vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../settings/queries", () => ({
  useSystemSettingsQuery: () => ({ data: { providerModel: "gpt-test" } }),
}));

import { CanvasPage } from "./page";

describe("add node (full integration)", () => {
  it("shows a rendered node after clicking Note on an empty canvas", async () => {
    const { container } = render(<CanvasPage />);
    await waitFor(() =>
      expect(container.querySelector(".react-flow")).toBeInTheDocument(),
    );
    expect(container.querySelectorAll(".react-flow__node").length).toBe(0);

    fireEvent.click(screen.getByRole("button", { name: /note/i }));

    await waitFor(() =>
      expect(container.querySelectorAll(".react-flow__node").length).toBe(1),
    );
  });
});
