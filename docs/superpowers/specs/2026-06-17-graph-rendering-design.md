# Graph Rendering Design

## Summary

The admin graph feature (`apps/admin/src/features/graph/`) currently renders only a text list of nodes — there is no rendered graph. Edges are fetched but unused. This design adds a full WebGL force-directed graph visualization, faithfully ported from the upstream reference renderer in `upstream_llm_wiki`.

The port reuses the existing backend graph API unchanged. All graph enrichment (relevance weighting + community detection) and rendering happen client-side, matching upstream exactly. The upstream renderer is a single 1,562-line file; this design splits it into focused modules to match this repo's conventions, but the behavior is a faithful port — not a reinvention.

Per the project memory directive, all key feature logic is ported from `upstream_llm_wiki` with cited paths; nothing is invented.

## Goals

- Replace the node list with a rendered, interactive force-directed graph as the primary view.
- Port the upstream renderer's full feature set: force layout, zoom/pan, hover + click highlighting, search, filters, type **and** community coloring, legend, and the insights panel.
- Compute community detection (Louvain) and richer edge relevance weights client-side.
- Keep the backend graph API contract unchanged (no Rust changes required for the core port).
- Keep the implementation faithful to upstream behavior while splitting the monolith into small, testable modules.

## Non-Goals

- No backend/Rust changes to the graph endpoints for the core port (one possible exception noted under Risks).
- No dark mode. The admin app is light-mode only (no `.dark` class, no `prefers-color-scheme` anywhere in `apps/admin/src`), so the upstream dark/light switching + `MutationObserver` machinery is not ported.
- No `zustand`. Upstream uses a zustand store; this app uses TanStack Query + local React state, and the graph UI state is page-local.
- No new graph data semantics beyond what upstream computes.

## Current State

**Frontend** (`apps/admin/src/features/graph/`):
- `page.tsx` (189 lines) — text/list UI only. Uses `useProjectGraphQuery()` and `useProjectGraphNeighborsQuery()`. Renders nodes as a card list; selected node's neighbors as a text list. Edges are fetched but unused (only a count badge).
- `queries.ts` (23 lines) — two TanStack Query hooks: `useProjectGraphQuery(projectId, query, nodeType, limit)` and `useProjectGraphNeighborsQuery(projectId, nodeId)`.
- API client funcs live in `apps/admin/src/features/shared/api.ts` (not in `packages/api-client`):
  - `getProjectGraph({projectId, query?, nodeType?, limit?})` → GET `/api/projects/{id}/graph?q=&nodeType=&limit=`, validated against `graphSchema`.
  - `getProjectGraphNeighbors({projectId, nodeId})` → GET `/api/projects/{id}/graph/{nodeId}/neighbors`, validated against `graphNeighborsSchema`.
- No visualization library is installed (no sigma/graphology/d3/cytoscape/reactflow).
- `index.css:131` already defines `.graph-layout` as `grid gap-6 xl:grid-cols-[minmax(0,1.3fr)_360px]` — the canvas-primary layout (wide canvas + 360px side panel).

**Backend** (already serving the data contract):
- `crates/knowledge-server/src/projects/routes.rs` — graph routes (lines 85-89): `graph_handler` (730-758) calls `build_graph_view(root, query, node_type, limit)`; `graph_neighbors_handler` calls `neighbors_for_node`. `GraphRequest`: `q`, `nodeType`, `limit` (default 100, max 1000).
- `crates/knowledge-core/src/graph.rs` — `GraphNode { id, label, node_type, path, link_count }`, `GraphEdge { source, target, weight }`. `build_graph_view()` reads `.md` from `wiki/`. Key behaviors:
  - Edges are **undirected**: `ordered_edge_key` (313-319) sorts the pair so A→B and B→A merge into one edge; `weight` = total wikilink count between the pair (127).
  - Structural pages (`index, log, overview, schema, purpose`) are **skipped at collection** (`should_skip_page` 240-242) and never reach the client.
  - Nodes are sorted by `link_count` DESC; truncation to the limit only applies when a project has **more nodes than the cap** (190-195). Edges among the returned nodes are kept complete (no edge is dropped below a weight threshold).

Current Zod contract (`apps/admin/src/features/shared/api.ts`):
```ts
const graphSchema = z.object({
  nodes: z.array(z.object({ id, label, nodeType, path, linkCount })),
  edges: z.array(z.object({ source, target, weight })),
});
// graphNeighborsSchema: { node, neighbors: node[] }
```

## Upstream Reference (sources to port)

