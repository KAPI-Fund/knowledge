import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { cancelProjectTask, getProjectTaskDetail, listProjectTasks, retryProjectTask } from "../shared/api";

export function useProjectTasksQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-tasks", projectId],
    queryFn: () => listProjectTasks(projectId),
    refetchInterval: (query) => {
      const tasks = query.state.data ?? [];
      return tasks.some(
        (task) => !["succeeded", "failed", "cancelled", "completed"].includes(task.status),
      )
        ? 1000
        : false;
    },
  });
}

export function useTaskDetailQuery(projectId: string, taskId: string) {
  return useQuery({
    queryKey: ["project-task-detail", projectId, taskId],
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

export function useRetryTaskMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: retryProjectTask,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: ["project-tasks", variables.projectId],
        }),
        queryClient.invalidateQueries({
          queryKey: ["project-task-detail", variables.projectId, variables.taskId],
        }),
      ]);
    },
  });
}

export function useCancelTaskMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: cancelProjectTask,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: ["project-tasks", variables.projectId],
        }),
        queryClient.invalidateQueries({
          queryKey: ["project-task-detail", variables.projectId, variables.taskId],
        }),
      ]);
    },
  });
}
