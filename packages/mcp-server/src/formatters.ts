import type {
  KnowledgeFileNode,
  KnowledgeGraphEdge,
  KnowledgeGraphNode,
  KnowledgeReviewItem,
  KnowledgeReviewsResponse,
  KnowledgeSearchResult,
} from "./api-client.js";

export function truncateText(value: string, maxBytes: number): string {
  const bytes = Buffer.byteLength(value, "utf8");
  if (bytes <= maxBytes) return value;
  let out = "";
  let used = 0;
  for (const ch of value) {
    const size = Buffer.byteLength(ch, "utf8");
    if (used + size > maxBytes) break;
    out += ch;
    used += size;
  }
  return `${out}\n\n[truncated: ${bytes - used} bytes omitted]`;
}

export function formatFileTree(files: KnowledgeFileNode[], truncated = false): string {
  if (files.length === 0) return "No files found.";
  const lines: string[] = truncated
    ? ["[warning] File tree was truncated by the knowledge-server maxFiles limit.", ""]
    : [];
  const walk = (nodes: KnowledgeFileNode[], depth: number) => {
    for (const node of nodes) {
      const prefix = "  ".repeat(depth);
      lines.push(`${prefix}${node.isDir ? "📁" : "📄"} ${node.path}`);
      if (node.children) walk(node.children, depth + 1);
    }
  };
  walk(files, 0);
  return lines.join("\n");
}

export function formatSearchResults(
  query: string,
  search: { results: KnowledgeSearchResult[]; mode?: string; tokenHits?: number; vectorHits?: number },
): string {
  const { results } = search;
  if (results.length === 0) return `No results for "${query}".`;
  const meta = [
    search.mode ? `Mode: ${search.mode}` : null,
    typeof search.tokenHits === "number" ? `Token hits: ${search.tokenHits}` : null,
    typeof search.vectorHits === "number" ? `Vector hits: ${search.vectorHits}` : null,
  ].filter(Boolean) as string[];
  const lines: string[] = [
    `# Search results for "${query}"`,
    ...(meta.length > 0 ? [meta.join(" | ")] : []),
    "",
  ];
  results.forEach((result, index) => {
    lines.push(`## ${index + 1}. ${result.title}`);
    lines.push(`Path: ${result.path}`);
    lines.push(
      `Score: ${result.score.toFixed(6)}${typeof result.vectorScore === "number" ? ` | Vector score: ${result.vectorScore.toFixed(6)}` : ""}`,
    );
    if (result.snippet) lines.push(`Snippet: ${result.snippet}`);
    if (result.images && result.images.length > 0) {
      lines.push(`Images: ${result.images.map((image) => image.url).join(", ")}`);
    }
    lines.push("");
  });
  return lines.join("\n");
}

export function formatReviews(response: KnowledgeReviewsResponse): string {
  const { reviews } = response;
  if (reviews.length === 0) return "No review items found.";
  const lines: string[] = ["# Review items", "", `Count: ${reviews.length}`, ""];
  reviews.forEach((review, index) => {
    lines.push(`## ${index + 1}. ${review.title || review.id}`);
    lines.push(`ID: ${review.id}`);
    lines.push(`Type: ${review.type}`);
    lines.push(`Status: ${review.status}`);
    if (review.sourcePath) lines.push(`Source: ${review.sourcePath}`);
    if (review.affectedPages && review.affectedPages.length > 0) {
      lines.push(`Affected pages: ${review.affectedPages.join(", ")}`);
    }
    if (review.searchQueries && review.searchQueries.length > 0) {
      lines.push(`Search queries: ${review.searchQueries.join(", ")}`);
    }
    if (review.description) lines.push(`Description: ${review.description}`);
    const optionSummary = formatReviewOptions(review);
    if (optionSummary) lines.push(`Options: ${optionSummary}`);
    lines.push("");
  });
  return lines.join("\n");
}

function formatReviewOptions(review: KnowledgeReviewItem): string {
  if (!review.options || review.options.length === 0) return "";
  return review.options
    .map((option) => (option.label ? `${option.label} (${option.action})` : option.action))
    .join(", ");
}

export function formatGraph(
  nodes: KnowledgeGraphNode[],
  edges: KnowledgeGraphEdge[],
): string {
  const typeCounts = new Map<string, number>();
  for (const node of nodes) typeCounts.set(node.type, (typeCounts.get(node.type) ?? 0) + 1);
  const lines: string[] = [
    "# Knowledge graph",
    "",
    `Nodes: ${nodes.length}`,
    `Edges: ${edges.length}`,
    "",
    "## Node types",
    ...[...typeCounts.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([type, count]) => `- ${type}: ${count}`),
    "",
    "## Top nodes",
    ...nodes
      .slice()
      .sort((a, b) => (b.linkCount ?? 0) - (a.linkCount ?? 0))
      .slice(0, 30)
      .map(
        (node) =>
          `- ${node.label} (${node.type}, ${node.linkCount ?? 0} links)${node.path ? ` — ${node.path}` : ""}`,
      ),
  ];
  return lines.join("\n");
}
