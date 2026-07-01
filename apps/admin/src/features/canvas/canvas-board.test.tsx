import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@xyflow/react", () => ({
  ReactFlow: ({ nodes }: { nodes: Array<{ id: string }> }) => (
    <div data-testid="rf">
      {nodes.map((n) => (
        <span key={n.id}>{n.id}</span>
      ))}
    </div>
  ),
  Background: () => null,
  Controls: () => null,
  ReactFlowProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
  applyNodeChanges: (_: unknown, nodes: unknown) => nodes,
  applyEdgeChanges: (_: unknown, edges: unknown) => edges,
  addEdge: (_: unknown, edges: unknown) => edges,
}));

import { CanvasBoard } from "./canvas-board";

describe("CanvasBoard", () => {
  it("renders a node per document node", () => {
    render(
      <CanvasBoard
        document={{
          nodes: [
            { id: "n1", type: "note", x: 0, y: 0, w: 280, h: 160, data: { markdown: "" } },
            { id: "n2", type: "note", x: 0, y: 0, w: 280, h: 160, data: { markdown: "" } },
          ],
          edges: [],
          viewport: { x: 0, y: 0, zoom: 1 },
        }}
        onChange={() => {}}
        onRunNode={() => {}}
        onFetchUrl={() => {}}
      />,
    );
    expect(screen.getByText("n1")).toBeInTheDocument();
    expect(screen.getByText("n2")).toBeInTheDocument();
  });
});
