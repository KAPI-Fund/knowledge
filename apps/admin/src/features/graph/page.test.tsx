import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

vi.mock("./graph-canvas", () => ({
  GraphCanvas: ({
    nodes,
    onNodeClick,
  }: {
    nodes: { id: string; label: string }[];
    onNodeClick: (id: string) => void;
  }) => (
    <div data-testid="graph-canvas">
      {nodes.map((node) => (
        <button key={node.id} onClick={() => onNodeClick(node.id)} type="button">
          {node.label}
        </button>
      ))}
    </div>
  ),
}));

vi.mock("./queries", () => ({
  GRAPH_NODE_LIMIT: 1000,
  useProjectGraphQuery: () => ({
    data: {
      nodes: [
        { id: "a", label: "Alpha", nodeType: "concept", path: "wiki/a.md", linkCount: 2, sources: ["s1.pdf"] },
        { id: "b", label: "Beta", nodeType: "concept", path: "wiki/b.md", linkCount: 2, sources: ["s1.pdf"] },
      ],
      edges: [{ source: "a", target: "b", weight: 2 }],
    },
    isLoading: false,
    error: null,
  }),
  useProjectGraphNeighborsQuery: () => ({ data: undefined, isLoading: false, error: null }),
}));

import { GraphPage } from "./page";

function renderPage() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/projects/p1/graph"]}>
        <Routes>
          <Route element={<GraphPage />} path="/projects/:projectId/graph" />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("GraphPage", () => {
  it("mounts the canvas, search control, and node labels", () => {
    renderPage();
    expect(screen.getByRole("heading", { name: "Graph" })).toBeInTheDocument();
    expect(screen.getByTestId("graph-canvas")).toBeInTheDocument();
    expect(screen.getByLabelText("Search graph")).toBeInTheDocument();
    expect(screen.getByText("Alpha")).toBeInTheDocument();
    expect(screen.getByText("Beta")).toBeInTheDocument();
  });
});
