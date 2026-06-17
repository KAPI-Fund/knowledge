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
