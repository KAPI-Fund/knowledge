import { useState } from "react";
import { useParams } from "react-router-dom";

import { EmptyState } from "@/components/layout/empty-state";
import { PageSection } from "@/components/layout/page-section";
import { RouteStatePane } from "@/components/layout/route-state-pane";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select } from "@/components/ui/select";
import { normalizeAppError } from "@/lib/app-error";

import { useProjectGraphNeighborsQuery, useProjectGraphQuery } from "./queries";

export function GraphPage() {
  const { projectId = "" } = useParams();
  const [query, setQuery] = useState("");
  const [graphQuery, setGraphQuery] = useState("");
  const [nodeTypeInput, setNodeTypeInput] = useState("");
  const [graphNodeType, setGraphNodeType] = useState("");
  const [limitInput, setLimitInput] = useState("100");
  const [graphLimit, setGraphLimit] = useState(100);
  const graph = useProjectGraphQuery(projectId, graphQuery, graphNodeType, graphLimit);
  const [selectedNodeId, setSelectedNodeId] = useState("");
  const neighbors = useProjectGraphNeighborsQuery(projectId, selectedNodeId);

  function applyFilters() {
    setGraphQuery(query.trim());
    setGraphNodeType(nodeTypeInput.trim().toLowerCase());
    setGraphLimit(Number(limitInput) || 100);
  }

  const graphError = graph.error ? normalizeAppError(graph.error) : null;

  return (
    <PageSection
      description="Inspect the project graph, filter node types, and drill into neighborhood links."
      title="Graph"
    >
      <Card>
        <CardHeader>
          <CardTitle>Graph Filters</CardTitle>
          <CardDescription>Refine the graph query before loading nodes and neighbors.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-[minmax(0,1.2fr)_220px_180px_auto] md:items-end">
          <label className="grid gap-2 text-sm font-medium">
            Graph Filter
            <Input
              aria-label="Graph Filter"
              onChange={(event) => setQuery(event.target.value)}
              value={query}
            />
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Node Type
            <Select
              aria-label="Node Type"
              onChange={(event) => setNodeTypeInput(event.target.value)}
              value={nodeTypeInput}
            >
              <option value="">all</option>
              <option value="concept">concept</option>
              <option value="entity">entity</option>
              <option value="source">source</option>
              <option value="query">query</option>
              <option value="other">other</option>
            </Select>
          </label>
          <label className="grid gap-2 text-sm font-medium">
            Node Limit
            <Input
              aria-label="Node Limit"
              onChange={(event) => setLimitInput(event.target.value)}
              value={limitInput}
            />
          </label>
          <div className="flex justify-end">
            <Button onClick={applyFilters}>Apply Graph Filters</Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardContent className="flex flex-wrap gap-3 p-6 text-sm">
          <Badge variant="outline">{`Nodes: ${graph.data?.nodes.length ?? 0}`}</Badge>
          <Badge variant="outline">{`Edges: ${graph.data?.edges.length ?? 0}`}</Badge>
        </CardContent>
      </Card>

      {graphError ? (
        <RouteStatePane
          description={graphError.message}
          state={graphError.kind === "forbidden" || graphError.kind === "not_found" ? graphError.kind : "failed"}
          title="Graph unavailable"
        />
      ) : graph.isLoading && !graph.data ? (
        <RouteStatePane description="Loading project graph." state="loading" title="Graph" />
      ) : (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1.4fr)_380px]">
          <Card>
            <CardHeader>
              <CardTitle>Nodes</CardTitle>
              <CardDescription>Click a node to inspect its linked neighborhood.</CardDescription>
            </CardHeader>
            <CardContent>
              {graph.data?.nodes.length ? (
                <ScrollArea className="max-h-[680px] pr-2">
                  <ul className="grid gap-3">
                    {graph.data.nodes.map((node) => (
                      <li key={node.id}>
                        <Card>
                          <CardContent className="grid gap-2 p-4">
                            <Button
                              className="justify-start px-0 text-left"
                              onClick={() => setSelectedNodeId(node.id)}
                              variant="ghost"
                            >
                              {node.label}
                            </Button>
                            <span className="text-xs font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                              {node.id}
                            </span>
                            <span className="text-sm text-muted-foreground">{node.nodeType}</span>
                            <span className="text-sm text-muted-foreground">{node.path}</span>
                            <Badge variant="secondary">{`Links ${node.linkCount}`}</Badge>
                          </CardContent>
                        </Card>
                      </li>
                    ))}
                  </ul>
                </ScrollArea>
              ) : (
                <EmptyState
                  description="Adjust the filters and load a graph before inspecting nodes."
                  title="No graph nodes"
                />
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Neighbors</CardTitle>
              <CardDescription>Selected node neighborhood and linked sources.</CardDescription>
            </CardHeader>
            <CardContent className="grid gap-4">
              {selectedNodeId ? (
                neighbors.isLoading ? (
                  <RouteStatePane description="Loading neighbors." state="loading" title="Neighbors" />
                ) : (
                  <>
                    <div className="flex flex-wrap items-center gap-3">
                      <Badge variant="secondary">{neighbors.data?.node.label ?? selectedNodeId}</Badge>
                      <Badge variant="outline">{`Links ${neighbors.data?.node.linkCount ?? 0}`}</Badge>
                    </div>
                    {neighbors.data?.neighbors.length ? (
                      <ul className="grid gap-2">
                        {neighbors.data.neighbors.map((node) => (
                          <li
                            key={node.id}
                            className="rounded-xl border border-border/70 bg-muted/20 px-3 py-2 text-sm"
                          >
                            {`${node.label} (${node.linkCount})`}
                          </li>
                        ))}
                      </ul>
                    ) : (
                      <EmptyState
                        description="This node has no listed neighbors."
                        title="No neighbors"
                      />
                    )}
                  </>
                )
              ) : (
                <EmptyState
                  description="Select a node in the graph to inspect its neighborhood."
                  title="No node selected"
                />
              )}
            </CardContent>
          </Card>
        </div>
      )}
    </PageSection>
  );
}
