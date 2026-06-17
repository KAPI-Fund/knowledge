import Graph from "graphology";
import louvain from "graphology-communities-louvain";
import { calculateRelevance } from "./graph-relevance";
import type {
  ApiGraphEdge,
  ApiGraphNode,
  CommunityInfo,
  GraphEdge,
  GraphModel,
  GraphNode,
  RelevanceGraph,
  RelevanceNode,
} from "./types";

const HIDDEN_TYPES = new Set(["query"]);

/** Run Louvain community detection and compute cohesion per community.
 * Verbatim port of upstream wiki-graph.ts:31-113. */
export function detectCommunities(
  nodes: { id: string; label: string; linkCount: number }[],
  edges: GraphEdge[],
): { assignments: Map<string, number>; communities: CommunityInfo[] } {
  if (nodes.length === 0) {
    return { assignments: new Map(), communities: [] };
  }

  const g = new Graph({ type: "undirected" });
  for (const node of nodes) {
    g.addNode(node.id);
  }
  for (const edge of edges) {
    if (g.hasNode(edge.source) && g.hasNode(edge.target)) {
      const key = `${edge.source}->${edge.target}`;
      if (!g.hasEdge(key) && !g.hasEdge(`${edge.target}->${edge.source}`)) {
        g.addEdgeWithKey(key, edge.source, edge.target, { weight: edge.weight });
      }
    }
  }

  // graphology-communities-louvain throws on an edgeless graph. With no edges
  // every node is a trivial community 0. (Not in upstream, which only ran on
  // file-derived graphs that had edges; needed here for empty/isolated REST data.)
  if (g.size === 0) {
    return {
      assignments: new Map(nodes.map((n) => [n.id, 0])),
      communities: [
        {
          id: 0,
          nodeCount: nodes.length,
          cohesion: 0,
          topNodes: [...nodes]
            .sort((a, b) => b.linkCount - a.linkCount)
            .slice(0, 5)
            .map((n) => n.label),
        },
      ],
    };
  }

  const communityMap: Record<string, number> = louvain(g, { resolution: 1 });
  const assignments = new Map(
    Object.entries(communityMap).map(([k, v]) => [k, v as number]),
  );

  const groups = new Map<number, string[]>();
  for (const [nodeId, commId] of assignments) {
    const list = groups.get(commId) ?? [];
    list.push(nodeId);
    groups.set(commId, list);
  }

  const edgeSet = new Set<string>();
  for (const edge of edges) {
    edgeSet.add(`${edge.source}:::${edge.target}`);
    edgeSet.add(`${edge.target}:::${edge.source}`);
  }

  const nodeInfo = new Map(
    nodes.map((n) => [n.id, { label: n.label, linkCount: n.linkCount }]),
  );

  const communities: CommunityInfo[] = [];
  for (const [commId, memberIds] of groups) {
    const n = memberIds.length;
    let intraEdges = 0;
    for (let i = 0; i < memberIds.length; i++) {
      for (let j = i + 1; j < memberIds.length; j++) {
        if (edgeSet.has(`${memberIds[i]}:::${memberIds[j]}`)) {
          intraEdges++;
        }
      }
    }
    const possibleEdges = n > 1 ? (n * (n - 1)) / 2 : 1;
    const cohesion = intraEdges / possibleEdges;

    const sorted = [...memberIds].sort(
      (a, b) => (nodeInfo.get(b)?.linkCount ?? 0) - (nodeInfo.get(a)?.linkCount ?? 0),
    );
    const topNodes = sorted.slice(0, 5).map((id) => nodeInfo.get(id)?.label ?? id);

    communities.push({ id: commId, nodeCount: n, cohesion, topNodes });
  }

  communities.sort((a, b) => b.nodeCount - a.nodeCount);

  const idRemap = new Map<number, number>();
  communities.forEach((c, idx) => {
    idRemap.set(c.id, idx);
    c.id = idx;
  });
  for (const [nodeId, oldId] of assignments) {
    assignments.set(nodeId, idRemap.get(oldId) ?? 0);
  }

  return { assignments, communities };
}

/** Enrich the raw graph API response with relevance weights + communities.
 * Replaces upstream buildWikiGraph (159-286); the data source is the REST API
 * instead of Tauri file reads, but the enrichment is faithful. */
export function buildGraphModel(
  apiNodes: ApiGraphNode[],
  apiEdges: ApiGraphEdge[],
): GraphModel {
  // Drop intermediate query artifacts (upstream wiki-graph.ts:201-209).
  const visibleNodes = apiNodes.filter((n) => !HIDDEN_TYPES.has(n.nodeType));
  const nodeIds = new Set(visibleNodes.map((n) => n.id));

  // Keep only edges among visible nodes; dedupe undirected pairs.
  const seen = new Set<string>();
  const keptEdges: ApiGraphEdge[] = [];
  for (const e of apiEdges) {
    if (!nodeIds.has(e.source) || !nodeIds.has(e.target)) continue;
    if (e.source === e.target) continue;
    const key = `${e.source}:::${e.target}`;
    const rev = `${e.target}:::${e.source}`;
    if (seen.has(key) || seen.has(rev)) continue;
    seen.add(key);
    keptEdges.push(e);
  }

  // Build neighbor sets for relevance scoring.
  const neighbors = new Map<string, Set<string>>();
  for (const id of nodeIds) neighbors.set(id, new Set());
  for (const e of keptEdges) {
    neighbors.get(e.source)!.add(e.target);
    neighbors.get(e.target)!.add(e.source);
  }

  const relevanceNodes = new Map<string, RelevanceNode>();
  for (const n of visibleNodes) {
    relevanceNodes.set(n.id, {
      id: n.id,
      type: n.nodeType,
      sources: n.sources ?? [],
      neighbors: neighbors.get(n.id) ?? new Set(),
    });
  }
  const relevanceGraph: RelevanceGraph = { nodes: relevanceNodes };

  // Relevance-weight each edge (upstream wiki-graph.ts:255-265).
  const edges: GraphEdge[] = keptEdges.map((e) => {
    const a = relevanceNodes.get(e.source);
    const b = relevanceNodes.get(e.target);
    const weight = a && b ? calculateRelevance(a, b, relevanceGraph) : e.weight;
    return { source: e.source, target: e.target, weight };
  });

  const prelim = visibleNodes.map((n) => ({
    id: n.id,
    label: n.label,
    linkCount: n.linkCount,
  }));
  const { assignments, communities } = detectCommunities(prelim, edges);

  const nodes: GraphNode[] = visibleNodes.map((n) => ({
    id: n.id,
    label: n.label,
    type: n.nodeType,
    path: n.path,
    linkCount: n.linkCount,
    community: assignments.get(n.id) ?? 0,
    sources: n.sources ?? [],
  }));

  return { nodes, edges, communities };
}
