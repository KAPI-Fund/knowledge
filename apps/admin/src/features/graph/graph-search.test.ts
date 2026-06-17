import { describe, expect, it } from "vitest";
import { applyGraphSearch } from "./graph-search";
import type { GraphEdge, GraphNode } from "./types";

const nodes: GraphNode[] = [
  { id: "a", label: "Alpha", type: "concept", path: "wiki/a.md", linkCount: 1, community: 0, sources: [] },
  { id: "b", label: "Beta", type: "entity", path: "wiki/b.md", linkCount: 1, community: 0, sources: [] },
];
const edges: GraphEdge[] = [{ source: "a", target: "b", weight: 1 }];

describe("applyGraphSearch", () => {
  it("returns everything with an empty matched set for a blank query", () => {
    const result = applyGraphSearch(nodes, edges, "   ");
    expect(result.nodes).toHaveLength(2);
    expect(result.matchedNodeIds.size).toBe(0);
  });

  it("matches on label and keeps only edges between visible nodes", () => {
    const result = applyGraphSearch(nodes, edges, "alpha");
    expect(result.nodes.map((n) => n.id)).toEqual(["a"]);
    expect(result.matchedNodeIds.has("a")).toBe(true);
    expect(result.edges).toHaveLength(0);
  });
});
