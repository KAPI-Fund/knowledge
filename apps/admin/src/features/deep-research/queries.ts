import { useMutation, useQueryClient } from "@tanstack/react-query";

import { createDeepResearchTask } from "../shared/api";

export function useCreateDeepResearchTaskMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: createDeepResearchTask,
    onSuccess: async (_result, variables) => {
      await queryClient.invalidateQueries({ queryKey: ["project-tasks", variables.projectId] });
    },
  });
}
