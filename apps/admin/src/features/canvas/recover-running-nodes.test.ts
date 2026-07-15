import { describe, expect, it } from "vitest";

import { recoverOrphanRunningNodes } from "./recover-running-nodes";
import type { CanvasDocument } from "./types";

function doc(nodes: CanvasDocument["nodes"]): CanvasDocument {
  return { nodes, edges: [], viewport: { x: 0, y: 0, zoom: 1 } };
}

function node(
  id: string,
  type: CanvasDocument["nodes"][number]["type"],
  data: Record<string, unknown>,
): CanvasDocument["nodes"][number] {
  return { id, type, x: 0, y: 0, w: 280, h: 160, data };
}

describe("recoverOrphanRunningNodes", () => {
  it("resets a running ai_image node to a retryable error", () => {
    const input = doc([node("n1", "ai_image", { status: "running", prompt: "p", error: null })]);
    const result = recoverOrphanRunningNodes(input);
    expect(result.nodes[0].data.status).toBe("error");
    expect(result.nodes[0].data.error).toContain("请重试");
    expect(result.nodes[0].data.prompt).toBe("p");
  });

  it("resets running ai_analyze and search nodes too", () => {
    const input = doc([
      node("n1", "ai_analyze", { status: "running" }),
      node("n2", "search", { status: "running" }),
    ]);
    const result = recoverOrphanRunningNodes(input);
    expect(result.nodes.every((n) => n.data.status === "error")).toBe(true);
  });

  it("leaves a running html skill node alone (job poller recovers it)", () => {
    const input = doc([node("n1", "html", { status: "running", jobId: "j1" })]);
    const result = recoverOrphanRunningNodes(input);
    expect(result.nodes[0].data.status).toBe("running");
  });

  it("leaves idle and error nodes untouched and returns the same doc when nothing is orphaned", () => {
    const input = doc([
      node("n1", "ai_image", { status: "idle" }),
      node("n2", "ai_image", { status: "error", error: "x" }),
      node("n3", "note", {}),
    ]);
    expect(recoverOrphanRunningNodes(input)).toBe(input);
  });
});
