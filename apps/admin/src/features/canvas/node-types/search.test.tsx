import { ReactFlowProvider } from "@xyflow/react";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import { SearchNode, type SearchNodeData } from "./search";

beforeAll(() => {
  class RO {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  (globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver = RO;
});

function renderNode(data: SearchNodeData, onRun = vi.fn(), onQueryChange = vi.fn()) {
  return render(
    <ReactFlowProvider>
      <SearchNode
        data={data}
        nodeId="abcd-1234"
        onQueryChange={onQueryChange}
        onRun={onRun}
      />
    </ReactFlowProvider>,
  );
}

describe("SearchNode", () => {
  it("shows the empty hint when there are no results", () => {
    renderNode({ query: "cats" });
    expect(screen.getByText(/click search/i)).toBeInTheDocument();
  });

  it("renders the markdown results when present", () => {
    renderNode({ query: "cats", markdown: "# found it" });
    expect(screen.getByText("found it")).toBeInTheDocument();
  });

  it("renders the error block on error status", () => {
    renderNode({ query: "cats", status: "error", error: "search boom" });
    expect(screen.getByText("search boom")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /retry/i })).toBeInTheDocument();
  });

  it("calls onRun when the button is clicked", () => {
    const onRun = vi.fn();
    renderNode({ query: "cats" }, onRun);
    fireEvent.click(screen.getByRole("button", { name: /search/i }));
    expect(onRun).toHaveBeenCalledTimes(1);
  });

  it("keeps the button enabled with an empty query (upstream can supply one)", () => {
    renderNode({ query: "" });
    expect(screen.getByRole("button", { name: /search/i })).not.toBeDisabled();
  });

  it("renders both a source and a target handle (it is a consumer)", () => {
    const { container } = renderNode({ query: "cats" });
    expect(container.querySelectorAll(".react-flow__handle").length).toBe(2);
  });
});
