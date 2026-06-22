import { useQuery } from "@tanstack/react-query";
import { fetchOrgMembers } from "../shared/tenancy-api";

export function useOrgMembersQuery(orgId: string) {
  return useQuery({
    queryKey: ["org-members", orgId],
    queryFn: () => fetchOrgMembers(orgId),
    enabled: Boolean(orgId),
  });
}
