import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  addOrgMember,
  setOrgMemberRole,
  removeOrgMember,
} from "../shared/tenancy-api";

export function useAddOrgMemberMutation(orgId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: {
      usernameOrEmail: string;
      role: "org_admin" | "org_member";
    }) => addOrgMember(orgId, input),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["org-members", orgId] });
    },
  });
}

export function useSetOrgMemberRoleMutation(orgId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: {
      userId: string;
      role: "org_admin" | "org_member";
    }) => setOrgMemberRole(orgId, input.userId, input.role),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["org-members", orgId] });
    },
  });
}

export function useRemoveOrgMemberMutation(orgId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { userId: string }) =>
      removeOrgMember(orgId, input.userId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["org-members", orgId] });
    },
  });
}
