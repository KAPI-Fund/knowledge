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
