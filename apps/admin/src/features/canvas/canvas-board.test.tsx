import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@xyflow/react/dist/style.css", () => ({}));
vi.mock("@xyflow/react", () => ({
  ReactFlow: (props: {
    onMoveEnd?: (e: unknown, vp: { x: number; y: number; zoom: number }) => void;
  }) => (
    <div>
      <button type="button" onClick={() => props.onMoveEnd?.(null, { x: 5, y: 6, zoom: 2 })}>
        move
      </button>
      flow
    </div>
  ),
  ReactFlowProvider: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  Background: () => null,
  Controls: () => null,
  addEdge: (c: unknown, edges: unknown[]) => edges,
  applyEdgeChanges: (_c: unknown, edges: unknown[]) => edges,
  applyNodeChanges: (_c: unknown, nodes: unknown[]) => nodes,
}));

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

import { CanvasBoard, commitNodeGeometry, isResizeEndChange, isValidConnection, pruneDanglingEdges } from "./canvas-board";
import type { CanvasDocument } from "./types";

const emptyDoc = { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } };

function node(id: string): CanvasDocument["nodes"][number] {
  return { id, type: "note", x: 0, y: 0, w: 280, h: 160, data: {} };
}

describe("CanvasBoard", () => {
  it("renders the flow", () => {
    render(
      <CanvasBoard
        document={emptyDoc}
        onChange={() => {}}
        onRunNode={() => {}}
        onFetchUrl={() => {}}
      />,
    );
    expect(screen.getByText("flow")).toBeInTheDocument();
  });

  it("does not persist on viewport move (pan/zoom is not saved)", () => {
    const onChange = vi.fn();
    render(
      <CanvasBoard
        document={emptyDoc}
        onChange={onChange}
        onRunNode={() => {}}
        onFetchUrl={() => {}}
      />,
    );
    // The board no longer wires onMoveEnd, so panning/zooming never triggers a
    // save -- only substantive node/edge changes should.
    fireEvent.click(screen.getByText("move"));
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("pruneDanglingEdges", () => {
  const edges = [
    { id: "e1", source: "a", target: "b" },
    { id: "e2", source: "b", target: "c" },
  ];

  it("keeps edges whose endpoints both survive", () => {
    const kept = pruneDanglingEdges([node("a"), node("b"), node("c")], edges);
    expect(kept.map((e) => e.id)).toEqual(["e1", "e2"]);
  });

  it("drops edges that reference a removed node", () => {
    const kept = pruneDanglingEdges([node("a"), node("b")], edges);
    expect(kept.map((e) => e.id)).toEqual(["e1"]);
  });
});

describe("isValidConnection", () => {
  const nodes = [
    { id: "note1", type: "note" },
    { id: "an1", type: "ai_analyze" },
    { id: "img1", type: "ai_image" },
    { id: "s1", type: "search" },
    { id: "kb1", type: "kb" },
  ];

  it("accepts a producer -> consumer edge", () => {
    expect(isValidConnection(nodes, [], { source: "note1", target: "an1" })).toBe(true);
  });

  it("rejects a self-loop", () => {
    expect(isValidConnection(nodes, [], { source: "an1", target: "an1" })).toBe(false);
  });

  it("rejects a target that is not a consumer", () => {
    expect(isValidConnection(nodes, [], { source: "an1", target: "note1" })).toBe(false);
    expect(isValidConnection(nodes, [], { source: "note1", target: "kb1" })).toBe(false);
  });

  it("rejects a duplicate edge", () => {
    const edges = [{ source: "note1", target: "an1" }];
    expect(isValidConnection(nodes, edges, { source: "note1", target: "an1" })).toBe(false);
  });

  it("rejects an edge that would form a cycle", () => {
    // an1 -> img1 exists; adding img1 -> an1 would close a loop.
    const edges = [{ source: "an1", target: "img1" }];
    expect(isValidConnection(nodes, edges, { source: "img1", target: "an1" })).toBe(false);
  });

  it("rejects a null endpoint", () => {
    expect(isValidConnection(nodes, [], { source: null, target: "an1" })).toBe(false);
  });
});

describe("isResizeEndChange", () => {
  it("is true only for a dimensions change that ends a resize (resizing === false)", () => {
    expect(isResizeEndChange({ id: "a", type: "dimensions", resizing: false })).toBe(true);
  });

  it("is false mid-resize (resizing === true)", () => {
    expect(isResizeEndChange({ id: "a", type: "dimensions", resizing: true })).toBe(false);
  });

  it("is false for a passive measurement (no resizing flag)", () => {
    // React Flow's ResizeObserver reports measured dimensions as a 'dimensions'
    // change with no `resizing` flag; that must not be treated as a user resize.
    expect(isResizeEndChange({ id: "a", type: "dimensions", dimensions: { width: 300, height: 200 } })).toBe(false);
  });

  it("is false for non-dimensions changes", () => {
    expect(isResizeEndChange({ id: "a", type: "position", dragging: false })).toBe(false);
    expect(isResizeEndChange({ id: "a", type: "select", selected: true })).toBe(false);
  });
});

describe("commitNodeGeometry", () => {
  const doc: CanvasDocument = {
    nodes: [{ id: "n1", type: "note", x: 10, y: 20, w: 280, h: 160, data: { markdown: "hi" } }],
    edges: [],
    viewport: { x: 0, y: 0, zoom: 1 },
  };

  it("writes back position and the resized width/height from React Flow's node", () => {
    const rfNodes = [
      { id: "n1", position: { x: 15, y: 25 }, width: 420, height: 300, data: {} },
    ];
    const next = commitNodeGeometry(doc, rfNodes);
    expect(next.nodes[0]).toMatchObject({ x: 15, y: 25, w: 420, h: 300 });
    // Untouched data is preserved.
    expect(next.nodes[0].data).toEqual({ markdown: "hi" });
  });

  it("falls back to measured dimensions, then the stored size, when width/height are absent", () => {
    const measured = [{ id: "n1", position: { x: 10, y: 20 }, measured: { width: 500, height: 350 }, data: {} }];
    expect(commitNodeGeometry(doc, measured).nodes[0]).toMatchObject({ w: 500, h: 350 });

    const bare = [{ id: "n1", position: { x: 10, y: 20 }, data: {} }];
    expect(commitNodeGeometry(doc, bare).nodes[0]).toMatchObject({ w: 280, h: 160 });
  });

  it("prunes edges left dangling by a removed node", () => {
    const twoNodeDoc: CanvasDocument = {
      nodes: [node("a"), node("b")],
      edges: [{ id: "e1", source: "a", target: "b" }],
      viewport: { x: 0, y: 0, zoom: 1 },
    };
    const remaining = [{ id: "a", position: { x: 0, y: 0 }, width: 280, height: 160, data: {} }];
    const next = commitNodeGeometry(twoNodeDoc, remaining);
    expect(next.nodes.map((n) => n.id)).toEqual(["a"]);
    expect(next.edges).toEqual([]);
  });
});
