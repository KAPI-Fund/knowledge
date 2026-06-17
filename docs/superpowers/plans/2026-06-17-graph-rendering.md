# Graph Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the admin graph feature's text node-list with a full WebGL force-directed graph visualization, faithfully ported from `upstream_llm_wiki`.

**Architecture:** The backend graph API is reused almost unchanged (one additive field: node `sources`). All enrichment — relevance-weighted edges + Louvain community detection — and rendering happen client-side, matching upstream exactly. The upstream 1,562-line renderer monolith is split into focused modules (pure-logic `.ts` files + Sigma-bound `.tsx` components) to match this repo's conventions, but behavior is a faithful port.

**Tech Stack:** React 19 + TypeScript, Sigma 3 + @react-sigma/core 5 + graphology (forceatlas2, communities-louvain), Vitest + jsdom, Rust/Axum + SQLx backend.

---

## Provenance (memory directive)

Per the standing project memory directive, **all key feature logic is ported from `upstream_llm_wiki` with cited paths; nothing is invented.** Each task that ports logic cites the exact upstream source file and line range. When in doubt, open the cited upstream file and match its behavior.

## File Structure

**Backend (modify):**
- `crates/knowledge-core/src/graph.rs` — add `sources: Vec<String>` to `GraphNode` + `PageRecord`; add `extract_sources()`; populate in `collect_pages` + `materialize_graph`; add `#[cfg(test)] mod tests`.

**Shared API (modify):**
- `apps/admin/src/features/shared/api.ts` — add `sources: z.array(z.string())` to the node objects in `graphSchema` and `graphNeighborsSchema`.

**Admin deps (modify):**
- `apps/admin/package.json` — add `sigma`, `@react-sigma/core`, `graphology`, `graphology-layout-forceatlas2`, `graphology-communities-louvain`, `lucide-react`.

**Graph feature — pure logic (create), `apps/admin/src/features/graph/`:**
- `types.ts` — `GraphNode`, `GraphEdge`, `CommunityInfo`, `RetrievalNode`, filter/insight types.
- `graph-colors.ts` — color maps, `nodeColor`, `hexToRgba`, `mixColor`, density/size helpers.
- `graph-relevance.ts` — `calculateRelevance` (4-signal weighting).
- `wiki-graph.ts` — `detectCommunities`, `buildGraphModel` enrichment.
- `graph-search.ts` — `applyGraphSearch`.
- `graph-filters.ts` — `applyGraphFilters`, `DEFAULT_GRAPH_FILTERS`, structural helpers.
- `graph-insights.ts` — `findSurprisingConnections`, `detectKnowledgeGaps`.
- `graph-layout.ts` — ForceAtlas2 settings, iteration counts, worker threshold, `makeLayoutWorker`.
- `graph-layout-worker.ts` — Web Worker entry for off-main-thread layout.

**Graph feature — Sigma-bound components (create):**
- `graph-loader.tsx` — `GraphLoader`: builds the graphology graph, runs layout.
- `graph-render-settings.tsx` — `GraphRenderSettings`: node/edge reducers (hover/highlight/dim).
- `graph-events.tsx` — `EventHandler`: click/hover/right-click wiring.
- `zoom-controls.tsx` — camera zoom/unzoom/reset overlay.
- `graph-canvas.tsx` — hosts `SigmaContainer` + the above. **All Sigma/WebGL is isolated here** so tests can mock this one module.
- `graph-legend.tsx` — type + community legend.
- `graph-insights-panel.tsx` — surprising connections + knowledge gaps panel.
- `node-detail-panel.tsx` — selected node info + neighbors (reuses neighbors query).

**Graph feature — shell + queries (modify):**
- `queries.ts` — simplify `useProjectGraphQuery` to fixed `limit=1000`; keep neighbors hook.
- `page.tsx` — rewritten shell: header, controls, canvas-primary layout, side panel.

**Tests (create/modify):**
- `graph-colors.test.ts`, `graph-relevance.test.ts`, `wiki-graph.test.ts`, `graph-search.test.ts`, `graph-filters.test.ts`, `graph-insights.test.ts` — pure-logic unit tests.
- `page.test.tsx` — smoke render with `graph-canvas` + `queries` mocked.
- `apps/admin/src/features/projects/operations-pages.test.tsx` — add `vi.mock("../graph/graph-canvas")` stub + `sources: []` on mocked nodes.

---

## Task 1: Backend — add node `sources` to the graph

**Files:**
- Modify: `crates/knowledge-core/src/graph.rs`
- Test: `crates/knowledge-core/src/graph.rs` (new `#[cfg(test)] mod tests`)

Upstream source: `upstream_llm_wiki/src/lib/graph-relevance.ts` `extractFrontmatter` — parses a `sources:` YAML block (`  - item`) and an inline form (`sources: [a, b]`). We port that parser to Rust so the client can compute shared-source overlap.

- [ ] **Step 1: Write the failing tests**

Append to the end of `crates/knowledge-core/src/graph.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::extract_sources;

    #[test]
    fn extracts_block_form_sources() {
        let content = "---\ntitle: Demo\nsources:\n  - alpha.pdf\n  - beta.docx\ntype: concept\n---\nBody";
        assert_eq!(
            extract_sources(content),
            vec!["alpha.pdf".to_string(), "beta.docx".to_string()]
        );
    }

    #[test]
    fn extracts_inline_form_sources() {
        let content = "---\nsources: [alpha.pdf, \"beta.docx\"]\n---\nBody";
        assert_eq!(
            extract_sources(content),
            vec!["alpha.pdf".to_string(), "beta.docx".to_string()]
        );
    }

    #[test]
    fn returns_empty_when_no_sources() {
        let content = "---\ntitle: Demo\ntype: concept\n---\nBody";
        assert!(extract_sources(content).is_empty());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p knowledge-core extract_sources`
Expected: FAIL — `cannot find function 'extract_sources' in this scope`.

- [ ] **Step 3: Add the `extract_sources` function**

Add this function to `crates/knowledge-core/src/graph.rs` (near the other frontmatter helpers such as `extract_frontmatter_field`):

```rust
/// Parse a `sources:` frontmatter entry. Supports an inline list
/// (`sources: [a, b]`) and a YAML block list (`sources:` followed by
/// `  - item` lines). Ported from upstream graph-relevance.ts extractFrontmatter.
fn extract_sources(content: &str) -> Vec<String> {
    let mut in_block = false;
    let mut sources = Vec::new();

    for line in content.lines() {
        if in_block {
            let trimmed = line.trim_start();
            if let Some(item) = trimmed.strip_prefix('-') {
                let value = clean_source_token(item.trim());
                if !value.is_empty() {
                    sources.push(value);
                }
                continue;
            }
            // A non-indented, non-`-` line ends the block.
            if !line.starts_with(char::is_whitespace) {
                in_block = false;
            } else {
                continue;
            }
        }

        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("sources:") {
            let rest = rest.trim();
            if rest.is_empty() {
                in_block = true;
            } else if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                for token in inner.split(',') {
                    let value = clean_source_token(token.trim());
                    if !value.is_empty() {
                        sources.push(value);
                    }
                }
            } else {
                let value = clean_source_token(rest);
                if !value.is_empty() {
                    sources.push(value);
                }
            }
        }
    }

    sources
}

fn clean_source_token(token: &str) -> String {
    token
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_string()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p knowledge-core extract_sources`
Expected: PASS (3 tests).

- [ ] **Step 5: Add the `sources` field to the structs and populate it**

In `crates/knowledge-core/src/graph.rs`, add `sources` to `GraphNode` (it serializes camelCase automatically):

```rust
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub path: String,
    pub link_count: usize,
    pub sources: Vec<String>,
}
```

Add `sources` to `PageRecord`:

```rust
struct PageRecord {
    id: String,
    label: String,
    node_type: String,
    path: String,
    links: Vec<String>,
    sources: Vec<String>,
}
```

In `collect_pages`, where a `PageRecord` is constructed from file content, populate it (alongside the existing field assignments):

```rust
sources: extract_sources(&content),
```

In `materialize_graph`, where `GraphNode` is built from a `PageRecord`, add (the `page.id`/`label` clones already exist; `sources` can be moved last):

```rust
sources: page.sources,
```

- [ ] **Step 6: Verify the crate compiles clean and tests pass**

Run: `cargo test -p knowledge-core && cargo clippy -p knowledge-core --all-targets -- -D warnings`
Expected: PASS, no clippy warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/knowledge-core/src/graph.rs
git commit -m "feat(graph): expose node sources for client relevance weighting"
```

---

## Task 2: Shared API — add `sources` to the Zod graph schemas

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts`

The backend now returns `sources` on each node; the Zod schemas must accept it so the client can read it.

- [ ] **Step 1: Add `sources` to `graphSchema` nodes**

In `apps/admin/src/features/shared/api.ts`, locate `graphSchema` and add `sources` to the node object. The node object becomes:

```ts
const graphSchema = z.object({
  nodes: z.array(
    z.object({
      id: z.string(),
      label: z.string(),
      nodeType: z.string(),
      path: z.string(),
      linkCount: z.number(),
      sources: z.array(z.string()).default([]),
    }),
  ),
  edges: z.array(
    z.object({
      source: z.string(),
      target: z.string(),
      weight: z.number(),
    }),
  ),
});
```

- [ ] **Step 2: Add `sources` to `graphNeighborsSchema` nodes**

