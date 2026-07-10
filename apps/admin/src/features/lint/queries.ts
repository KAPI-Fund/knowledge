import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  createLintTask,
  deleteLintOrphan,
  dismissLintItems,
  fixLintItem,
  getProjectTaskDetail,
  listLintItems,
  sendLintItemsToReview,
} from "../shared/api";

export function useCreateLintTaskMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: createLintTask,
    onSuccess: async (_result, variables) => {
      await queryClient.invalidateQueries({
        queryKey: ["project-tasks", variables.projectId],
      });
    },
  });
}

export function useLintTaskDetailQuery(projectId: string, taskId: string) {
  return useQuery({
    queryKey: ["lint-task-detail", projectId, taskId],
    queryFn: () => getProjectTaskDetail({ projectId, taskId }),
    enabled: Boolean(projectId) && Boolean(taskId),
    refetchInterval: (query) => {
      const status = query.state.data?.status;
      return status && ["succeeded", "failed", "cancelled", "completed"].includes(status)
        ? false
        : 1000;
    },
  });
}

export function useLintItemsQuery(projectId: string) {
  return useQuery({
    queryKey: ["lint-items", projectId],
    queryFn: () => listLintItems(projectId),
    enabled: Boolean(projectId),
  });
}

export function useFixLintItemMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: fixLintItem,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["lint-items", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-files", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-reviews", variables.projectId] }),
      ]);
    },
  });
}

export function useDeleteLintOrphanMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: deleteLintOrphan,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["lint-items", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-files", variables.projectId] }),
      ]);
    },
  });
}

export function useDismissLintItemsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: dismissLintItems,
    onSuccess: async (_result, variables) => {
      await queryClient.invalidateQueries({
        queryKey: ["lint-items", variables.projectId],
      });
    },
  });
}

export function useSendLintItemsToReviewMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: sendLintItemsToReview,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["lint-items", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-reviews", variables.projectId] }),
      ]);
    },
  });
}