- `upstream_llm_wiki/src/components/graph/graph-view.tsx` (1,562 lines) — the renderer. Inner components: `GraphLoader` (216-366), `GraphRenderSettings` (368-442), `EventHandler` (444-481), `ZoomControls` (493-533). Color maps + sizing (23-148). Sigma settings (1003-1018). Legend (1244-1345). Insights panel (1348-1469).
- `upstream_llm_wiki/src/lib/wiki-graph.ts` — `buildWikiGraph()` (159-286): relevance weighting + Louvain. Types: `GraphNode { id, label, type, path, linkCount, community }` (8-15), `GraphEdge { source, target, weight }` (17-21), `CommunityInfo { id, nodeCount, cohesion, topNodes }` (23-28).
- `upstream_llm_wiki/src/lib/graph-search.ts` — `applyGraphSearch(nodes, edges, query)`: multi-token AND on label/id/type/path.
- `upstream_llm_wiki/src/lib/graph-filters.ts` — `applyGraphFilters`: hiddenNodeIds, hiddenTypes, hideStructural, hideIsolated, maxLinks.
- `upstream_llm_wiki/package.json` (lines 25, 36-38, 54) — dependency versions.

## Architecture

### Dependencies (exact upstream versions)

Add to `apps/admin/package.json`:
- `sigma` `^3.0.2`
- `@react-sigma/core` `^5.0.6`
- `graphology` `^0.26.0`
- `graphology-layout-forceatlas2` `^0.10.1`
- `graphology-communities-louvain` `^2.0.2`

Not ported: `zustand` (use React state), upstream dark/light theme deps.

### Data flow & enrichment (client-side)

1. `page.tsx` fetches the full graph via `useProjectGraphQuery` with `limit=1000` (backend max), so the layout is not truncated.
2. The API returns `nodes[]` + raw-wikilink-count `edges[]`. The client treats these edges as the raw wikilink adjacency.
3. `wiki-graph.ts` (port of `upstream_llm_wiki/src/lib/wiki-graph.ts:159-286`) recomputes **relevance weights** from the adjacency + node types:
   - direct link: +3.0
   - shared-source overlap: +4.0 per shared source-type neighbor
   - common-neighbor / Adamic-Adar: +1.5 (weighted by neighbor degree)
   - type affinity: +1.0 for same/related type
4. Run **Louvain** via `graphology-communities-louvain` to assign each node a `community`, and derive `CommunityInfo` (nodeCount, cohesion, topNodes).
5. Output enriched `{ nodes(+community), edges(relevanceWeight) }` for rendering and the insights panel.

This is the same algorithm and the same client-side placement as upstream; only the data source differs (REST API response instead of Tauri file reads).

### Module / component architecture (`apps/admin/src/features/graph/`)

- `types.ts` — `GraphNode`, `GraphEdge`, `CommunityInfo` (port of `wiki-graph.ts:8-28`).
- `graph-colors.ts` — `NODE_TYPE_COLORS`, `CUSTOM_NODE_COLORS` (hash fallback), `COMMUNITY_COLORS` (12, indexed `community % 12`), node sizing (`BASE_NODE_SIZE=8`, `MAX_NODE_SIZE=28`, `size = 8 + sqrt(linkCount/maxLinks)*20`, density- and user-scaled). Light palette only (port of `graph-view.tsx:23-148`, dark branch dropped).
- `wiki-graph.ts` — enrichment (relevance weights + Louvain), adapted to API input (port of `lib/wiki-graph.ts`).
- `graph-search.ts` — `applyGraphSearch` (port of `lib/graph-search.ts`).
- `graph-filters.ts` — `applyGraphFilters` (port of `lib/graph-filters.ts`).
- `graph-layout.ts` — ForceAtlas2 settings (`inferSettings`, `gravity:1`, `scalingRatio` by node count, `strongGravityMode:true`, `barnesHutOptimize` when n>50), iteration counts (>2500→28, >1200→40, >600→65, >250→90, else 140), and Web Worker threshold (n≥220). Position caching. (Port of `graph-view.tsx:285-356`.)
- `graph-canvas.tsx` — hosts `SigmaContainer` from `@react-sigma/core` with settings (`defaultNodeType:"circle"`, `hideEdgesOnMove:true`, `hideLabelsOnMove:true`, `labelSize:13`, `stagePadding:30`). (Port of `graph-view.tsx` SigmaContainer + 1003-1018.)
- `graph-loader.tsx` — `GraphLoader`: builds the graphology graph from enriched data (node `addNode` with type/x/y/size/color/label/nodeType/nodePath/community; edges with size `0.5 + normalizedWeight*3.5`, alpha `40 + normalizedWeight*180`, color `rgba(100,116,139,…)`, `lowPriority` below weak-edge threshold), runs layout via `graph-layout.ts`. (Port of 216-366.)
- `graph-render-settings.tsx` — `GraphRenderSettings`: node/edge reducers for hover/highlight/dim. (Port of 368-442.)
- `graph-events.tsx` — `EventHandler`: click (select), hover (highlight neighborhood + dim rest), right-click. (Port of 444-481.)
- `zoom-controls.tsx` — camera zoom/unzoom/reset overlay. (Port of 493-533.)
- `graph-legend.tsx` — type + community legend, bottom-left. (Port of 1244-1345.)
- `graph-insights.tsx` — surprising connections + knowledge gaps. (Port of 1348-1469.)
- `node-detail-panel.tsx` — selected node info + neighbors; reuses `useProjectGraphNeighborsQuery`.
- `page.tsx` — rewritten shell: header + project context, search/filter/coloring controls, canvas-primary layout via existing `.graph-layout`, side detail panel. Replaces the list.
- `queries.ts` — keep both hooks; default fetch with `limit=1000`.

