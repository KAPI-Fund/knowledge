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
