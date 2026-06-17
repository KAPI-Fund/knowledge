import { describe, expect, it } from "vitest";
import { detectKnowledgeGaps, findSurprisingConnections } from "./graph-insights";
import type { CommunityInfo, GraphEdge, GraphNode } from "./types";

const nodes: GraphNode[] = [
  { id: "a", label: "Alpha", type: "source", path: "wiki/a.md", linkCount: 4, community: 0, sources: [] },
  { id: "b", label: "Beta", type: "concept", path: "wiki/b.md", linkCount: 1, community: 1, sources: [] },
  { id: "iso", label: "Lonely", type: "concept", path: "wiki/iso.md", linkCount: 0, community: 2, sources: [] },
];
const edges: GraphEdge[] = [{ source: "a", target: "b", weight: 1 }];
const communities: CommunityInfo[] = [
  { id: 0, nodeCount: 1, cohesion: 0, topNodes: ["Alpha"] },
  { id: 1, nodeCount: 1, cohesion: 0, topNodes: ["Beta"] },
  { id: 2, nodeCount: 1, cohesion: 0, topNodes: ["Lonely"] },
];

describe("graph-insights", () => {
  it("flags cross-community cross-type edges as surprising", () => {
    const result = findSurprisingConnections(nodes, edges, communities);
    expect(result).toHaveLength(1);
    expect(result[0].score).toBeGreaterThanOrEqual(3);
    expect(result[0].key).toBe("a:::b");
  });

  it("detects isolated nodes as knowledge gaps", () => {
    const gaps = detectKnowledgeGaps(nodes, edges, communities);
    const isolated = gaps.find((g) => g.type === "isolated-node");
    expect(isolated).toBeDefined();
    expect(isolated?.nodeIds).toContain("iso");
  });
});
