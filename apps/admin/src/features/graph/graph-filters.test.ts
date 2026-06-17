import { describe, expect, it } from "vitest";
import { applyGraphFilters, DEFAULT_GRAPH_FILTERS, isStructuralGraphNode } from "./graph-filters";
import type { GraphEdge, GraphNode } from "./types";

const nodes: GraphNode[] = [
  { id: "index", label: "Index", type: "overview", path: "wiki/index.md", linkCount: 9, community: 0, sources: [] },
  { id: "a", label: "Alpha", type: "concept", path: "wiki/a.md", linkCount: 2, community: 0, sources: [] },
  { id: "b", label: "Beta", type: "entity", path: "wiki/b.md", linkCount: 0, community: 1, sources: [] },
];
const edges: GraphEdge[] = [{ source: "index", target: "a", weight: 1 }];

describe("applyGraphFilters", () => {
  it("hides structural nodes by default", () => {
    expect(isStructuralGraphNode(nodes[0])).toBe(true);
    const result = applyGraphFilters(nodes, edges, DEFAULT_GRAPH_FILTERS);
    expect(result.nodes.map((n) => n.id).sort()).toEqual(["a", "b"]);
    expect(result.edges).toHaveLength(0);
  });

  it("hides isolated nodes when hideIsolated is on", () => {
    const result = applyGraphFilters(nodes, edges, {
      ...DEFAULT_GRAPH_FILTERS,
      hideIsolated: true,
    });
    expect(result.nodes.map((n) => n.id)).toEqual(["a"]);
  });

  it("hides nodes whose type is in hiddenTypes", () => {
    const result = applyGraphFilters(nodes, edges, {
      ...DEFAULT_GRAPH_FILTERS,
      hideStructural: false,
      hiddenTypes: new Set(["entity"]),
    });
    expect(result.nodes.map((n) => n.id).sort()).toEqual(["a", "index"]);
  });
});
