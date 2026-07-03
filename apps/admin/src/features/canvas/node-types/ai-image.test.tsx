import { ReactFlowProvider } from "@xyflow/react";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";

import { AiImageNode, type AiImageNodeData } from "./ai-image";

beforeAll(() => {
  class RO {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  (globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver = RO;
});

function renderNode(data: AiImageNodeData, onRegenerate = vi.fn()) {
  const result = render(
    <ReactFlowProvider>
      <AiImageNode
        data={data}
        nodeId="abcd1234"
        model="gpt-image"
        onRegenerate={onRegenerate}
        onVersionChange={vi.fn()}
      />
    </ReactFlowProvider>,
  );
  return { ...result, onRegenerate };
}

describe("AiImageNode", () => {
  it("renders the failure block on error", () => {
    renderNode({ status: "error", error: "HTTP 502 - provider_error" });
    expect(screen.getByText("Image generation failed")).toBeInTheDocument();
    expect(screen.getByText(/HTTP 502 - provider_error/)).toBeInTheDocument();
  });

  it("renders the active version image", () => {
    const { container } = renderNode({
      status: "idle",
      activeVersionId: "v1",
      versions: [{ id: "v1", url: "https://img.test/a.png" }],
    });
    const img = container.querySelector("img");
    expect(img).not.toBeNull();
    expect(img?.getAttribute("src")).toBe("https://img.test/a.png");
  });

  it("renders the empty hint when there is no version", () => {
    renderNode({ status: "idle" });
    expect(screen.getByText(/click run/i)).toBeInTheDocument();
  });

  it("calls onRegenerate when the action button is clicked", () => {
    const { onRegenerate } = renderNode({ status: "error", error: "boom" });
    fireEvent.click(screen.getByRole("button", { name: /regenerate image/i }));
    expect(onRegenerate).toHaveBeenCalledOnce();
  });
});
