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
  useSystemSettingsQuery: () => ({ data: { providerModel: "gpt-test" } }),
}));

import { CanvasBoard, pruneDanglingEdges } from "./canvas-board";
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
        onSearchNode={() => {}}
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
        onSearchNode={() => {}}
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
