export interface GraphNode {
  id: string;
  label: string;
  type: string;
  path: string;
  linkCount: number;
  community: number;
  sources: string[];
}

export interface GraphEdge {
  source: string;
  target: string;
  weight: number;
}

export interface CommunityInfo {
  id: number;
  nodeCount: number;
  cohesion: number;
  topNodes: string[];
}

export interface RetrievalNode {
  id: string;
  title: string;
  type: string;
  path: string;
  sources: string[];
  outLinks: string[];
  inLinks: string[];
}

export interface GraphFilterState {
  hiddenTypes: Set<string>;
  hiddenNodeIds: Set<string>;
  hideStructural: boolean;
  hideIsolated: boolean;
  maxLinks?: number;
}

export interface SurprisingConnection {
  source: GraphNode;
  target: GraphNode;
  score: number;
  reasons: string[];
  key: string;
}

export interface KnowledgeGap {
  type: "isolated-node" | "sparse-community" | "bridge-node";
  title: string;
  description: string;
  nodeIds: string[];
  suggestion: string;
}

/** Node shape used for relevance scoring (adapted from upstream RetrievalNode). */
export interface RelevanceNode {
  id: string;
  type: string;
  sources: string[];
  neighbors: Set<string>;
}

export interface RelevanceGraph {
  nodes: Map<string, RelevanceNode>;
}

export interface GraphModel {
  nodes: GraphNode[];
  edges: GraphEdge[];
  communities: CommunityInfo[];
}

/** Shape of a node as returned by the graph API (pre-enrichment). */
export interface ApiGraphNode {
  id: string;
  label: string;
  nodeType: string;
  path: string;
  linkCount: number;
  sources: string[];
}

export interface ApiGraphEdge {
  source: string;
  target: string;
  weight: number;
}
