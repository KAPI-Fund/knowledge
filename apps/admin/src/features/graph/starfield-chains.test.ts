import { describe, expect, it } from "vitest";

import { calculateHighlightChain, computeDepthLayers, edgeKey } from "./starfield-chains";
import type { GraphEdge } from "./types";

function edge(source: string, target: string): GraphEdge {
  return { source, target, weight: 1 };
}

describe("calculateHighlightChain", () => {
  // a -> b -> c -> d, plus x -> c
  const edges = [edge("a", "b"), edge("b", "c"), edge("c", "d"), edge("x", "c")];

  it("collects transitive upstream and downstream nodes", () => {
    const chain = calculateHighlightChain("c", edges);
    expect(chain.upstream).toEqual(new Set(["a", "b", "x"]));
    expect(chain.downstream).toEqual(new Set(["d"]));
  });

  it("excludes the selected node from both chains", () => {
    const chain = calculateHighlightChain("c", edges);
    expect(chain.upstream.has("c")).toBe(false);
    expect(chain.downstream.has("c")).toBe(false);
  });

  it("collects only edges inside a chain or touching the selected node", () => {
    const chain = calculateHighlightChain("c", edges);
    expect(chain.links).toEqual(
      new Set([edgeKey(edges[0]!), edgeKey(edges[1]!), edgeKey(edges[2]!), edgeKey(edges[3]!)]),
    );
    // edge between an upstream and a downstream node (not through selection) is not highlighted
    const cross = [...edges, edge("a", "d")];
    const crossChain = calculateHighlightChain("c", cross);
    expect(crossChain.links.has("a->d")).toBe(false);
  });

  it("terminates on cycles without duplicating nodes", () => {
    const cyclic = [edge("a", "b"), edge("b", "a"), edge("b", "c")];
    const chain = calculateHighlightChain("c", cyclic);
    expect(chain.upstream).toEqual(new Set(["a", "b"]));
    expect(chain.downstream.size).toBe(0);
  });

  it("returns empty chains for an isolated node", () => {
    const chain = calculateHighlightChain("solo", edges);
    expect(chain.upstream.size).toBe(0);
    expect(chain.downstream.size).toBe(0);
    expect(chain.links.size).toBe(0);
  });
});

describe("computeDepthLayers", () => {
  it("assigns BFS depth from in-degree-0 roots", () => {
    const nodes = [{ id: "a" }, { id: "b" }, { id: "c" }, { id: "d" }];
    const edges = [edge("a", "b"), edge("b", "c"), edge("a", "d")];
    const { layers, maxLayer } = computeDepthLayers(nodes, edges);
    expect(layers.get("a")).toBe(0);
    expect(layers.get("b")).toBe(1);
    expect(layers.get("d")).toBe(1);
    expect(layers.get("c")).toBe(2);
    expect(maxLayer).toBe(2);
  });

  it("puts isolated nodes on layer 0", () => {
    const { layers } = computeDepthLayers([{ id: "solo" }], []);
    expect(layers.get("solo")).toBe(0);
  });

  it("resolves cycle members via assigned predecessors", () => {
    // root -> x, then x <-> y cycle
    const nodes = [{ id: "root" }, { id: "x" }, { id: "y" }];
    const edges = [edge("root", "x"), edge("x", "y"), edge("y", "x")];
    const { layers } = computeDepthLayers(nodes, edges);
    expect(layers.get("root")).toBe(0);
    expect(layers.get("x")).toBe(1);
    expect(layers.get("y")).toBe(2);
  });

  it("defaults unreachable pure cycles to layer 0", () => {
    const nodes = [{ id: "p" }, { id: "q" }];
    const edges = [edge("p", "q"), edge("q", "p")];
    const { layers, maxLayer } = computeDepthLayers(nodes, edges);
    expect(layers.get("p")).toBe(0);
    expect(layers.get("q")).toBe(0);
    expect(maxLayer).toBe(0);
  });

  it("ignores edges referencing nodes outside the visible set", () => {
    const nodes = [{ id: "a" }, { id: "b" }];
    const edges = [edge("a", "b"), edge("ghost", "b"), edge("b", "ghost")];
    const { layers } = computeDepthLayers(nodes, edges);
    expect(layers.get("a")).toBe(0);
    expect(layers.get("b")).toBe(1);
    expect(layers.has("ghost")).toBe(false);
  });
});
