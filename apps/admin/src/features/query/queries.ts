import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { createQueryTask, getQueryTaskDetail } from "../shared/api";

export function useCreateQueryTaskMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: createQueryTask,
    onSuccess: async (_result, variables) => {
      await queryClient.invalidateQueries({
        queryKey: ["project-tasks", variables.projectId],
      });
    },
  });
}

export function useQueryTaskDetailQuery(projectId: string, taskId: string) {
  return useQuery({
    queryKey: ["query-task-detail", projectId, taskId],
    queryFn: () => getQueryTaskDetail({ projectId, taskId }),
    enabled: Boolean(projectId) && Boolean(taskId),
    refetchInterval: (query) => {
      const status = query.state.data?.status;
      return status && ["succeeded", "failed", "cancelled"].includes(status) ? false : 1000;
    },
  });
}
