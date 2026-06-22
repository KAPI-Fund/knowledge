import { useQuery } from "@tanstack/react-query";
import { fetchTeamMembers, fetchOrgProjects } from "../shared/tenancy-api";

export function useTeamMembersQuery(orgId: string, teamId: string) {
  return useQuery({
    queryKey: ["team-members", orgId, teamId],
    queryFn: () => fetchTeamMembers(orgId, teamId),
    enabled: Boolean(orgId) && Boolean(teamId),
  });
}

export function useTeamProjectsQuery(teamSpaceId: string, teamId: string) {
  return useQuery({
    queryKey: ["team-projects", teamSpaceId],
    queryFn: async () => {
      const result = await fetchOrgProjects(teamSpaceId);
      return result.filter((project) => project.teamId === teamId);
    },
    enabled: Boolean(teamSpaceId),
  });
}
