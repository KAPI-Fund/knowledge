export const NODE_TYPE_COLORS: Record<string, string> = {
  entity: "#60a5fa",
  concept: "#c084fc",
  source: "#fb923c",
  query: "#4ade80",
  synthesis: "#f87171",
  overview: "#facc15",
  comparison: "#2dd4bf",
  finding: "#a855f7",
  thesis: "#f43f5e",
  methodology: "#14b8a6",
  other: "#94a3b8",
};

export const CUSTOM_NODE_COLORS = [
  "#38bdf8",
  "#34d399",
  "#fbbf24",
  "#fb7185",
  "#a78bfa",
  "#22d3ee",
  "#f97316",
  "#84cc16",
];

export const COMMUNITY_COLORS = [
  "#60a5fa",
  "#4ade80",
  "#fb923c",
  "#c084fc",
  "#f87171",
  "#2dd4bf",
  "#facc15",
  "#f472b6",
  "#a78bfa",
  "#38bdf8",
  "#34d399",
  "#fbbf24",
];

export const BASE_NODE_SIZE = 8;
export const MAX_NODE_SIZE = 28;
export const DEFAULT_NODE_SCALE = 1;
export const DEFAULT_GRAPH_SPACING = 1;
export const WORKER_LAYOUT_NODE_THRESHOLD = 220;

export interface GraphThemePalette {
  defaultEdge: string;
  label: string;
  mutedNodeMixTarget: string;
  dimmedEdge: string;
  activeEdge: string;
}

/** Light palette only (upstream graph-view.tsx:90-96). */
export const GRAPH_PALETTE: GraphThemePalette = {
  defaultEdge: "#cbd5e1",
  label: "#1e293b",
  mutedNodeMixTarget: "#e2e8f0",
  dimmedEdge: "rgba(148,163,184,0.22)",
  activeEdge: "#1e293b",
};

export function nodeColor(type: string): string {
  if (NODE_TYPE_COLORS[type]) return NODE_TYPE_COLORS[type];
  let hash = 0;
  for (const char of type) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return CUSTOM_NODE_COLORS[hash % CUSTOM_NODE_COLORS.length] ?? NODE_TYPE_COLORS.other;
}

export function hexToRgba(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r},${g},${b},${alpha})`;
}

export function mixColor(color1: string, color2: string, ratio: number): string {
  const hex = (c: string) => parseInt(c, 16);
  const r1 = hex(color1.slice(1, 3)),
    g1 = hex(color1.slice(3, 5)),
    b1 = hex(color1.slice(5, 7));
  const r2 = hex(color2.slice(1, 3)),
    g2 = hex(color2.slice(3, 5)),
    b2 = hex(color2.slice(5, 7));
  const r = Math.round(r1 + (r2 - r1) * ratio);
  const g = Math.round(g1 + (g2 - g1) * ratio);
  const b = Math.round(b1 + (b2 - b1) * ratio);
  return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

export function graphDensityScale(nodeCount: number): number {
  if (nodeCount <= 150) return 1;
  return Math.max(0.35, Math.sqrt(150 / nodeCount));
}

export function nodeSize(
  linkCount: number,
  maxLinks: number,
  nodeCount: number,
  userScale: number,
): number {
  if (maxLinks === 0) return BASE_NODE_SIZE;
  const ratio = linkCount / maxLinks;
  const size = BASE_NODE_SIZE + Math.sqrt(ratio) * (MAX_NODE_SIZE - BASE_NODE_SIZE);
  return size * graphDensityScale(nodeCount) * userScale;
}
