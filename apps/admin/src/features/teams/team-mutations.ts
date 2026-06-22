import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  addTeamMember,
  removeTeamMember,
  createSpaceProject,
} from "../shared/tenancy-api";

export function useAddTeamMemberMutation(orgId: string, teamId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { usernameOrEmail: string }) =>
      addTeamMember(orgId, teamId, input),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["team-members", orgId, teamId],
      });
    },
  });
}

export function useRemoveTeamMemberMutation(orgId: string, teamId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { userId: string }) =>
      removeTeamMember(orgId, teamId, input.userId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["team-members", orgId, teamId],
      });
    },
  });
}

export function useCreateTeamKbMutation(teamSpaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { name: string }) =>
      createSpaceProject({ name: input.name, spaceId: teamSpaceId }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["team-projects", teamSpaceId],
      });
    },
  });
}