### Rendering

WebGL via Sigma 3. Colors use the community palette when community-coloring is on, else the type palette, with the hash fallback for unknown types. Edge appearance scales with normalized relevance weight. ForceAtlas2 runs synchronously for small graphs and in a Web Worker (`graphology-layout-forceatlas2/worker`) at n≥220; Barnes-Hut optimization at n>50; iteration count scales down as node count grows; computed positions are cached to avoid relayout churn.

### UI / UX

Canvas-primary layout (existing `.graph-layout`): the graph fills the main column; the 360px side panel shows selected-node detail + neighbors. Controls above/over the canvas:
- search box (multi-token AND over label/id/type/path),
- filter toggles (hide types / isolated / structural),
- community-coloring toggle,
- zoom controls overlay,
- legend (bottom-left),
- insights panel.

Interactions: click selects a node (populates the side panel), hover highlights its neighborhood and dims the rest.

### State management

Local React state in `page.tsx` (`useState`/`useReducer`):
`{ search, filters: { hiddenTypes, hideIsolated, hideStructural }, selectedNodeId, hoveredNodeId, useCommunityColors, scale }`. No global store.

### Theming

Light palette only, sourced from upstream's light branch. No `MutationObserver`, no `dark` class detection.

## Testing

Per `AGENTS.md` ("don't write excessive tests"), test the pure logic, not WebGL:
- Vitest unit tests for `wiki-graph.ts` (relevance weights + community assignment on a small fixture graph), `graph-search.ts`, `graph-filters.ts`, and the color/size functions in `graph-colors.ts`.
- jsdom has no WebGL, so component tests stay light: a smoke render of `page.tsx` with Sigma/canvas mocked, asserting controls and the side panel mount. No snapshot expansion.

## Performance

`hideEdgesOnMove` + `hideLabelsOnMove` during interaction, Web Worker layout at scale, Barnes-Hut for larger graphs, scaled iteration counts, and position caching — all ported from upstream. Backend cap is 1,000 nodes.

## Risks / Open items (verify during implementation)

- **Backend graph completeness (verified, low risk):** `materialize_graph()` keeps all edges among returned nodes (no per-edge weight threshold) and only truncates nodes when a project exceeds the 1,000-node cap. For projects ≤1,000 nodes (the expected case) the full graph reaches the client, so client-side shared-source-overlap weighting is faithful. For very large projects, the top-degree subgraph is returned; source nodes are typically high-degree and survive. No backend change is needed for the common case.
- **Structural filter parity:** the backend already drops the canonical structural IDs (`index, log, overview, schema, purpose`) at collection, so the ported `hideStructural` filter is largely redundant for those IDs. It is still ported for parity with upstream (it may catch structural nodes the backend does not).
- **Undirected edges:** backend edges are undirected/merged; the client enrichment and rendering treat the graph as undirected, matching upstream's relevance graph.
- **Vite worker bundling:** confirm `graphology-layout-forceatlas2/worker` bundles cleanly under this app's Vite 7 config; fall back to synchronous layout if a worker import issue appears.
- **Zod contract:** the current `graphSchema` already covers the fields the client needs; `community`/`relevanceWeight` are computed client-side and are not part of the API contract.

## Out of Scope

- Dark mode.
- Backend relevance-weighting / Louvain (kept client-side per the chosen approach).
- Editing the graph (this is read-only visualization, matching upstream's view).
