import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useNavigate } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

vi.mock("./graph-canvas", () => ({
  GraphCanvas: ({
    nodes,
    highlightedNodes,
    onNodeClick,
    onNodeContextMenu,
  }: {
    nodes: { id: string; label: string }[];
    highlightedNodes: Set<string>;
    onNodeClick: (id: string) => void;
    onNodeContextMenu?: (id: string, x: number, y: number) => void;
  }) => (
    <div data-testid="graph-canvas" data-highlighted={[...highlightedNodes].join(",")}>
      {nodes.map((node) => (
        <div key={node.id}>
          <button onClick={() => onNodeClick(node.id)} type="button">
            {node.label}
          </button>
          <button
            aria-label={`context ${node.label}`}
            onClick={() => onNodeContextMenu?.(node.id, 0, 0)}
            type="button"
          >
            ctx {node.label}
          </button>
        </div>
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
        { id: "ov", label: "Overview", nodeType: "overview", path: "wiki/overview.md", linkCount: 0, sources: [] },
        { id: "iso", label: "Iso", nodeType: "concept", path: "wiki/iso.md", linkCount: 0, sources: [] },
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

function NavTo({ to, label }: { to: string; label: string }) {
  const navigate = useNavigate();
  return (
    <button onClick={() => navigate(to)} type="button">
      {label}
    </button>
  );
}

function renderWithProjectNav() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/projects/p1/graph"]}>
        <NavTo label="go p2" to="/projects/p2/graph" />
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

  it("legend reflects the full graph, including types filtered from the view", () => {
    renderPage();
    // 'overview' is structural and hidden from the canvas by default, but the
    // legend is full-graph (upstream graph-view.tsx:795-798), so its type shows.
    expect(screen.queryByRole("button", { name: "Overview" })).not.toBeInTheDocument();
    expect(screen.getByText("overview")).toBeInTheDocument();
  });

  it("clears the highlight when the insights panel is closed", async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(screen.getByRole("button", { name: "Insights" }));
    await user.click(screen.getByRole("button", { name: /isolated page/i }));
    expect(screen.getByTestId("graph-canvas")).toHaveAttribute("data-highlighted", "iso");
    await user.click(screen.getByRole("button", { name: "Close insights" }));
    expect(screen.getByTestId("graph-canvas")).toHaveAttribute("data-highlighted", "");
  });

  it("keeps dismissed insights dismissed across a panel toggle", async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(screen.getByRole("button", { name: "Insights" }));
    const gapButton = screen.getByRole("button", { name: /isolated page/i });
    await user.click(within(gapButton.closest("li")!).getByRole("button", { name: "Dismiss insight" }));
    expect(screen.queryByText(/isolated page/i)).not.toBeInTheDocument();
    // toggle the panel off and back on
    await user.click(screen.getByRole("button", { name: "Insights" }));
    await user.click(screen.getByRole("button", { name: "Insights" }));
    expect(screen.queryByText(/isolated page/i)).not.toBeInTheDocument();
  });

  it("clears a matching highlight when its insight is dismissed", async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(screen.getByRole("button", { name: "Insights" }));
    await user.click(screen.getByRole("button", { name: /isolated page/i }));
    expect(screen.getByTestId("graph-canvas")).toHaveAttribute("data-highlighted", "iso");
    const gapButton = screen.getByRole("button", { name: /isolated page/i });
    await user.click(within(gapButton.closest("li")!).getByRole("button", { name: "Dismiss insight" }));
    expect(screen.getByTestId("graph-canvas")).toHaveAttribute("data-highlighted", "");
  });

  it("hides a node via the context menu and restores it from the hidden panel", async () => {
    const user = userEvent.setup();
    renderPage();
    expect(screen.getByRole("button", { name: "Alpha" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "context Alpha" }));
    await user.click(screen.getByRole("button", { name: "Hide this node" }));
    expect(screen.queryByRole("button", { name: "Alpha" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Show" }));
    expect(screen.getByRole("button", { name: "Alpha" })).toBeInTheDocument();
  });

  it("resets hidden-node state when switching projects", async () => {
    const user = userEvent.setup();
    renderWithProjectNav();

    await user.click(screen.getByRole("button", { name: "context Alpha" }));
    await user.click(screen.getByRole("button", { name: "Hide this node" }));
    expect(screen.queryByRole("button", { name: "Alpha" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "go p2" }));
    expect(screen.getByRole("button", { name: "Alpha" })).toBeInTheDocument();
  });
});
