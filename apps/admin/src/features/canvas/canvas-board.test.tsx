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

import { CanvasBoard } from "./canvas-board";

const emptyDoc = { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } };

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

  it("persists viewport on move end", () => {
    const onChange = vi.fn();
    render(
      <CanvasBoard
        document={emptyDoc}
        onChange={onChange}
        onRunNode={() => {}}
        onFetchUrl={() => {}}
      />,
    );
    fireEvent.click(screen.getByText("move"));
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({ viewport: { x: 5, y: 6, zoom: 2 } }),
    );
  });
});
