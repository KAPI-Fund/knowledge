import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  detectProjectDuplicates,
  dismissProjectDuplicateGroup,
  getProjectDedup,
  mergeProjectDuplicateGroup,
} from "../shared/api";

export function useProjectDedupQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-dedup", projectId],
    queryFn: () => getProjectDedup(projectId),
    refetchInterval: 3000,
  });
}

function useDedupInvalidation() {
  const queryClient = useQueryClient();
  return async (projectId: string) => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["project-dedup", projectId] }),
      queryClient.invalidateQueries({ queryKey: ["project-tasks", projectId] }),
    ]);
  };
}

export function useDetectDuplicatesMutation() {
  const invalidate = useDedupInvalidation();
  return useMutation({
    mutationFn: detectProjectDuplicates,
    onSuccess: async (_result, variables) => {
      await invalidate(variables.projectId);
    },
  });
}

export function useMergeDuplicateGroupMutation() {
  const invalidate = useDedupInvalidation();
  return useMutation({
    mutationFn: mergeProjectDuplicateGroup,
    onSuccess: async (_result, variables) => {
      await invalidate(variables.projectId);
    },
  });
}

export function useDismissDuplicateGroupMutation() {
  const invalidate = useDedupInvalidation();
  return useMutation({
    mutationFn: dismissProjectDuplicateGroup,
    onSuccess: async (_result, variables) => {
      await invalidate(variables.projectId);
    },
  });
}
