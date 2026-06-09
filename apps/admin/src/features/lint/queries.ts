import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { createLintTask, getProjectTaskDetail } from "../shared/api";

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