In the same file, locate `graphNeighborsSchema` and add `sources: z.array(z.string()).default([])` to the node shape used for both `node` and `neighbors[]`. If a shared inline node object is used, add the field once; otherwise add it to each node object.

- [ ] **Step 3: Type-check**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/shared` (or `npx tsc --noEmit` in `apps/admin`)
Expected: No type errors from the schema change. (`.default([])` keeps it backward-compatible if the backend response omits it.)

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/features/shared/api.ts
git commit -m "feat(graph): accept node sources in graph API schemas"
```

---

## Task 3: Add graph rendering dependencies

**Files:**
- Modify: `apps/admin/package.json`

Versions match upstream exactly (`upstream_llm_wiki/package.json`).

- [ ] **Step 1: Add dependencies**

Run from the repo root:

```bash
npm install --workspace @knowledge/admin \
  sigma@^3.0.2 \
  @react-sigma/core@^5.0.6 \
  graphology@^0.26.0 \
  graphology-layout-forceatlas2@^0.10.1 \
  graphology-communities-louvain@^2.0.2 \
  lucide-react@^1.7.0
```

- [ ] **Step 2: Verify install resolved**

Run: `npm ls --workspace @knowledge/admin sigma @react-sigma/core graphology graphology-layout-forceatlas2 graphology-communities-louvain lucide-react`
Expected: each resolves to a version satisfying the range above.

> **Note for executor:** `lucide-react@^1.7.0` is the version upstream pins. If npm cannot resolve `^1.7.0` against the registry, install the closest published version that provides the same icon set and record the actual version in the commit message — but do not substitute a different icon library.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/package.json package-lock.json
git commit -m "build(graph): add sigma, graphology, and lucide-react deps"
```

---

## Task 4: `types.ts` — shared graph types

**Files:**
- Create: `apps/admin/src/features/graph/types.ts`

Ported from `upstream_llm_wiki/src/lib/wiki-graph.ts:8-28` and `graph-relevance.ts` (RetrievalNode), `graph-filters.ts`, `graph-insights.ts`.

- [ ] **Step 1: Create the types file**

```ts
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
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/types.ts
git commit -m "feat(graph): add shared graph types"
```

---

## Task 5: `graph-colors.ts` — color maps and node sizing

**Files:**
- Create: `apps/admin/src/features/graph/graph-colors.ts`
- Test: `apps/admin/src/features/graph/graph-colors.test.ts`

Ported verbatim from `upstream_llm_wiki/src/components/graph/graph-view.tsx:23-148` — **light palette only** (the dark branch and `useResolvedDarkMode`/`MutationObserver` are dropped; this app is light-mode only).

- [ ] **Step 1: Write the failing test**

```ts
import { describe, expect, it } from "vitest";
import { hexToRgba, mixColor, nodeColor, nodeSize } from "./graph-colors";

