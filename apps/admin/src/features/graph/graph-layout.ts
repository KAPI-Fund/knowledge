import forceAtlas2 from "graphology-layout-forceatlas2";
import type Graph from "graphology";
import type { GraphEdge, GraphNode } from "./types";

export function layoutIterations(nodeCount: number): number {
  if (nodeCount > 2500) return 28;
  if (nodeCount > 1200) return 40;
  if (nodeCount > 600) return 65;
  if (nodeCount > 250) return 90;
  return 140;
}

export function edgeVisibilityThreshold(nodeCount: number): number {
  if (nodeCount > 2500) return 0.16;
  if (nodeCount > 1200) return 0.1;
  if (nodeCount > 700) return 0.05;
  return 0;
}

export function labelSizeThreshold(nodeCount: number): number {
  if (nodeCount > 2500) return 18;
  if (nodeCount > 1200) return 14;
  if (nodeCount > 600) return 10;
  return 6;
}

export function labelDensity(nodeCount: number): number {
  if (nodeCount > 2500) return 0.08;
  if (nodeCount > 1200) return 0.14;
  if (nodeCount > 600) return 0.24;
  return 0.4;
}

export function hashParts(parts: readonly string[]): string {
  let hash = 2166136261;
  for (const part of parts) {
    for (let i = 0; i < part.length; i++) {
      hash ^= part.charCodeAt(i);
      hash = Math.imul(hash, 16777619);
    }
    hash ^= 0xff;
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

export function graphDataKey(
  nodes: readonly GraphNode[],
  edges: readonly GraphEdge[],
  graphSpacing: number,
): string {
  const nodeIds = nodes.map((n) => n.id).sort();
  const edgeIds = edges
    .map((e) => `${e.source}->${e.target}:${Math.round(e.weight * 1000)}`)
    .sort();
  return `${hashParts(nodeIds)}:${hashParts(edgeIds)}:${nodes.length}:${edges.length}:${graphSpacing.toFixed(2)}`;
}

export function scalingRatioFor(nodeCount: number, graphSpacing: number): number {
  return graphSpacing * (nodeCount > 400 ? 3 : 2);
}

/** Run ForceAtlas2 in place on the graph (upstream graph-view.tsx:285-303). */
export function runForceLayout(
  graph: Graph,
  nodeCount: number,
  graphSpacing: number,
): void {
  const settings = forceAtlas2.inferSettings(graph);
  forceAtlas2.assign(graph, {
    iterations: layoutIterations(nodeCount),
    settings: {
      ...settings,
      gravity: 1,
      scalingRatio: scalingRatioFor(nodeCount, graphSpacing),
      strongGravityMode: true,
      barnesHutOptimize: nodeCount > 50,
    },
  });
}

export function makeLayoutWorker(): Worker | null {
  try {
    return new Worker(new URL("./graph-layout-worker.ts", import.meta.url), {
      type: "module",
    });
  } catch (err) {
    console.warn("[Graph] failed to start layout worker; falling back to main-thread layout:", err);
    return null;
  }
}
