import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  getProjectSourceWatchSettings,
  scanProjectSourceWatch,
  updateProjectSourceWatchSettings,
} from "../shared/api";

export function useProjectSourceWatchQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-source-watch", projectId],
    queryFn: () => getProjectSourceWatchSettings(projectId),
  });
}

export function useUpdateProjectSourceWatchMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: updateProjectSourceWatchSettings,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["project-source-watch", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-audit", variables.projectId] }),
      ]);
    },
  });
}

export function useScanProjectSourceWatchMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: scanProjectSourceWatch,
    onSuccess: async (_result, projectId) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["project-source-watch", projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-sources", projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-tasks", projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-audit", projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-detail", projectId] }),
      ]);
    },
  });
}