describe("graph-colors", () => {
  it("maps known node types to palette colors", () => {
    expect(nodeColor("concept")).toBe("#c084fc");
    expect(nodeColor("entity")).toBe("#60a5fa");
  });

  it("falls back to a hashed custom color for unknown types", () => {
    expect(nodeColor("madeuptype")).toMatch(/^#[0-9a-f]{6}$/i);
  });

  it("converts hex to rgba", () => {
    expect(hexToRgba("#60a5fa", 0.5)).toBe("rgba(96,165,250,0.5)");
  });

  it("mixes two colors by ratio", () => {
    expect(mixColor("#000000", "#ffffff", 0.5)).toBe("#808080");
  });

  it("returns the base size when there are no links", () => {
    expect(nodeSize(0, 0, 10, 1)).toBe(8);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/graph-colors.test.ts`
Expected: FAIL — cannot resolve `./graph-colors`.

- [ ] **Step 3: Create `graph-colors.ts`**

```ts
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
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/graph-colors.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/graph/graph-colors.ts apps/admin/src/features/graph/graph-colors.test.ts
git commit -m "feat(graph): port node color and sizing helpers"
```

---

## Task 6: `graph-relevance.ts` — edge relevance weighting

**Files:**
- Create: `apps/admin/src/features/graph/graph-relevance.ts`
- Test: `apps/admin/src/features/graph/graph-relevance.test.ts`

Ported from `upstream_llm_wiki/src/lib/graph-relevance.ts:30-43,247-287`. The four-signal weighting (`directLink`, `sourceOverlap`, `commonNeighbor`/Adamic-Adar, `typeAffinity`) is verbatim. **Adaptation:** backend edges are undirected/merged, so `directLink` counts a present edge once (`linked ? 3 : 0`) instead of forward+backward — accepted in the design spec.

- [ ] **Step 1: Write the failing test**

```ts
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/graph-relevance.test.ts`
Expected: FAIL — cannot resolve `./graph-relevance`.

- [ ] **Step 3: Create `graph-relevance.ts`**

```ts
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
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/graph-relevance.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/graph/graph-relevance.ts apps/admin/src/features/graph/graph-relevance.test.ts
git commit -m "feat(graph): port edge relevance weighting"
```

---

## Task 7: `wiki-graph.ts` — community detection + enrichment

**Files:**
- Create: `apps/admin/src/features/graph/wiki-graph.ts`
- Test: `apps/admin/src/features/graph/wiki-graph.test.ts`

`detectCommunities` is a verbatim port of `upstream_llm_wiki/src/lib/wiki-graph.ts:31-113`. `buildGraphModel` replaces upstream `buildWikiGraph` (159-286): the file-reading first pass is gone — input is the API's nodes/edges — but the `HIDDEN_TYPES` drop, relevance weighting (255-265), community detection call, and enriched-node assembly are faithful.

- [ ] **Step 1: Write the failing test**

```ts
import { describe, expect, it } from "vitest";
import { buildGraphModel } from "./wiki-graph";
import type { ApiGraphEdge, ApiGraphNode } from "./types";

const nodes: ApiGraphNode[] = [
  { id: "a", label: "Alpha", nodeType: "concept", path: "wiki/a.md", linkCount: 2, sources: ["s1.pdf"] },
  { id: "b", label: "Beta", nodeType: "concept", path: "wiki/b.md", linkCount: 2, sources: ["s1.pdf"] },
  { id: "c", label: "Gamma", nodeType: "entity", path: "wiki/c.md", linkCount: 1, sources: [] },
  { id: "q", label: "Query", nodeType: "query", path: "wiki/q.md", linkCount: 1, sources: [] },
];
const edges: ApiGraphEdge[] = [
  { source: "a", target: "b", weight: 2 },
  { source: "b", target: "c", weight: 1 },
  { source: "a", target: "q", weight: 1 },
];

describe("buildGraphModel", () => {
  it("drops query-type nodes and edges touching them", () => {
    const model = buildGraphModel(nodes, edges);
    expect(model.nodes.map((n) => n.id).sort()).toEqual(["a", "b", "c"]);
    expect(model.edges).toHaveLength(2);
  });

  it("assigns every node a non-negative community", () => {
    const model = buildGraphModel(nodes, edges);
    expect(model.communities.length).toBeGreaterThan(0);
    for (const node of model.nodes) {
      expect(node.community).toBeGreaterThanOrEqual(0);
    }
  });

  it("recomputes edge weights via relevance scoring", () => {
    const model = buildGraphModel(nodes, edges);
    const ab = model.edges.find((e) => e.source === "a" && e.target === "b");
    // linked 3.0 + shared source 4.0 + concept-concept affinity 0.8 = 7.8
    expect(ab?.weight).toBeCloseTo(7.8, 5);
  });

  it("handles a single isolated node without throwing (no-edge guard)", () => {
    const lone: ApiGraphNode[] = [
      { id: "x", label: "Lone", nodeType: "concept", path: "wiki/x.md", linkCount: 0, sources: [] },
    ];
    const model = buildGraphModel(lone, []);
    expect(model.nodes).toHaveLength(1);
    expect(model.edges).toHaveLength(0);
    expect(model.nodes[0].community).toBe(0);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/wiki-graph.test.ts`
Expected: FAIL — cannot resolve `./wiki-graph`.

- [ ] **Step 3: Create `wiki-graph.ts`**

```ts
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
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/wiki-graph.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/graph/wiki-graph.ts apps/admin/src/features/graph/wiki-graph.test.ts
git commit -m "feat(graph): port community detection and graph enrichment"
```

---

## Task 8: `graph-search.ts` — multi-token search

**Files:**
- Create: `apps/admin/src/features/graph/graph-search.ts`
- Test: `apps/admin/src/features/graph/graph-search.test.ts`

Verbatim port of `upstream_llm_wiki/src/lib/graph-search.ts`.

- [ ] **Step 1: Write the failing test**

```ts
import { describe, expect, it } from "vitest";
import { applyGraphSearch } from "./graph-search";
import type { GraphEdge, GraphNode } from "./types";

const nodes: GraphNode[] = [
  { id: "a", label: "Alpha", type: "concept", path: "wiki/a.md", linkCount: 1, community: 0, sources: [] },
  { id: "b", label: "Beta", type: "entity", path: "wiki/b.md", linkCount: 1, community: 0, sources: [] },
];
const edges: GraphEdge[] = [{ source: "a", target: "b", weight: 1 }];

describe("applyGraphSearch", () => {
  it("returns everything with an empty matched set for a blank query", () => {
    const result = applyGraphSearch(nodes, edges, "   ");
    expect(result.nodes).toHaveLength(2);
    expect(result.matchedNodeIds.size).toBe(0);
  });

  it("matches on label and keeps only edges between visible nodes", () => {
    const result = applyGraphSearch(nodes, edges, "alpha");
    expect(result.nodes.map((n) => n.id)).toEqual(["a"]);
    expect(result.matchedNodeIds.has("a")).toBe(true);
    expect(result.edges).toHaveLength(0);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/graph-search.test.ts`
Expected: FAIL — cannot resolve `./graph-search`.

- [ ] **Step 3: Create `graph-search.ts`**

```ts
import type { GraphEdge, GraphNode } from "./types";

export interface GraphSearchResult {
  nodes: GraphNode[];
  edges: GraphEdge[];
  matchedNodeIds: Set<string>;
}

export function applyGraphSearch(
  nodes: readonly GraphNode[],
  edges: readonly GraphEdge[],
  query: string,
): GraphSearchResult {
  const tokens = query.toLowerCase().trim().split(/\s+/).filter(Boolean);

  if (tokens.length === 0) {
    return { nodes: [...nodes], edges: [...edges], matchedNodeIds: new Set() };
  }

  const matchedNodeIds = new Set<string>();
  const matchedNodes = nodes.filter((node) => {
    const haystack = [node.label, node.id, node.type, node.path].join(" ").toLowerCase();
    const matched = tokens.every((token) => haystack.includes(token));
    if (matched) matchedNodeIds.add(node.id);
    return matched;
  });

  const visibleNodeIds = new Set(matchedNodes.map((node) => node.id));
  const visibleEdges = edges.filter(
    (edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target),
  );

  return { nodes: matchedNodes, edges: visibleEdges, matchedNodeIds };
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/graph-search.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/graph/graph-search.ts apps/admin/src/features/graph/graph-search.test.ts
git commit -m "feat(graph): port multi-token graph search"
```

---

## Task 9: `graph-filters.ts` — visibility filters

**Files:**
- Create: `apps/admin/src/features/graph/graph-filters.ts`
- Test: `apps/admin/src/features/graph/graph-filters.test.ts`

Verbatim port of `upstream_llm_wiki/src/lib/graph-filters.ts`. The `shouldHideNodeType` helper (from upstream `graph-visibility.ts:1-6`) is inlined here rather than creating a separate one-function module.

- [ ] **Step 1: Write the failing test**

```ts
import { describe, expect, it } from "vitest";
import { applyGraphFilters, DEFAULT_GRAPH_FILTERS, isStructuralGraphNode } from "./graph-filters";
import type { GraphEdge, GraphNode } from "./types";

const nodes: GraphNode[] = [
  { id: "index", label: "Index", type: "overview", path: "wiki/index.md", linkCount: 9, community: 0, sources: [] },
  { id: "a", label: "Alpha", type: "concept", path: "wiki/a.md", linkCount: 2, community: 0, sources: [] },
  { id: "b", label: "Beta", type: "entity", path: "wiki/b.md", linkCount: 0, community: 1, sources: [] },
];
const edges: GraphEdge[] = [{ source: "index", target: "a", weight: 1 }];

describe("applyGraphFilters", () => {
  it("hides structural nodes by default", () => {
    expect(isStructuralGraphNode(nodes[0])).toBe(true);
    const result = applyGraphFilters(nodes, edges, DEFAULT_GRAPH_FILTERS);
    expect(result.nodes.map((n) => n.id).sort()).toEqual(["a", "b"]);
    expect(result.edges).toHaveLength(0);
  });

  it("hides isolated nodes when hideIsolated is on", () => {
    const result = applyGraphFilters(nodes, edges, {
      ...DEFAULT_GRAPH_FILTERS,
      hideIsolated: true,
    });
    expect(result.nodes.map((n) => n.id)).toEqual(["a"]);
  });

  it("hides nodes whose type is in hiddenTypes", () => {
    const result = applyGraphFilters(nodes, edges, {
      ...DEFAULT_GRAPH_FILTERS,
      hideStructural: false,
      hiddenTypes: new Set(["entity"]),
    });
    expect(result.nodes.map((n) => n.id).sort()).toEqual(["a", "index"]);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/graph-filters.test.ts`
Expected: FAIL — cannot resolve `./graph-filters`.

- [ ] **Step 3: Create `graph-filters.ts`**

```ts
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
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/graph-filters.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/graph/graph-filters.ts apps/admin/src/features/graph/graph-filters.test.ts
git commit -m "feat(graph): port graph visibility filters"
```

---

## Task 10: `graph-insights.ts` — surprising connections + knowledge gaps

**Files:**
- Create: `apps/admin/src/features/graph/graph-insights.ts`
- Test: `apps/admin/src/features/graph/graph-insights.test.ts`

Verbatim port of `upstream_llm_wiki/src/lib/graph-insights.ts`.

- [ ] **Step 1: Write the failing test**

```ts
import { describe, expect, it } from "vitest";
import { detectKnowledgeGaps, findSurprisingConnections } from "./graph-insights";
import type { CommunityInfo, GraphEdge, GraphNode } from "./types";

const nodes: GraphNode[] = [
  { id: "a", label: "Alpha", type: "source", path: "wiki/a.md", linkCount: 4, community: 0, sources: [] },
  { id: "b", label: "Beta", type: "concept", path: "wiki/b.md", linkCount: 1, community: 1, sources: [] },
  { id: "iso", label: "Lonely", type: "concept", path: "wiki/iso.md", linkCount: 0, community: 2, sources: [] },
];
const edges: GraphEdge[] = [{ source: "a", target: "b", weight: 1 }];
const communities: CommunityInfo[] = [
  { id: 0, nodeCount: 1, cohesion: 0, topNodes: ["Alpha"] },
  { id: 1, nodeCount: 1, cohesion: 0, topNodes: ["Beta"] },
  { id: 2, nodeCount: 1, cohesion: 0, topNodes: ["Lonely"] },
];

describe("graph-insights", () => {
  it("flags cross-community cross-type edges as surprising", () => {
    const result = findSurprisingConnections(nodes, edges, communities);
    expect(result).toHaveLength(1);
    expect(result[0].score).toBeGreaterThanOrEqual(3);
    expect(result[0].key).toBe("a:::b");
  });

  it("detects isolated nodes as knowledge gaps", () => {
    const gaps = detectKnowledgeGaps(nodes, edges, communities);
    const isolated = gaps.find((g) => g.type === "isolated-node");
    expect(isolated).toBeDefined();
    expect(isolated?.nodeIds).toContain("iso");
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/graph-insights.test.ts`
Expected: FAIL — cannot resolve `./graph-insights`.

- [ ] **Step 3: Create `graph-insights.ts`**

```ts
import type {
  CommunityInfo,
  GraphEdge,
  GraphNode,
  KnowledgeGap,
  SurprisingConnection,
} from "./types";

/** Find edges that connect across communities, across types, or peripheral-to-hub.
 * Verbatim port of upstream graph-insights.ts:31-102. */
export function findSurprisingConnections(
  nodes: GraphNode[],
  edges: GraphEdge[],
  _communities: CommunityInfo[],
  limit: number = 5,
): SurprisingConnection[] {
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));
  const degreeMap = new Map(nodes.map((n) => [n.id, n.linkCount]));
  const maxDegree = Math.max(...nodes.map((n) => n.linkCount), 1);

  const STRUCTURAL_IDS = new Set(["index", "log", "overview"]);

  const scored: SurprisingConnection[] = [];

  for (const edge of edges) {
    const source = nodeMap.get(edge.source);
    const target = nodeMap.get(edge.target);
    if (!source || !target) continue;
    if (STRUCTURAL_IDS.has(source.id) || STRUCTURAL_IDS.has(target.id)) continue;

    let score = 0;
    const reasons: string[] = [];

    if (source.community !== target.community) {
      score += 3;
      reasons.push("crosses community boundary");
    }

    if (source.type !== target.type) {
      const distantPairs = new Set([
        "source-concept",
        "concept-source",
        "source-synthesis",
        "synthesis-source",
        "query-entity",
        "entity-query",
      ]);
      const pair = `${source.type}-${target.type}`;
      if (distantPairs.has(pair)) {
        score += 2;
        reasons.push(`connects ${source.type} to ${target.type}`);
      } else {
        score += 1;
        reasons.push("different types");
      }
    }

    const sourceDeg = degreeMap.get(source.id) ?? 0;
    const targetDeg = degreeMap.get(target.id) ?? 0;
    const minDeg = Math.min(sourceDeg, targetDeg);
    const maxDeg = Math.max(sourceDeg, targetDeg);
    if (minDeg <= 2 && maxDeg >= maxDegree * 0.5) {
      score += 2;
      reasons.push("peripheral node links to hub");
    }

    if (edge.weight < 2 && edge.weight > 0) {
      score += 1;
      reasons.push("weak but present connection");
    }

    if (score >= 3 && reasons.length > 0) {
      const key = [source.id, target.id].sort().join(":::");
      scored.push({ source, target, score, reasons, key });
    }
  }

  scored.sort((a, b) => b.score - a.score);
  return scored.slice(0, limit);
}

/** Detect knowledge gaps: isolated nodes, sparse communities, bridge nodes.
 * Verbatim port of upstream graph-insights.ts:114-193. */
export function detectKnowledgeGaps(
  nodes: GraphNode[],
  edges: GraphEdge[],
  communities: CommunityInfo[],
  limit: number = 8,
): KnowledgeGap[] {
  const gaps: KnowledgeGap[] = [];
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));

  const isolatedNodes = nodes.filter(
    (n) => n.linkCount <= 1 && n.type !== "overview" && n.id !== "index" && n.id !== "log",
  );
  if (isolatedNodes.length > 0) {
    const topIsolated = isolatedNodes.slice(0, 5);
    gaps.push({
      type: "isolated-node",
      title: `${isolatedNodes.length} isolated page${isolatedNodes.length > 1 ? "s" : ""}`,
      description:
        topIsolated.map((n) => n.label).join(", ") +
        (isolatedNodes.length > 5 ? ` and ${isolatedNodes.length - 5} more` : ""),
      nodeIds: isolatedNodes.map((n) => n.id),
      suggestion:
        "These pages have few or no connections. Consider adding [[wikilinks]] to related pages, or research to expand their content.",
    });
  }

  for (const comm of communities) {
    if (comm.cohesion < 0.15 && comm.nodeCount >= 3) {
      gaps.push({
        type: "sparse-community",
        title: `Sparse cluster: ${comm.topNodes[0] ?? `Community ${comm.id}`}`,
        description: `${comm.nodeCount} pages with cohesion ${comm.cohesion.toFixed(2)} — internal connections are weak.`,
        nodeIds: nodes.filter((n) => n.community === comm.id).map((n) => n.id),
        suggestion: `This knowledge area lacks internal cross-references. Consider adding links between these pages or researching to fill gaps.`,
      });
    }
  }

  const communityNeighbors = new Map<string, Set<number>>();
  for (const node of nodes) {
    communityNeighbors.set(node.id, new Set());
  }
  for (const edge of edges) {
    const sourceNode = nodeMap.get(edge.source);
    const targetNode = nodeMap.get(edge.target);
    if (sourceNode && targetNode) {
      communityNeighbors.get(edge.source)?.add(targetNode.community);
      communityNeighbors.get(edge.target)?.add(sourceNode.community);
    }
  }

  const STRUCTURAL_IDS = new Set(["index", "log", "overview"]);

  const bridgeNodes = nodes
    .filter((n) => {
      if (STRUCTURAL_IDS.has(n.id)) return false;
      const neighborComms = communityNeighbors.get(n.id);
      return neighborComms && neighborComms.size >= 3;
    })
    .sort((a, b) => {
      const aComms = communityNeighbors.get(a.id)?.size ?? 0;
      const bComms = communityNeighbors.get(b.id)?.size ?? 0;
      return bComms - aComms;
    })
    .slice(0, 3);

  for (const bridge of bridgeNodes) {
    const commCount = communityNeighbors.get(bridge.id)?.size ?? 0;
    gaps.push({
      type: "bridge-node",
      title: `Key bridge: ${bridge.label}`,
      description: `Connects ${commCount} different knowledge clusters. This is a critical junction in your wiki.`,
      nodeIds: [bridge.id],
      suggestion: `This page bridges multiple knowledge areas. Ensure it's well-maintained — if it's thin, expanding it will strengthen your entire wiki.`,
    });
  }

  return gaps.slice(0, limit);
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/graph-insights.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/graph/graph-insights.ts apps/admin/src/features/graph/graph-insights.test.ts
git commit -m "feat(graph): port surprising connections and knowledge gaps"
```

---

## Task 11: `graph-layout.ts` + `graph-layout-worker.ts` — ForceAtlas2 layout

**Files:**
- Create: `apps/admin/src/features/graph/graph-layout.ts`
- Create: `apps/admin/src/features/graph/graph-layout-worker.ts`

Ported from `upstream_llm_wiki/src/components/graph/graph-view.tsx:150-207` (threshold/key/hash/worker helpers) and `graph-layout-worker.ts`. No unit test (these are layout thresholds + Web Worker plumbing exercised by the smoke test and at runtime; per `AGENTS.md` "don't write excessive tests").

- [ ] **Step 1: Create `graph-layout.ts`**

```ts
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
```

- [ ] **Step 2: Create `graph-layout-worker.ts`**

```ts
import Graph from "graphology";
import forceAtlas2 from "graphology-layout-forceatlas2";

interface LayoutMessage {
  key: string;
  nodes: Array<{ id: string; x: number; y: number }>;
  edges: Array<{ source: string; target: string; weight: number }>;
  iterations: number;
  scalingRatio: number;
}

self.onmessage = (event: MessageEvent<LayoutMessage>) => {
  const { key, nodes, edges, iterations, scalingRatio } = event.data;

  const graph = new Graph();
  for (const node of nodes) {
    graph.addNode(node.id, { x: node.x, y: node.y });
  }
  for (const edge of edges) {
    if (
      graph.hasNode(edge.source) &&
      graph.hasNode(edge.target) &&
      !graph.hasEdge(edge.source, edge.target)
    ) {
      graph.addEdge(edge.source, edge.target, { weight: edge.weight });
    }
  }

  const settings = forceAtlas2.inferSettings(graph);
  forceAtlas2.assign(graph, {
    iterations,
    settings: {
      ...settings,
      gravity: 1,
      scalingRatio,
      strongGravityMode: true,
      barnesHutOptimize: nodes.length > 50,
    },
  });

  const positions = graph.mapNodes((id, attrs) => ({
    id,
    x: attrs.x as number,
    y: attrs.y as number,
  }));

  (self as unknown as Worker).postMessage({ key, positions });
};
```

- [ ] **Step 3: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/features/graph/graph-layout.ts apps/admin/src/features/graph/graph-layout-worker.ts
git commit -m "feat(graph): port ForceAtlas2 layout helpers and worker"
```

---

## Task 12: `graph-loader.tsx` — build the graphology graph + run layout

**Files:**
- Create: `apps/admin/src/features/graph/graph-loader.tsx`

Port of `upstream_llm_wiki/src/components/graph/graph-view.tsx:211-366`. The module-level position cache, edge styling (size/alpha/lowPriority), and worker-vs-main-thread layout decision are faithful; layout helpers now come from `graph-layout.ts`.

- [ ] **Step 1: Create `graph-loader.tsx`**

```tsx
import { useEffect } from "react";
import Graph from "graphology";
import { useLoadGraph, useSigma } from "@react-sigma/core";
import { COMMUNITY_COLORS, nodeColor, nodeSize, WORKER_LAYOUT_NODE_THRESHOLD } from "./graph-colors";
import {
  edgeVisibilityThreshold,
  graphDataKey,
  layoutIterations,
  makeLayoutWorker,
  runForceLayout,
  scalingRatioFor,
} from "./graph-layout";
import type { GraphEdge, GraphNode } from "./types";

export type ColorMode = "type" | "community";

const positionCache = new Map<string, { x: number; y: number }>();
let lastLayoutDataKey = "";
let pendingLayoutDataKey = "";

interface GraphLoaderProps {
  nodes: GraphNode[];
  edges: GraphEdge[];
  colorMode: ColorMode;
  nodeScale: number;
  graphSpacing: number;
}

export function GraphLoader({ nodes, edges, colorMode, nodeScale, graphSpacing }: GraphLoaderProps) {
  const loadGraph = useLoadGraph();
  const sigma = useSigma();

  useEffect(() => {
    const dataKey = graphDataKey(nodes, edges, graphSpacing);
    const needsLayout = dataKey !== lastLayoutDataKey && dataKey !== pendingLayoutDataKey;
    let cancelled = false;
    let worker: Worker | null = null;

    const graph = new Graph();
    const maxLinks = Math.max(...nodes.map((n) => n.linkCount), 1);
    const weakEdgeThreshold = edgeVisibilityThreshold(nodes.length);

    for (const node of nodes) {
      const cached = positionCache.get(node.id);
      const color =
        colorMode === "community"
          ? COMMUNITY_COLORS[node.community % COMMUNITY_COLORS.length]
          : nodeColor(node.type);
      graph.addNode(node.id, {
        type: "circle",
        x: cached?.x ?? Math.random() * 100,
        y: cached?.y ?? Math.random() * 100,
        size: nodeSize(node.linkCount, maxLinks, nodes.length, nodeScale),
        color,
        label: node.label,
        nodeType: node.type,
        nodePath: node.path,
        community: node.community,
      });
    }

    const maxWeight = Math.max(...edges.map((e) => e.weight), 1);

    for (const edge of edges) {
      if (graph.hasNode(edge.source) && graph.hasNode(edge.target)) {
        const edgeKey = `${edge.source}->${edge.target}`;
        if (!graph.hasEdge(edgeKey) && !graph.hasEdge(`${edge.target}->${edge.source}`)) {
          const normalizedWeight = edge.weight / maxWeight;
          const size = 0.5 + normalizedWeight * 3.5;
          const alpha = Math.round(40 + normalizedWeight * 180);
          const color = `rgba(100,116,139,${alpha / 255})`;
          graph.addEdgeWithKey(edgeKey, edge.source, edge.target, {
            color,
            size,
            weight: edge.weight,
            normalizedWeight,
            sourceNode: edge.source,
            targetNode: edge.target,
            lowPriority: weakEdgeThreshold > 0 && normalizedWeight < weakEdgeThreshold,
          });
        }
      }
    }

    const runMainThreadLayout = () => {
      runForceLayout(graph, nodes.length, graphSpacing);
      lastLayoutDataKey = dataKey;
      graph.forEachNode((nodeId, attrs) => {
        positionCache.set(nodeId, { x: attrs.x as number, y: attrs.y as number });
      });
    };

    if (needsLayout && nodes.length > 1 && nodes.length < WORKER_LAYOUT_NODE_THRESHOLD) {
      runMainThreadLayout();
    }

    loadGraph(graph);

    if (needsLayout && nodes.length >= WORKER_LAYOUT_NODE_THRESHOLD) {
      worker = makeLayoutWorker();
      if (!worker) {
        runMainThreadLayout();
        loadGraph(graph);
        return undefined;
      }
      pendingLayoutDataKey = dataKey;

      worker.onmessage = (
        event: MessageEvent<{ key: string; positions: Array<{ id: string; x: number; y: number }> }>,
      ) => {
        if (cancelled || event.data.key !== dataKey) return;
        for (const { id, x, y } of event.data.positions) {
          if (!graph.hasNode(id)) continue;
          graph.setNodeAttribute(id, "x", x);
          graph.setNodeAttribute(id, "y", y);
          positionCache.set(id, { x, y });
        }
        lastLayoutDataKey = dataKey;
        if (pendingLayoutDataKey === dataKey) pendingLayoutDataKey = "";
        sigma.refresh();
      };
      worker.onerror = (event) => {
        if (cancelled) return;
        console.warn("[Graph] layout worker failed; falling back to main-thread layout:", event.message);
        if (pendingLayoutDataKey === dataKey) pendingLayoutDataKey = "";
        runMainThreadLayout();
        loadGraph(graph);
      };
      worker.postMessage({
        key: dataKey,
        nodes: nodes.map((node) => {
          const cached = positionCache.get(node.id);
          return {
            id: node.id,
            x: cached?.x ?? (graph.getNodeAttribute(node.id, "x") as number),
            y: cached?.y ?? (graph.getNodeAttribute(node.id, "y") as number),
          };
        }),
        edges: edges.map((edge) => ({ source: edge.source, target: edge.target, weight: edge.weight })),
        iterations: layoutIterations(nodes.length),
        scalingRatio: scalingRatioFor(nodes.length, graphSpacing),
      });
    }

    return () => {
      cancelled = true;
      if (pendingLayoutDataKey === dataKey) pendingLayoutDataKey = "";
      worker?.terminate();
    };
  }, [loadGraph, sigma, nodes, edges, colorMode, nodeScale, graphSpacing]);

  return null;
}
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/graph-loader.tsx
git commit -m "feat(graph): port graphology graph loader and layout wiring"
```

---

## Task 13: `graph-render-settings.tsx` — hover/highlight reducers

**Files:**
- Create: `apps/admin/src/features/graph/graph-render-settings.tsx`

Port of `upstream_llm_wiki/src/components/graph/graph-view.tsx:368-442`, using the light `GRAPH_PALETTE`.

- [ ] **Step 1: Create `graph-render-settings.tsx`**

```tsx
import { useEffect } from "react";
import { useSetSettings, useSigma } from "@react-sigma/core";
import { BASE_NODE_SIZE, GRAPH_PALETTE, mixColor } from "./graph-colors";
import { labelDensity, labelSizeThreshold } from "./graph-layout";

export type HoverState = { node: string; neighbors: Set<string> } | null;

interface GraphRenderSettingsProps {
  hoverState: HoverState;
  highlightedNodes: Set<string>;
  nodeCount: number;
}

export function GraphRenderSettings({ hoverState, highlightedNodes, nodeCount }: GraphRenderSettingsProps) {
  const sigma = useSigma();
  const setSettings = useSetSettings();

  useEffect(() => {
    setSettings({
      hideEdgesOnMove: true,
      hideLabelsOnMove: true,
      labelDensity: labelDensity(nodeCount),
      labelRenderedSizeThreshold: labelSizeThreshold(nodeCount),
      renderEdgeLabels: false,
      nodeReducer: (node, attrs) => {
        const result = { ...attrs };
        const hasHover = !!hoverState;
        const hasHighlight = highlightedNodes.size > 0;
        const isHoverNode = hoverState?.node === node;
        const isHoverNeighbor = hoverState?.neighbors.has(node) ?? false;
        const isHighlighted = highlightedNodes.has(node);

        if (isHighlighted) {
          result.size = (attrs.size ?? BASE_NODE_SIZE) * 1.5;
          result.zIndex = 10;
          result.forceLabel = true;
        }
        if (isHoverNode) {
          result.size = (attrs.size ?? BASE_NODE_SIZE) * 1.4;
          result.zIndex = 10;
          result.forceLabel = true;
        }
        if ((hasHover && !isHoverNode && !isHoverNeighbor) || (hasHighlight && !isHighlighted)) {
          result.color = mixColor(attrs.color ?? "#94a3b8", GRAPH_PALETTE.mutedNodeMixTarget, 0.75);
          result.label = "";
          result.size = (attrs.size ?? BASE_NODE_SIZE) * 0.6;
        }
        return result;
      },
      edgeReducer: (_edge, attrs) => {
        const result = { ...attrs };
        const source = String(attrs.sourceNode ?? "");
        const target = String(attrs.targetNode ?? "");
        const hasHover = !!hoverState;
        const hasHighlight = highlightedNodes.size > 0;
        const hoverEdge = hasHover && (source === hoverState?.node || target === hoverState?.node);
        const highlightedEdge =
          hasHighlight && highlightedNodes.has(source) && highlightedNodes.has(target);

        if (attrs.lowPriority && !hoverEdge && !highlightedEdge) {
          result.hidden = true;
          return result;
        }
        if ((hasHover && !hoverEdge) || (hasHighlight && !highlightedEdge)) {
          result.color = GRAPH_PALETTE.dimmedEdge;
          result.size = 0.3;
        }
        if (hoverEdge || highlightedEdge) {
          result.color = GRAPH_PALETTE.activeEdge;
          result.size = Math.max(2, (attrs.size ?? 1) * 1.5);
        }
        return result;
      },
    });
    sigma.refresh();
  }, [setSettings, sigma, hoverState, highlightedNodes, nodeCount]);

  return null;
}
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/graph-render-settings.tsx
git commit -m "feat(graph): port hover and highlight render reducers"
```

---

## Task 14: `graph-events.tsx` — click + hover wiring

**Files:**
- Create: `apps/admin/src/features/graph/graph-events.tsx`

Port of `upstream_llm_wiki/src/components/graph/graph-view.tsx:444-481`. The research right-click handler is dropped (deep-research dialog is out of scope); click + hover are faithful.

- [ ] **Step 1: Create `graph-events.tsx`**

```tsx
import { useEffect } from "react";
import { useRegisterEvents, useSigma } from "@react-sigma/core";
import type { HoverState } from "./graph-render-settings";

interface EventHandlerProps {
  onNodeClick: (nodeId: string) => void;
  onHoverChange: (state: HoverState) => void;
}

export function EventHandler({ onNodeClick, onHoverChange }: EventHandlerProps) {
  const registerEvents = useRegisterEvents();
  const sigma = useSigma();

  useEffect(() => {
    registerEvents({
      clickNode: ({ node }) => onNodeClick(node),
      enterNode: ({ node }) => {
        const container = sigma.getContainer();
        container.style.cursor = "pointer";
        const graph = sigma.getGraph();
        onHoverChange({ node, neighbors: new Set(graph.neighbors(node)) });
      },
      leaveNode: () => {
        const container = sigma.getContainer();
        container.style.cursor = "default";
        onHoverChange(null);
      },
    });
  }, [registerEvents, sigma, onNodeClick, onHoverChange]);

  return null;
}
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/graph-events.tsx
git commit -m "feat(graph): port node click and hover events"
```

---

## Task 15: `zoom-controls.tsx` — camera overlay

**Files:**
- Create: `apps/admin/src/features/graph/zoom-controls.tsx`

Port of `upstream_llm_wiki/src/components/graph/graph-view.tsx:493-533`, using this app's `Button` and `lucide-react` icons.

- [ ] **Step 1: Create `zoom-controls.tsx`**

```tsx
import { useSigma } from "@react-sigma/core";
import { Maximize, ZoomIn, ZoomOut } from "lucide-react";
import { Button } from "@/components/ui/button";

export function ZoomControls() {
  const sigma = useSigma();

  return (
    <div className="absolute top-3 right-3 flex flex-col gap-1">
      <Button
        variant="outline"
        size="icon"
        className="h-7 w-7 bg-background/80 backdrop-blur-sm"
        aria-label="Zoom in"
        onClick={() => sigma.getCamera().animatedZoom({ duration: 200 })}
      >
        <ZoomIn className="h-3.5 w-3.5" />
      </Button>
      <Button
        variant="outline"
        size="icon"
        className="h-7 w-7 bg-background/80 backdrop-blur-sm"
        aria-label="Zoom out"
        onClick={() => sigma.getCamera().animatedUnzoom({ duration: 200 })}
      >
        <ZoomOut className="h-3.5 w-3.5" />
      </Button>
      <Button
        variant="outline"
        size="icon"
        className="h-7 w-7 bg-background/80 backdrop-blur-sm"
        aria-label="Reset view"
        onClick={() => sigma.getCamera().animatedReset({ duration: 300 })}
      >
        <Maximize className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/zoom-controls.tsx
git commit -m "feat(graph): port zoom controls overlay"
```

---

## Task 16: `graph-canvas.tsx` — Sigma host (isolation boundary)

**Files:**
- Create: `apps/admin/src/features/graph/graph-canvas.tsx`

Hosts `SigmaContainer` and wires the loader, render settings, events, and zoom controls. **This is the single module that touches Sigma/WebGL** — tests mock this one file so the rest of `page.tsx` renders under jsdom. SigmaContainer settings ported from `upstream_llm_wiki/src/components/graph/graph-view.tsx:1003-1018`.

- [ ] **Step 1: Create `graph-canvas.tsx`**

```tsx
import { useState } from "react";
import { SigmaContainer } from "@react-sigma/core";
import "@react-sigma/core/lib/style.css";
import { DEFAULT_GRAPH_SPACING, DEFAULT_NODE_SCALE } from "./graph-colors";
import { GraphLoader, type ColorMode } from "./graph-loader";
import { GraphRenderSettings, type HoverState } from "./graph-render-settings";
import { EventHandler } from "./graph-events";
import { ZoomControls } from "./zoom-controls";
import type { GraphEdge, GraphNode } from "./types";

export interface GraphCanvasProps {
  nodes: GraphNode[];
  edges: GraphEdge[];
  colorMode: ColorMode;
  highlightedNodes: Set<string>;
  onNodeClick: (nodeId: string) => void;
  nodeScale?: number;
  graphSpacing?: number;
}

const SIGMA_SETTINGS = {
  defaultNodeType: "circle",
  hideEdgesOnMove: true,
  hideLabelsOnMove: true,
  renderEdgeLabels: false,
  labelSize: 13,
  labelWeight: "bold",
  stagePadding: 30,
};

export function GraphCanvas({
  nodes,
  edges,
  colorMode,
  highlightedNodes,
  onNodeClick,
  nodeScale = DEFAULT_NODE_SCALE,
  graphSpacing = DEFAULT_GRAPH_SPACING,
}: GraphCanvasProps) {
  const [hoverState, setHoverState] = useState<HoverState>(null);

  return (
    <div data-testid="graph-canvas" className="relative h-full w-full">
      <SigmaContainer style={{ height: "100%", width: "100%" }} settings={SIGMA_SETTINGS}>
        <GraphLoader
          nodes={nodes}
          edges={edges}
          colorMode={colorMode}
          nodeScale={nodeScale}
          graphSpacing={graphSpacing}
        />
        <GraphRenderSettings
          hoverState={hoverState}
          highlightedNodes={highlightedNodes}
          nodeCount={nodes.length}
        />
        <EventHandler onNodeClick={onNodeClick} onHoverChange={setHoverState} />
        <ZoomControls />
      </SigmaContainer>
    </div>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors. (If `SIGMA_SETTINGS` triggers a type error, annotate it `as const` or cast to the `@react-sigma/core` `Partial<Settings>` type imported from the package.)

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/graph-canvas.tsx
git commit -m "feat(graph): add Sigma canvas host as the WebGL isolation boundary"
```

---

## Task 17: `graph-legend.tsx` — type + community legend

**Files:**
- Create: `apps/admin/src/features/graph/graph-legend.tsx`

Port of `upstream_llm_wiki/src/components/graph/graph-view.tsx:1244-1345`, light palette only. Bottom-left overlay, collapsible. In type mode each row toggles type visibility; in community mode each row shows the community color and a cohesion warning when cohesion is low.

- [ ] **Step 1: Create `graph-legend.tsx`**

```tsx
import { useMemo, useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { COMMUNITY_COLORS, nodeColor } from "./graph-colors";
import type { CommunityInfo, GraphFilterState, GraphNode } from "./types";

interface GraphLegendProps {
  nodes: GraphNode[];
  communities: CommunityInfo[];
  colorMode: "type" | "community";
  hiddenTypes: GraphFilterState["hiddenTypes"];
  onToggleType: (type: string) => void;
  onShowAllTypes: () => void;
}

export function GraphLegend({
  nodes,
  communities,
  colorMode,
  hiddenTypes,
  onToggleType,
  onShowAllTypes,
}: GraphLegendProps) {
  const [collapsed, setCollapsed] = useState(false);

  const typeCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const node of nodes) {
      counts.set(node.type, (counts.get(node.type) ?? 0) + 1);
    }
    return [...counts.entries()].sort((a, b) => b[1] - a[1]);
  }, [nodes]);

  return (
    <div className="absolute bottom-3 left-3 w-56 rounded-md border bg-background/85 p-3 text-xs shadow-sm backdrop-blur-sm">
      <div className="flex items-center justify-between">
        <span className="font-semibold">
          {colorMode === "community" ? "Communities" : "Node Types"}
        </span>
        <div className="flex items-center gap-1">
          {colorMode === "type" && hiddenTypes.size > 0 ? (
            <button
              className="text-[11px] text-muted-foreground hover:text-foreground"
              onClick={onShowAllTypes}
              type="button"
            >
              show all
            </button>
          ) : null}
          <button
            aria-label={collapsed ? "Expand legend" : "Collapse legend"}
            className="text-muted-foreground hover:text-foreground"
            onClick={() => setCollapsed((value) => !value)}
            type="button"
          >
            {collapsed ? <ChevronUp className="h-3.5 w-3.5" /> : <ChevronDown className="h-3.5 w-3.5" />}
          </button>
        </div>
      </div>

      {collapsed ? null : (
        <ul className="mt-2 grid max-h-48 gap-1 overflow-y-auto pr-1">
          {colorMode === "community"
            ? communities.map((community) => {
                const sparse = community.cohesion < 0.15 && community.nodeCount >= 3;
                return (
                  <li key={community.id} className="flex items-center gap-2">
                    <span
                      className="inline-block h-3 w-3 shrink-0 rounded-full"
                      style={{ backgroundColor: COMMUNITY_COLORS[community.id % COMMUNITY_COLORS.length] }}
                    />
                    <span className="truncate">{community.topNodes[0] ?? `Community ${community.id}`}</span>
                    <span className="ml-auto text-muted-foreground">{community.nodeCount}</span>
                    {sparse ? (
                      <span className="text-amber-500" title="Low cohesion cluster">!</span>
                    ) : null}
                  </li>
                );
              })
            : typeCounts.map(([type, count]) => {
                const hidden = hiddenTypes.has(type);
                return (
                  <li key={type}>
                    <button
                      aria-pressed={!hidden}
                      className={`flex w-full items-center gap-2 rounded px-1 py-0.5 text-left hover:bg-muted ${hidden ? "opacity-40" : ""}`}
                      onClick={() => onToggleType(type)}
                      type="button"
                    >
                      <span
                        className="inline-block h-3 w-3 shrink-0 rounded-full"
                        style={{ backgroundColor: nodeColor(type) }}
                      />
                      <span className={`truncate ${hidden ? "line-through" : ""}`}>{type}</span>
                      <span className="ml-auto text-muted-foreground">{count}</span>
                    </button>
                  </li>
                );
              })}
        </ul>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/graph-legend.tsx
git commit -m "feat(graph): port type and community legend overlay"
```

---

## Task 18: `graph-insights-panel.tsx` — surprising connections + knowledge gaps

**Files:**
- Create: `apps/admin/src/features/graph/graph-insights-panel.tsx`

Port of `upstream_llm_wiki/src/components/graph/graph-view.tsx:1348-1469`. The deep-research button is **dropped** (deep-research dialog is out of scope). Each surprising connection / knowledge gap is clickable to highlight its nodes; each can be individually dismissed.

- [ ] **Step 1: Create `graph-insights-panel.tsx`**

```tsx
import { useState } from "react";
import { AlertTriangle, Lightbulb, Link2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { KnowledgeGap, SurprisingConnection } from "./types";

function knowledgeGapKey(gap: KnowledgeGap): string {
  return `${gap.type}:${gap.nodeIds.join(",")}`;
}

interface GraphInsightsPanelProps {
  surprising: SurprisingConnection[];
  gaps: KnowledgeGap[];
  highlightedNodes: Set<string>;
  onHighlight: (nodeIds: Set<string>) => void;
  onClose: () => void;
}

export function GraphInsightsPanel({
  surprising,
  gaps,
  highlightedNodes,
  onHighlight,
  onClose,
}: GraphInsightsPanelProps) {
  const [dismissed, setDismissed] = useState<Set<string>>(() => new Set());

  const visibleSurprising = surprising.filter((item) => !dismissed.has(item.key));
  const visibleGaps = gaps.filter((gap) => !dismissed.has(knowledgeGapKey(gap)));

  function dismiss(key: string) {
    setDismissed((prev) => {
      const next = new Set(prev);
      next.add(key);
      return next;
    });
  }

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0">
        <CardTitle className="flex items-center gap-2 text-sm">
          <Lightbulb className="h-4 w-4 text-amber-500" />
          Insights
        </CardTitle>
        <Button aria-label="Close insights" onClick={onClose} size="icon" variant="ghost" className="h-7 w-7">
          <X className="h-3.5 w-3.5" />
        </Button>
      </CardHeader>
      <CardContent className="grid gap-4 text-sm">
        <section className="grid gap-2">
          <h3 className="flex items-center gap-2 font-medium">
            <Link2 className="h-3.5 w-3.5 text-sky-500" />
            Surprising connections
          </h3>
          {visibleSurprising.length === 0 ? (
            <p className="text-xs text-muted-foreground">No surprising connections in the current view.</p>
          ) : (
            <ul className="grid gap-2">
              {visibleSurprising.map((item) => {
                const active = highlightedNodes.has(item.source.id) && highlightedNodes.has(item.target.id);
                return (
                  <li key={item.key}>
                    <div
                      className={`rounded-md border p-2 ${active ? "border-sky-400 bg-sky-50" : ""}`}
                    >
                      <div className="flex items-start justify-between gap-2">
                        <button
                          className="text-left font-medium hover:underline"
                          onClick={() => onHighlight(new Set([item.source.id, item.target.id]))}
                          type="button"
                        >
                          {item.source.label} → {item.target.label}
                        </button>
                        <button
                          aria-label="Dismiss insight"
                          className="text-muted-foreground hover:text-foreground"
                          onClick={() => dismiss(item.key)}
                          type="button"
                        >
                          <X className="h-3 w-3" />
                        </button>
                      </div>
                      <p className="mt-1 text-xs text-muted-foreground">{item.reasons.join("; ")}</p>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <section className="grid gap-2">
          <h3 className="flex items-center gap-2 font-medium">
            <AlertTriangle className="h-3.5 w-3.5 text-amber-500" />
            Knowledge gaps
          </h3>
          {visibleGaps.length === 0 ? (
            <p className="text-xs text-muted-foreground">No knowledge gaps detected in the current view.</p>
          ) : (
            <ul className="grid gap-2">
              {visibleGaps.map((gap) => {
                const key = knowledgeGapKey(gap);
                return (
                  <li key={key}>
                    <div className="rounded-md border p-2">
                      <div className="flex items-start justify-between gap-2">
                        <button
                          className="text-left font-medium hover:underline"
                          onClick={() => onHighlight(new Set(gap.nodeIds))}
                          type="button"
                        >
                          {gap.title}
                        </button>
                        <button
                          aria-label="Dismiss insight"
                          className="text-muted-foreground hover:text-foreground"
                          onClick={() => dismiss(key)}
                          type="button"
                        >
                          <X className="h-3 w-3" />
                        </button>
                      </div>
                      <p className="mt-1 text-xs text-muted-foreground">{gap.description}</p>
                      <p className="mt-1 text-xs text-muted-foreground/80">{gap.suggestion}</p>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </section>
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/graph-insights-panel.tsx
git commit -m "feat(graph): port insights panel without deep-research action"
```

---

## Task 19: `node-detail-panel.tsx` — selected node info + neighbors

**Files:**
- Create: `apps/admin/src/features/graph/node-detail-panel.tsx`

Selected-node detail with type/links/community badges and the neighbor list (reuses `useProjectGraphNeighborsQuery`). When no node is selected, shows an `EmptyState`. The neighbor objects come straight from the API and use `nodeType` (not the enriched `type`).

- [ ] **Step 1: Create `node-detail-panel.tsx`**

```tsx
import { X } from "lucide-react";
import { EmptyState } from "@/components/layout/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { nodeColor } from "./graph-colors";
import { useProjectGraphNeighborsQuery } from "./queries";
import type { GraphNode } from "./types";

interface NodeDetailPanelProps {
  projectId: string;
  node: GraphNode | null;
  onClose: () => void;
}

export function NodeDetailPanel({ projectId, node, onClose }: NodeDetailPanelProps) {
  const neighbors = useProjectGraphNeighborsQuery(projectId, node?.id ?? "");

  if (!node) {
    return (
      <EmptyState
        title="No node selected"
        description="Click a node in the graph to inspect its type, links, and neighborhood."
      />
    );
  }

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between space-y-0">
        <div className="grid gap-1">
          <CardTitle className="flex items-center gap-2 text-base">
            <span
              className="inline-block h-3 w-3 shrink-0 rounded-full"
              style={{ backgroundColor: nodeColor(node.type) }}
            />
            {node.label}
          </CardTitle>
          <CardDescription className="break-all">{node.path}</CardDescription>
        </div>
        <Button aria-label="Close node detail" onClick={onClose} size="icon" variant="ghost" className="h-7 w-7">
          <X className="h-3.5 w-3.5" />
        </Button>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="flex flex-wrap gap-2">
          <Badge variant="outline">{node.type}</Badge>
          <Badge variant="outline">{`Links: ${node.linkCount}`}</Badge>
          <Badge variant="outline">{`Community: ${node.community}`}</Badge>
        </div>

        <div className="grid gap-2">
          <span className="text-sm font-medium">Neighbors</span>
          {neighbors.isLoading ? (
            <p className="text-xs text-muted-foreground">Loading neighbors…</p>
          ) : neighbors.data && neighbors.data.neighbors.length > 0 ? (
            <ScrollArea className="max-h-72 pr-2">
              <ul className="grid gap-1">
                {neighbors.data.neighbors.map((neighbor) => (
                  <li key={neighbor.id} className="flex items-center gap-2 text-sm">
                    <span
                      className="inline-block h-2.5 w-2.5 shrink-0 rounded-full"
                      style={{ backgroundColor: nodeColor(neighbor.nodeType) }}
                    />
                    <span className="truncate">{neighbor.label}</span>
                    <span className="ml-auto text-xs text-muted-foreground">{neighbor.nodeType}</span>
                  </li>
                ))}
              </ul>
            </ScrollArea>
          ) : (
            <p className="text-xs text-muted-foreground">No linked neighbors.</p>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/node-detail-panel.tsx
git commit -m "feat(graph): add selected-node detail panel"
```

---

## Task 20: `queries.ts` — simplify graph query to a fixed full fetch

**Files:**
- Modify: `apps/admin/src/features/graph/queries.ts`

The new page fetches the whole graph once (`limit=1000`, the backend max) and does all filtering/search client-side, so `useProjectGraphQuery` no longer needs query/nodeType/limit args. The neighbors hook is unchanged.

- [ ] **Step 1: Replace the file contents**

```ts
import { useQuery } from "@tanstack/react-query";

import { getProjectGraph, getProjectGraphNeighbors } from "../shared/api";

export const GRAPH_NODE_LIMIT = 1000;

export function useProjectGraphQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-graph", projectId, GRAPH_NODE_LIMIT],
    queryFn: () => getProjectGraph({ projectId, limit: GRAPH_NODE_LIMIT }),
    enabled: Boolean(projectId),
  });
}

export function useProjectGraphNeighborsQuery(projectId: string, nodeId: string) {
  return useQuery({
    queryKey: ["project-graph-neighbors", projectId, nodeId],
    queryFn: () => getProjectGraphNeighbors({ projectId, nodeId }),
    enabled: Boolean(projectId) && Boolean(nodeId),
  });
}
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: errors only in `page.tsx` (still using the old 4-arg signature) until Task 21. The hook signature itself compiles.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/queries.ts
git commit -m "feat(graph): fetch full graph once for client-side enrichment"
```

---

## Task 21: `page.tsx` — rewrite as the graph rendering shell

**Files:**
- Modify: `apps/admin/src/features/graph/page.tsx` (full rewrite)

Replaces the node-list UI. Enriches the API graph with `buildGraphModel`, applies filters then search, computes insights, and renders the canvas-primary layout: controls row, the Sigma canvas (with the legend overlay) in the main column, and the node-detail + insights panels in the 360px side column. All graph state is local React state per the design.

- [ ] **Step 1: Replace the file contents**

```tsx
import { useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import { Filter, Lightbulb, Palette } from "lucide-react";

import { PageSection } from "@/components/layout/page-section";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { normalizeAppError } from "@/lib/app-error";

import { GraphCanvas } from "./graph-canvas";
import { GraphInsightsPanel } from "./graph-insights-panel";
import { GraphLegend } from "./graph-legend";
import { NodeDetailPanel } from "./node-detail-panel";
import { applyGraphFilters, DEFAULT_GRAPH_FILTERS } from "./graph-filters";
import { applyGraphSearch } from "./graph-search";
import { detectKnowledgeGaps, findSurprisingConnections } from "./graph-insights";
import { buildGraphModel } from "./wiki-graph";
import { useProjectGraphQuery } from "./queries";
import type { ColorMode } from "./graph-loader";
import type { GraphFilterState } from "./types";

export function GraphPage() {
  const { projectId = "" } = useParams();
  const graph = useProjectGraphQuery(projectId);

  const [search, setSearch] = useState("");
  const [filters, setFilters] = useState<GraphFilterState>(() => ({
    ...DEFAULT_GRAPH_FILTERS,
    hiddenTypes: new Set(),
    hiddenNodeIds: new Set(),
  }));
  const [colorMode, setColorMode] = useState<ColorMode>("type");
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [highlightedNodes, setHighlightedNodes] = useState<Set<string>>(() => new Set());
  const [showInsights, setShowInsights] = useState(false);

  const model = useMemo(
    () => buildGraphModel(graph.data?.nodes ?? [], graph.data?.edges ?? []),
    [graph.data],
  );
  const filtered = useMemo(
    () => applyGraphFilters(model.nodes, model.edges, filters),
    [model, filters],
  );
  const searched = useMemo(
    () => applyGraphSearch(filtered.nodes, filtered.edges, search),
    [filtered, search],
  );
  const surprising = useMemo(
    () => findSurprisingConnections(searched.nodes, searched.edges, model.communities),
    [searched, model.communities],
  );
  const gaps = useMemo(
    () => detectKnowledgeGaps(searched.nodes, searched.edges, model.communities),
    [searched, model.communities],
  );
  const selectedNode = useMemo(
    () => searched.nodes.find((node) => node.id === selectedNodeId) ?? null,
    [searched.nodes, selectedNodeId],
  );

  function toggleType(type: string) {
    setFilters((prev) => {
      const hiddenTypes = new Set(prev.hiddenTypes);
      if (hiddenTypes.has(type)) hiddenTypes.delete(type);
      else hiddenTypes.add(type);
      return { ...prev, hiddenTypes };
    });
  }

  function handleNodeClick(nodeId: string) {
    setSelectedNodeId(nodeId);
    setHighlightedNodes(new Set());
  }

  const graphError = graph.error ? normalizeAppError(graph.error) : null;

  return (
    <PageSection
      description="Explore the project knowledge graph: search, filter, and inspect node neighborhoods."
      title="Graph"
      actions={
        <div className="flex flex-wrap items-center gap-2">
          <Button
            aria-pressed={filters.hideStructural}
            onClick={() => setFilters((prev) => ({ ...prev, hideStructural: !prev.hideStructural }))}
            size="sm"
            variant={filters.hideStructural ? "default" : "outline"}
          >
            <Filter className="mr-1.5 h-3.5 w-3.5" />
            Hide structural
          </Button>
          <Button
            aria-pressed={filters.hideIsolated}
            onClick={() => setFilters((prev) => ({ ...prev, hideIsolated: !prev.hideIsolated }))}
            size="sm"
            variant={filters.hideIsolated ? "default" : "outline"}
          >
            Hide isolated
          </Button>
          <Button
            aria-pressed={colorMode === "community"}
            onClick={() => setColorMode((mode) => (mode === "community" ? "type" : "community"))}
            size="sm"
            variant={colorMode === "community" ? "default" : "outline"}
          >
            <Palette className="mr-1.5 h-3.5 w-3.5" />
            Community colors
          </Button>
          <Button
            aria-pressed={showInsights}
            onClick={() => setShowInsights((value) => !value)}
            size="sm"
            variant={showInsights ? "default" : "outline"}
          >
            <Lightbulb className="mr-1.5 h-3.5 w-3.5" />
            Insights
          </Button>
        </div>
      }
    >
      <div className="max-w-sm">
        <Input
          aria-label="Search graph"
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Search nodes by label, id, type, or path…"
          value={search}
        />
      </div>

      {graphError ? (
        <RouteStatePane
          description={graphError.message}
          state={graphError.kind === "forbidden" || graphError.kind === "not_found" ? graphError.kind : "failed"}
          title="Graph unavailable"
        />
      ) : graph.isLoading && !graph.data ? (
        <RouteStatePane description="Loading project graph." state="loading" title="Graph" />
      ) : (
        <div className="graph-layout">
          <div className="relative h-[680px] overflow-hidden rounded-lg border">
            {searched.nodes.length > 0 ? (
              <>
                <GraphCanvas
                  nodes={searched.nodes}
                  edges={searched.edges}
                  colorMode={colorMode}
                  highlightedNodes={highlightedNodes}
                  onNodeClick={handleNodeClick}
                />
                <GraphLegend
                  nodes={searched.nodes}
                  communities={model.communities}
                  colorMode={colorMode}
                  hiddenTypes={filters.hiddenTypes}
                  onToggleType={toggleType}
                  onShowAllTypes={() => setFilters((prev) => ({ ...prev, hiddenTypes: new Set() }))}
                />
              </>
            ) : (
              <div className="flex h-full items-center justify-center p-6 text-sm text-muted-foreground">
                No nodes match the current search and filters.
              </div>
            )}
          </div>

          <div className="grid gap-6">
            <NodeDetailPanel
              projectId={projectId}
              node={selectedNode}
              onClose={() => setSelectedNodeId(null)}
            />
            {showInsights ? (
              <GraphInsightsPanel
                surprising={surprising}
                gaps={gaps}
                highlightedNodes={highlightedNodes}
                onHighlight={setHighlightedNodes}
                onClose={() => setShowInsights(false)}
              />
            ) : null}
          </div>
        </div>
      )}
    </PageSection>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `npx tsc --noEmit` in `apps/admin`
Expected: no errors. (`PageSection` already accepts an `actions` prop; if not, move the control buttons into a `<div>` directly under the search `Input` instead.)

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/graph/page.tsx
git commit -m "feat(graph): render interactive force-directed graph shell"
```

---

## Task 22: Component smoke tests

**Files:**
- Create: `apps/admin/src/features/graph/page.test.tsx`
- Modify: `apps/admin/src/features/projects/operations-pages.test.tsx`

jsdom has no WebGL, so the canvas is mocked. The smoke test asserts the shell mounts, the search control is present, and node labels reach the (mocked) canvas. The operations-pages test is updated so its mocked graph data carries `sources` and its render does not try to instantiate Sigma.

- [ ] **Step 1: Create `page.test.tsx`**

```tsx
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";

vi.mock("./graph-canvas", () => ({
  GraphCanvas: ({
    nodes,
    onNodeClick,
  }: {
    nodes: { id: string; label: string }[];
    onNodeClick: (id: string) => void;
  }) => (
    <div data-testid="graph-canvas">
      {nodes.map((node) => (
        <button key={node.id} onClick={() => onNodeClick(node.id)} type="button">
          {node.label}
        </button>
      ))}
    </div>
  ),
}));

vi.mock("./queries", () => ({
  GRAPH_NODE_LIMIT: 1000,
  useProjectGraphQuery: () => ({
    data: {
      nodes: [
        { id: "a", label: "Alpha", nodeType: "concept", path: "wiki/a.md", linkCount: 2, sources: ["s1.pdf"] },
        { id: "b", label: "Beta", nodeType: "concept", path: "wiki/b.md", linkCount: 2, sources: ["s1.pdf"] },
      ],
      edges: [{ source: "a", target: "b", weight: 2 }],
    },
    isLoading: false,
    error: null,
  }),
  useProjectGraphNeighborsQuery: () => ({ data: undefined, isLoading: false, error: null }),
}));

import { GraphPage } from "./page";

function renderPage() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/projects/p1/graph"]}>
        <Routes>
          <Route element={<GraphPage />} path="/projects/:projectId/graph" />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("GraphPage", () => {
  it("mounts the canvas, search control, and node labels", () => {
    renderPage();
    expect(screen.getByRole("heading", { name: "Graph" })).toBeInTheDocument();
    expect(screen.getByTestId("graph-canvas")).toBeInTheDocument();
    expect(screen.getByLabelText("Search graph")).toBeInTheDocument();
    expect(screen.getByText("Alpha")).toBeInTheDocument();
    expect(screen.getByText("Beta")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the smoke test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/graph/page.test.tsx`
Expected: PASS (1 test). (`buildGraphModel` runs on the real enrichment path with 2 connected concept nodes; Louvain has one edge so the no-edge guard is not hit.)

- [ ] **Step 3: Update `operations-pages.test.tsx`**

Add a `graph-canvas` mock next to the existing `../graph/queries` mock so the real Sigma container never instantiates under jsdom, and add `sources: []` to the mocked graph nodes. Locate the `vi.mock("../graph/queries", ...)` block and add this mock immediately above it:

```tsx
vi.mock("../graph/graph-canvas", () => ({
  GraphCanvas: ({ nodes }: { nodes: { id: string; label: string }[] }) => (
    <div data-testid="graph-canvas">
      {nodes.map((node) => (
        <span key={node.id}>{node.label}</span>
      ))}
    </div>
  ),
}));
```

Then in the existing `useProjectGraphQuery` mock return value, add `sources: []` to each node object so it satisfies the enrichment input. The node becomes:

```tsx
{ id: "demo", label: "Demo", nodeType: "source", path: "wiki/sources/demo.md", linkCount: 1, sources: [] },
```

(The existing edge `{ source: "demo", target: "peer", weight: 1 }` references a non-existent `peer`; `buildGraphModel` drops it, leaving zero edges — the `detectCommunities` no-edge guard from Task 7 handles this without throwing.)

- [ ] **Step 4: Run the operations-pages test to verify it still passes**

Run: `npm run test --workspace @knowledge/admin -- --run src/features/projects/operations-pages.test.tsx`
Expected: PASS — the graph route still renders and `getByText("Demo")` resolves via the mocked canvas.

- [ ] **Step 5: Full admin test + type-check**

Run: `npm run test --workspace @knowledge/admin -- --run` then `npx tsc --noEmit` in `apps/admin`
Expected: all graph unit tests + both smoke tests pass; no type errors.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/graph/page.test.tsx apps/admin/src/features/projects/operations-pages.test.tsx
git commit -m "test(graph): smoke-test graph shell with Sigma canvas mocked"
```

---

## Final verification

- [ ] **Step 1: Run the full project test suite**

Run from repo root: `npm run test --workspace @knowledge/admin -- --run` and `cargo test -p knowledge-core`
Expected: all pass.

- [ ] **Step 2: Run lint**

Run: `npm run lint`
Expected: admin eslint clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.

- [ ] **Step 3: Manual UI verification (golden path)**

Start the stack (`npm run docker:up` or the dev server + backend), open a project with an ingested wiki, navigate to its Graph tab, and confirm:
- the force-directed graph renders (not a node list),
- hovering a node highlights its neighborhood and dims the rest,
- clicking a node populates the side detail panel + neighbor list,
- the search box filters nodes live,
- "Hide structural" / "Hide isolated" toggles change the rendered set,
- "Community colors" recolors nodes and switches the legend to communities,
- "Insights" shows surprising connections + knowledge gaps, and clicking one highlights its nodes,
- zoom in/out/reset controls work.

If WebGL cannot be exercised in the environment, state so explicitly rather than reporting success.

---
