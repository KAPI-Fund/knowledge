import type { GraphEdge } from "./types";

export interface HighlightChain {
  /** Nodes that (transitively) link into the selected node. */
  upstream: Set<string>;
  /** Nodes the selected node (transitively) links to. */
  downstream: Set<string>;
  /** Edge keys (`source->target`) belonging to either chain. */
  links: Set<string>;
}

export function edgeKey(edge: { source: string; target: string }): string {
  return `${edge.source}->${edge.target}`;
}

export const EMPTY_CHAIN: HighlightChain = {
  upstream: new Set(),
  downstream: new Set(),
  links: new Set(),
};

/**
 * Bidirectional BFS over link direction from the selected node. Port of
 * os-taxonomy index.html calculateHighlightChain, with prerequisite edges
 * mapped to our wiki-link edges (source links to target).
 */
export function calculateHighlightChain(nodeId: string, edges: GraphEdge[]): HighlightChain {
  const incoming = new Map<string, string[]>();
  const outgoing = new Map<string, string[]>();
  for (const edge of edges) {
    const inList = incoming.get(edge.target);
    if (inList) inList.push(edge.source);
    else incoming.set(edge.target, [edge.source]);
    const outList = outgoing.get(edge.source);
    if (outList) outList.push(edge.target);
    else outgoing.set(edge.source, [edge.target]);
  }

  const upstream = new Set<string>([nodeId]);
  const upQueue = [nodeId];
  while (upQueue.length > 0) {
    const current = upQueue.shift() as string;
    for (const source of incoming.get(current) ?? []) {
      if (!upstream.has(source)) {
        upstream.add(source);
        upQueue.push(source);
      }
    }
  }

  const downstream = new Set<string>([nodeId]);
  const downQueue = [nodeId];
  while (downQueue.length > 0) {
    const current = downQueue.shift() as string;
    for (const target of outgoing.get(current) ?? []) {
      if (!downstream.has(target)) {
        downstream.add(target);
        downQueue.push(target);
      }
    }
  }

  upstream.delete(nodeId);
  downstream.delete(nodeId);

  const links = new Set<string>();
  for (const edge of edges) {
    const sourceInUp = upstream.has(edge.source) || edge.source === nodeId;
    const targetInUp = upstream.has(edge.target) || edge.target === nodeId;
    const sourceInDown = downstream.has(edge.source) || edge.source === nodeId;
    const targetInDown = downstream.has(edge.target) || edge.target === nodeId;
    if ((sourceInUp && targetInUp) || (sourceInDown && targetInDown)) {
      links.add(edgeKey(edge));
    }
  }

  return { upstream, downstream, links };
}

/**
 * BFS dependency depth used for the tornado layout tiers (replaces the age
 * field in os-taxonomy). Roots are nodes with in-degree 0; nodes only
 * reachable through cycles fall back to min(predecessor layer) + 1.
 */
export function computeDepthLayers(
  nodes: Array<{ id: string }>,
  edges: GraphEdge[],
): { layers: Map<string, number>; maxLayer: number } {
  const ids = new Set(nodes.map((node) => node.id));
  const incoming = new Map<string, string[]>();
  const outgoing = new Map<string, string[]>();
  for (const edge of edges) {
    if (!ids.has(edge.source) || !ids.has(edge.target)) continue;
    const inList = incoming.get(edge.target);
    if (inList) inList.push(edge.source);
    else incoming.set(edge.target, [edge.source]);
    const outList = outgoing.get(edge.source);
    if (outList) outList.push(edge.target);
    else outgoing.set(edge.source, [edge.target]);
  }

  const layers = new Map<string, number>();
  const queue: string[] = [];
  for (const node of nodes) {
    if (!incoming.has(node.id)) {
      layers.set(node.id, 0);
      queue.push(node.id);
    }
  }
  while (queue.length > 0) {
    const current = queue.shift() as string;
    const depth = layers.get(current) as number;
    for (const next of outgoing.get(current) ?? []) {
      if (!layers.has(next)) {
        layers.set(next, depth + 1);
        queue.push(next);
      }
    }
  }

  let changed = true;
  while (changed) {
    changed = false;
    for (const node of nodes) {
      if (layers.has(node.id)) continue;
      const assigned = (incoming.get(node.id) ?? [])
        .map((source) => layers.get(source))
        .filter((value): value is number => value !== undefined);
      if (assigned.length > 0) {
        layers.set(node.id, Math.min(...assigned) + 1);
        changed = true;
      }
    }
  }
  for (const node of nodes) {
    if (!layers.has(node.id)) layers.set(node.id, 0);
  }

  const maxLayer = Math.max(0, ...layers.values());
  return { layers, maxLayer };
}
