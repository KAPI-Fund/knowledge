import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createSpaceProject, createTeam } from "../shared/tenancy-api";

export function useCreateSpaceProjectMutation(orgSpaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { name: string; spaceId: string }) =>
      createSpaceProject(input),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["org-projects", orgSpaceId],
      });
      await queryClient.invalidateQueries({ queryKey: ["team-projects"] });
    },
  });
}

export function useCreateTeamMutation(orgId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { name: string; slug: string }) =>
      createTeam(orgId, input),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["org-teams", orgId] });
      await queryClient.invalidateQueries({ queryKey: ["spaces"] });
    },
  });
}
