import { useCallback, useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import { EyeOff, Filter, Lightbulb, Palette } from "lucide-react";

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
import { detectKnowledgeGaps, findSurprisingConnections, knowledgeGapKey } from "./graph-insights";
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
  const [dismissedInsights, setDismissedInsights] = useState<Set<string>>(() => new Set());
  const [contextMenu, setContextMenu] = useState<{ id: string; x: number; y: number } | null>(null);

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
    () => findSurprisingConnections(model.nodes, model.edges, model.communities),
    [model],
  );
  const gaps = useMemo(
    () => detectKnowledgeGaps(model.nodes, model.edges, model.communities),
    [model],
  );
  const visibleSurprising = useMemo(
    () => surprising.filter((item) => !dismissedInsights.has(item.key)),
    [surprising, dismissedInsights],
  );
  const visibleGaps = useMemo(
    () => gaps.filter((gap) => !dismissedInsights.has(knowledgeGapKey(gap))),
    [gaps, dismissedInsights],
  );
  const dismissInsight = useCallback(
    (key: string, ids?: Set<string>) => {
      setDismissedInsights((prev) => new Set([...prev, key]));
      if (ids && highlightedNodes.size === ids.size && [...ids].every((id) => highlightedNodes.has(id))) {
        setHighlightedNodes(new Set());
      }
    },
    [highlightedNodes],
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

  function handleNodeContextMenu(nodeId: string, x: number, y: number) {
    setContextMenu(nodeId ? { id: nodeId, x, y } : null);
  }

  function hideNode(nodeId: string) {
    setFilters((prev) => ({ ...prev, hiddenNodeIds: new Set([...prev.hiddenNodeIds, nodeId]) }));
    setContextMenu(null);
  }

  function unhideNode(nodeId: string) {
    setFilters((prev) => {
      const next = new Set(prev.hiddenNodeIds);
      next.delete(nodeId);
      return { ...prev, hiddenNodeIds: next };
    });
  }

  const contextNode = useMemo(
    () => (contextMenu ? model.nodes.find((node) => node.id === contextMenu.id) ?? null : null),
    [contextMenu, model.nodes],
  );

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
            onClick={() =>
              setShowInsights((value) => {
                if (value) setHighlightedNodes(new Set());
                return !value;
              })
            }
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
                  onNodeContextMenu={handleNodeContextMenu}
                />
                <GraphLegend
                  nodes={model.nodes}
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

            {contextMenu && contextNode ? (
              <>
                <button
                  aria-label="Close context menu"
                  className="absolute inset-0 z-10 cursor-default"
                  onClick={() => setContextMenu(null)}
                  type="button"
                />
                <div
                  className="absolute z-20 w-48 rounded-md border bg-background py-1 text-xs shadow-lg"
                  style={{ left: contextMenu.x, top: contextMenu.y }}
                >
                  <div className="border-b px-3 py-2">
                    <div className="truncate font-medium text-foreground">{contextNode.label}</div>
                    <div className="text-muted-foreground">{contextNode.linkCount} links</div>
                  </div>
                  <button
                    className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-accent"
                    onClick={() => hideNode(contextNode.id)}
                    type="button"
                  >
                    <EyeOff className="h-3.5 w-3.5" />
                    Hide this node
                  </button>
                </div>
              </>
            ) : null}
          </div>

          <div className="grid gap-6">
            {filters.hiddenNodeIds.size > 0 ? (
              <div className="rounded-lg border p-3 text-xs">
                <div className="mb-2 font-medium text-muted-foreground">Hidden nodes</div>
                <div className="grid max-h-24 gap-1 overflow-y-auto">
                  {[...filters.hiddenNodeIds].map((nodeId) => {
                    const node = model.nodes.find((n) => n.id === nodeId);
                    return (
                      <div
                        key={nodeId}
                        className="flex items-center justify-between gap-2 rounded bg-muted/50 px-2 py-1"
                      >
                        <span className="truncate">{node?.label ?? nodeId}</span>
                        <button
                          className="text-muted-foreground hover:text-foreground"
                          onClick={() => unhideNode(nodeId)}
                          type="button"
                        >
                          Show
                        </button>
                      </div>
                    );
                  })}
                </div>
              </div>
            ) : null}
            <NodeDetailPanel
              projectId={projectId}
              node={selectedNode}
              onClose={() => setSelectedNodeId(null)}
            />
            {showInsights ? (
              <GraphInsightsPanel
                surprising={visibleSurprising}
                gaps={visibleGaps}
                highlightedNodes={highlightedNodes}
                onHighlight={setHighlightedNodes}
                onDismiss={dismissInsight}
                onClose={() => {
                  setShowInsights(false);
                  setHighlightedNodes(new Set());
                }}
              />
            ) : null}
          </div>
        </div>
      )}
    </PageSection>
  );
}
