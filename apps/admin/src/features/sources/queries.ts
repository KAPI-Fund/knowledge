import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  deleteProjectSource,
  importProjectSource,
  ingestProjectSource,
  listProjectSources,
  rescanProjectSources,
} from "../shared/api";

export function useProjectSourcesQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-sources", projectId],
    queryFn: () => listProjectSources(projectId),
    refetchInterval: 1000,
  });
}

export function useImportSourceMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: importProjectSource,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["project-sources", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-tasks", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-detail", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-audit", variables.projectId] }),
      ]);
    },
  });
}

export function useIngestSourceMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ingestProjectSource,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["project-tasks", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-detail", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-audit", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-reviews", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-graph", variables.projectId] }),
      ]);
    },
  });
}

export function useRescanSourcesMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: rescanProjectSources,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["project-sources", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-tasks", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-detail", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-audit", variables.projectId] }),
      ]);
    },
  });
}

export function useDeleteSourceMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: deleteProjectSource,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["project-sources", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-tasks", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-detail", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-audit", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-graph", variables.projectId] }),
      ]);
    },
  });
}
