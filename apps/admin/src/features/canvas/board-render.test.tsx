import { render } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

vi.mock("../settings/queries", () => ({
  useSystemSettingsQuery: () => ({
    data: {
      providerMode: "openai-compatible",
      connections: [
        { id: "c1", label: "Active", baseUrl: "u", model: "analyze-model", timeoutSeconds: null, isActive: true, apiKeyConfigured: true },
      ],
      image: { baseUrl: "u", model: "image-model", size: "1024x1024", timeoutSeconds: null, apiKeyConfigured: true },
    },
  }),
}));

import { CanvasBoard } from "./canvas-board";
import type { CanvasDocument } from "./types";

beforeAll(() => {
  class RO {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  // React Flow needs ResizeObserver to measure the pane/nodes.
  (globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver = RO;
});

function docWith(n: number): CanvasDocument {
  return {
    nodes: Array.from({ length: n }, (_, i) => ({
      id: `note-${i}`,
      type: "note" as const,
      x: 80 + i * 40,
      y: 80 + i * 40,
      w: 280,
      h: 160,
      data: { markdown: `hello ${i}` },
    })),
    edges: [],
    viewport: { x: 0, y: 0, zoom: 1 },
  };
}

describe("CanvasBoard rendering", () => {
  it("renders a react-flow node for each document node", () => {
    const { container } = render(
      <CanvasBoard
        document={docWith(1)}
        onChange={vi.fn()}
        onRunNode={vi.fn()}
        onFetchUrl={vi.fn()}
        onSearchNode={vi.fn()}
      />,
    );
    expect(container.querySelectorAll(".react-flow__node").length).toBe(1);
  });

  it("renders nodes visible without waiting on async measurement", () => {
    // jsdom's ResizeObserver is a no-op and there is no layout engine, so React
    // Flow never records measured dimensions. React Flow gates node visibility on
    // nodeHasDimensions (measured ?? width ?? initialWidth); if the board does not
    // hand React Flow the node's width/height, the node stays `visibility: hidden`
    // forever -- which is exactly what users saw in the real browser, because our
    // controlled onNodesChange discards the measured dimensions on every change.
    const { container } = render(
      <CanvasBoard
        document={docWith(1)}
        onChange={vi.fn()}
        onRunNode={vi.fn()}
        onFetchUrl={vi.fn()}
        onSearchNode={vi.fn()}
      />,
    );
    const node = container.querySelector<HTMLElement>(".react-flow__node");
    expect(node).not.toBeNull();
    expect(node?.style.visibility).not.toBe("hidden");
  });

  it("renders source and target handles so edges can be drawn by hand", () => {
    // A pure producer (note: source only) plus a consumer (ai_analyze: source +
    // target). Without both handle kinds users could only get edges from /analyze
    // auto-wiring, never by dragging between nodes.
    const doc: CanvasDocument = {
      nodes: [
        { id: "n1", type: "note", x: 0, y: 0, w: 280, h: 160, data: {} },
        { id: "a1", type: "ai_analyze", x: 400, y: 0, w: 360, h: 320, data: {} },
      ],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    };
    const { container } = render(
      <CanvasBoard
        document={doc}
        onChange={vi.fn()}
        onRunNode={vi.fn()}
        onFetchUrl={vi.fn()}
        onSearchNode={vi.fn()}
      />,
    );
    // note contributes 1 (source), ai_analyze contributes 2 (source + target) = 3.
    expect(container.querySelectorAll(".react-flow__handle").length).toBeGreaterThanOrEqual(3);
  });

  it("renders the added node after the document prop grows", () => {
    const { container, rerender } = render(
      <CanvasBoard
        document={docWith(1)}
        onChange={vi.fn()}
        onRunNode={vi.fn()}
        onFetchUrl={vi.fn()}
        onSearchNode={vi.fn()}
      />,
    );
    expect(container.querySelectorAll(".react-flow__node").length).toBe(1);

    rerender(
      <CanvasBoard
        document={docWith(2)}
        onChange={vi.fn()}
        onRunNode={vi.fn()}
        onFetchUrl={vi.fn()}
        onSearchNode={vi.fn()}
      />,
    );
    expect(container.querySelectorAll(".react-flow__node").length).toBe(2);
  });

  it("shows the active-connection model on ai_analyze and image model on ai_image", () => {
    const doc: CanvasDocument = {
      nodes: [
        { id: "a1", type: "ai_analyze", x: 0, y: 0, w: 360, h: 320, data: {} },
        { id: "i1", type: "ai_image", x: 0, y: 0, w: 320, h: 400, data: {} },
      ],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    };
    const { getByText } = render(
      <CanvasBoard document={doc} onChange={vi.fn()} onRunNode={vi.fn()} onFetchUrl={vi.fn()} onSearchNode={vi.fn()} />,
    );
    expect(getByText("analyze-model")).toBeInTheDocument();
    expect(getByText("image-model")).toBeInTheDocument();
  });
});
