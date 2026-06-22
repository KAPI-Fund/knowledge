import { useQuery } from "@tanstack/react-query";
import { fetchOrgProjects, fetchOrgTeams } from "../shared/tenancy-api";

export function useOrgProjectsQuery(orgSpaceId: string) {
  return useQuery({
    queryKey: ["org-projects", orgSpaceId],
    queryFn: () => fetchOrgProjects(orgSpaceId),
    enabled: Boolean(orgSpaceId),
  });
}

export function useOrgTeamsQuery(orgId: string) {
  return useQuery({
    queryKey: ["org-teams", orgId],
    queryFn: () => fetchOrgTeams(orgId),
    enabled: Boolean(orgId),
  });
}
