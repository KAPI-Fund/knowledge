import { describe, expect, it } from "vitest";
import { buildGraphModel } from "./wiki-graph";
import type { ApiGraphEdge, ApiGraphNode } from "./types";

const nodes: ApiGraphNode[] = [
  { id: "a", label: "Alpha", nodeType: "concept", path: "wiki/a.md", linkCount: 2, sources: ["s1.pdf"] },
  { id: "b", label: "Beta", nodeType: "concept", path: "wiki/b.md", linkCount: 2, sources: ["s1.pdf"] },
  { id: "c", label: "Gamma", nodeType: "entity", path: "wiki/c.md", linkCount: 1, sources: [] },
  { id: "q", label: "Query", nodeType: "query", path: "wiki/q.md", linkCount: 1, sources: [] },
];
const edges: ApiGraphEdge[] = [
  { source: "a", target: "b", weight: 2 },
  { source: "b", target: "c", weight: 1 },
  { source: "a", target: "q", weight: 1 },
];

describe("buildGraphModel", () => {
  it("drops query-type nodes and edges touching them", () => {
    const model = buildGraphModel(nodes, edges);
    expect(model.nodes.map((n) => n.id).sort()).toEqual(["a", "b", "c"]);
    expect(model.edges).toHaveLength(2);
  });

  it("assigns every node a non-negative community", () => {
    const model = buildGraphModel(nodes, edges);
    expect(model.communities.length).toBeGreaterThan(0);
    for (const node of model.nodes) {
      expect(node.community).toBeGreaterThanOrEqual(0);
    }
  });

  it("recomputes edge weights via relevance scoring", () => {
    const model = buildGraphModel(nodes, edges);
    const ab = model.edges.find((e) => e.source === "a" && e.target === "b");
    // linked 3.0 + shared source 4.0 + concept-concept affinity 0.8 = 7.8
    expect(ab?.weight).toBeCloseTo(7.8, 5);
  });

  it("handles a single isolated node without throwing (no-edge guard)", () => {
    const lone: ApiGraphNode[] = [
      { id: "x", label: "Lone", nodeType: "concept", path: "wiki/x.md", linkCount: 0, sources: [] },
    ];
    const model = buildGraphModel(lone, []);
    expect(model.nodes).toHaveLength(1);
    expect(model.edges).toHaveLength(0);
    expect(model.nodes[0].community).toBe(0);
  });
});
