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

function renderNode(data: SearchNodeData, onSearch = vi.fn(), onQueryChange = vi.fn()) {
  return render(
    <ReactFlowProvider>
      <SearchNode
        data={data}
        nodeId="abcd-1234"
        onQueryChange={onQueryChange}
        onSearch={onSearch}
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

  it("calls onSearch when the button is clicked", () => {
    const onSearch = vi.fn();
    renderNode({ query: "cats" }, onSearch);
    fireEvent.click(screen.getByRole("button", { name: /search/i }));
    expect(onSearch).toHaveBeenCalledTimes(1);
  });

  it("disables the button with an empty query", () => {
    renderNode({ query: "" });
    expect(screen.getByRole("button", { name: /search/i })).toBeDisabled();
  });

  it("renders only a source handle (no target — it is a data source)", () => {
    const { container } = renderNode({ query: "cats" });
    expect(container.querySelectorAll(".react-flow__handle").length).toBe(1);
  });
});
