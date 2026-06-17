import { describe, expect, it } from "vitest";
import { calculateRelevance } from "./graph-relevance";
import type { RelevanceGraph, RelevanceNode } from "./types";

function makeGraph(nodes: RelevanceNode[]): RelevanceGraph {
  return { nodes: new Map(nodes.map((n) => [n.id, n])) };
}

describe("calculateRelevance", () => {
  it("sums direct link, source overlap, and type affinity", () => {
    const a: RelevanceNode = { id: "a", type: "concept", sources: ["s1.pdf"], neighbors: new Set(["b"]) };
    const b: RelevanceNode = { id: "b", type: "concept", sources: ["s1.pdf"], neighbors: new Set(["a"]) };
    const graph = makeGraph([a, b]);
    // directLink 3.0 + sourceOverlap 4.0 + commonNeighbor 0 + typeAffinity 0.8 = 7.8
    expect(calculateRelevance(a, b, graph)).toBeCloseTo(7.8, 5);
  });

  it("returns 0 for the same node", () => {
    const a: RelevanceNode = { id: "a", type: "concept", sources: [], neighbors: new Set() };
    expect(calculateRelevance(a, a, makeGraph([a]))).toBe(0);
  });
});
