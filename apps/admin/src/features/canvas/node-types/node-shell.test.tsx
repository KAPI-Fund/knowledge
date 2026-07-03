import { ReactFlowProvider } from "@xyflow/react";
import { render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";

import { NodeShell } from "./node-shell";

beforeAll(() => {
  class RO {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  // React Flow's store touches ResizeObserver; <Handle> needs the provider store.
  (globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver = RO;
});

function renderShell() {
  return render(
    <ReactFlowProvider>
      <NodeShell icon={<span>icon</span>} label="AI · IMAGE" nodeId="4339abcd-1111" status="error">
        <div>body</div>
      </NodeShell>
    </ReactFlowProvider>,
  );
}

describe("NodeShell", () => {
  it("renders the label and short-id chip", () => {
    renderShell();
    expect(screen.getByText("AI · IMAGE")).toBeInTheDocument();
    expect(screen.getByText("4339")).toBeInTheDocument();
  });

  it("shows an ERROR status pill for the error status", () => {
    renderShell();
    expect(screen.getByText("ERROR")).toBeInTheDocument();
  });

  it("renders a source and a target connection handle", () => {
    const { container } = renderShell();
    expect(container.querySelectorAll(".react-flow__handle").length).toBe(2);
  });

  it("renders the node index badge when provided", () => {
    render(
      <ReactFlowProvider>
        <NodeShell icon={<span>icon</span>} label="NOTE" nodeId="abcd-1" index={3}>
          <div>body</div>
        </NodeShell>
      </ReactFlowProvider>,
    );
    // Both the index badge (3) and the short-id chip (abcd) are shown.
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("abcd")).toBeInTheDocument();
  });

  it("applies a selection ring only when selected", () => {
    const { rerender, container } = render(
      <ReactFlowProvider>
        <NodeShell icon={<span>icon</span>} label="NOTE" nodeId="abcd-1">
          <div>body</div>
        </NodeShell>
      </ReactFlowProvider>,
    );
    expect(container.querySelector(".ring-2")).toBeNull();
    rerender(
      <ReactFlowProvider>
        <NodeShell icon={<span>icon</span>} label="NOTE" nodeId="abcd-1" selected>
          <div>body</div>
        </NodeShell>
      </ReactFlowProvider>,
    );
    expect(container.querySelector(".ring-2")).not.toBeNull();
  });
});
