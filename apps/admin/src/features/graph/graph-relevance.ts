import type { RelevanceGraph, RelevanceNode } from "./types";

export const WEIGHTS = {
  directLink: 3.0,
  sourceOverlap: 4.0,
  commonNeighbor: 1.5,
  typeAffinity: 1.0,
} as const;

export const TYPE_AFFINITY: Record<string, Record<string, number>> = {
  entity: { concept: 1.2, entity: 0.8, source: 1.0, synthesis: 1.0, query: 0.8 },
  concept: { entity: 1.2, concept: 0.8, source: 1.0, synthesis: 1.2, query: 1.0 },
  source: { entity: 1.0, concept: 1.0, source: 0.5, query: 0.8, synthesis: 1.0 },
  query: { concept: 1.0, entity: 0.8, synthesis: 1.0, source: 0.8, query: 0.5 },
  synthesis: { concept: 1.2, entity: 1.0, source: 1.0, query: 1.0, synthesis: 0.8 },
};

export function calculateRelevance(
  nodeA: RelevanceNode,
  nodeB: RelevanceNode,
  graph: RelevanceGraph,
): number {
  if (nodeA.id === nodeB.id) return 0;

  // Signal 1: Direct link (weight 3.0). Backend edges are undirected/merged,
  // so a present edge counts once. (Upstream graph-relevance.ts:254-257.)
  const linked = nodeA.neighbors.has(nodeB.id) ? 1 : 0;
  const directLinkScore = linked * WEIGHTS.directLink;

  // Signal 2: Source overlap (weight 4.0). (Upstream :259-265.)
  const sourcesA = new Set(nodeA.sources);
  let sharedSourceCount = 0;
  for (const src of nodeB.sources) {
    if (sourcesA.has(src)) sharedSourceCount += 1;
  }
  const sourceOverlapScore = sharedSourceCount * WEIGHTS.sourceOverlap;

  // Signal 3: Common neighbors — Adamic-Adar (weight 1.5). (Upstream :267-280.)
  let adamicAdar = 0;
  for (const neighborId of nodeA.neighbors) {
    if (nodeB.neighbors.has(neighborId)) {
      const neighbor = graph.nodes.get(neighborId);
      if (neighbor) {
        const degree = neighbor.neighbors.size;
        adamicAdar += 1 / Math.log(Math.max(degree, 2));
      }
    }
  }
  const commonNeighborScore = adamicAdar * WEIGHTS.commonNeighbor;

  // Signal 4: Type affinity (weight 1.0). (Upstream :282-284.)
  const affinityMap = TYPE_AFFINITY[nodeA.type];
  const typeAffinityScore = (affinityMap?.[nodeB.type] ?? 0.5) * WEIGHTS.typeAffinity;

  return directLinkScore + sourceOverlapScore + commonNeighborScore + typeAffinityScore;
}
