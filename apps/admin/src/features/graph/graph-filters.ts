import type { GraphEdge, GraphFilterState, GraphNode } from "./types";

export interface FilteredGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
  hiddenNodeIds: Set<string>;
}

export const DEFAULT_GRAPH_FILTERS: GraphFilterState = {
  hiddenTypes: new Set(),
  hiddenNodeIds: new Set(),
  hideStructural: true,
  hideIsolated: false,
  maxLinks: undefined,
};

const STRUCTURAL_IDS = new Set(["index", "overview", "log", "schema", "purpose"]);

/** Inlined from upstream graph-visibility.ts:1-6. */
function shouldHideNodeType(
  nodeType: string | undefined,
  hiddenTypes: ReadonlySet<string>,
): boolean {
  return nodeType !== undefined && hiddenTypes.has(nodeType);
}

export function isStructuralGraphNode(
  node: Pick<GraphNode, "id" | "path" | "type">,
): boolean {
  const id = node.id.toLowerCase();
  if (STRUCTURAL_IDS.has(id)) return true;
  if (node.type === "overview") return true;

  const normalizedPath = node.path.replace(/\\/g, "/").toLowerCase();
  return (
    normalizedPath.endsWith("/wiki/index.md") ||
    normalizedPath.endsWith("/wiki/overview.md") ||
    normalizedPath.endsWith("/wiki/log.md") ||
    normalizedPath.endsWith("/purpose.md") ||
    normalizedPath.endsWith("/schema.md")
  );
}

export function applyGraphFilters(
  nodes: readonly GraphNode[],
  edges: readonly GraphEdge[],
  filters: GraphFilterState,
): FilteredGraph {
  const hiddenNodeIds = new Set<string>();

  for (const node of nodes) {
    if (filters.hiddenNodeIds.has(node.id)) {
      hiddenNodeIds.add(node.id);
      continue;
    }
    if (shouldHideNodeType(node.type, filters.hiddenTypes)) {
      hiddenNodeIds.add(node.id);
      continue;
    }
    if (filters.hideStructural && isStructuralGraphNode(node)) {
      hiddenNodeIds.add(node.id);
      continue;
    }
    if (filters.hideIsolated && node.linkCount <= 0) {
      hiddenNodeIds.add(node.id);
      continue;
    }
    if (filters.maxLinks !== undefined && node.linkCount > filters.maxLinks) {
      hiddenNodeIds.add(node.id);
    }
  }

  const visibleNodes = nodes.filter((node) => !hiddenNodeIds.has(node.id));
  const visibleNodeIds = new Set(visibleNodes.map((node) => node.id));
  const visibleEdges = edges.filter(
    (edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target),
  );

  return { nodes: visibleNodes, edges: visibleEdges, hiddenNodeIds };
}

export function hasActiveGraphFilters(filters: GraphFilterState): boolean {
  return (
    filters.hideStructural ||
    filters.hideIsolated ||
    filters.hiddenTypes.size > 0 ||
    filters.hiddenNodeIds.size > 0 ||
    filters.maxLinks !== undefined
  );
}
