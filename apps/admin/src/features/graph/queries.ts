import { useQuery } from "@tanstack/react-query";

import { getProjectGraph, getProjectGraphNeighbors } from "../shared/api";

export function useProjectGraphQuery(
  projectId: string,
  query: string,
  nodeType: string,
  limit: number,
) {
  return useQuery({
    queryKey: ["project-graph", projectId, query, nodeType, limit],
    queryFn: () => getProjectGraph({ projectId, query, nodeType, limit }),
  });
}

export function useProjectGraphNeighborsQuery(projectId: string, nodeId: string) {
  return useQuery({
    queryKey: ["project-graph-neighbors", projectId, nodeId],
    queryFn: () => getProjectGraphNeighbors({ projectId, nodeId }),
    enabled: Boolean(projectId) && Boolean(nodeId),
  });
}
