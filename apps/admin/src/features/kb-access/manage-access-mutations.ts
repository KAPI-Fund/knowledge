import { useMutation, useQueryClient } from "@tanstack/react-query";
import { upsertGrant, removeGrant } from "../shared/tenancy-api";

export function useUpsertGrantMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { userId: string; role: "editor" | "viewer" }) =>
      upsertGrant(projectId, input),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["project-grants", projectId],
      });
    },
  });
}

export function useRemoveGrantMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { userId: string }) =>
      removeGrant(projectId, input.userId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["project-grants", projectId],
      });
    },
  });
}
