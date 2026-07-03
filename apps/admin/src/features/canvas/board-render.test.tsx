import { render } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

vi.mock("../settings/queries", () => ({
  useSystemSettingsQuery: () => ({ data: { providerModel: "gpt-test" } }),
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
    // Each redesigned node composes NodeShell, which renders a target handle
    // (left) and a source handle (right). Without handles users can only get
    // edges from /analyze auto-wiring, never by dragging between nodes.
    const { container } = render(
      <CanvasBoard
        document={docWith(2)}
        onChange={vi.fn()}
        onRunNode={vi.fn()}
        onFetchUrl={vi.fn()}
        onSearchNode={vi.fn()}
      />,
    );
    expect(container.querySelectorAll(".react-flow__handle").length).toBeGreaterThanOrEqual(4);
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
});
