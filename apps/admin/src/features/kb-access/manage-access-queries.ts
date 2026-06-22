import { useQuery } from "@tanstack/react-query";
import {
  fetchProjectMembers,
  fetchOrgMembers,
  fetchTeamMembers,
} from "../shared/tenancy-api";

export function useProjectGranteesQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-grants", projectId],
    queryFn: () => fetchProjectMembers(projectId),
  });
}

export function useGrantCandidatesQuery(params: {
  spaceKind: "org" | "team";
  orgId: string;
  teamId: string | null;
}) {
  const { spaceKind, orgId, teamId } = params;
  return useQuery({
    queryKey:
      spaceKind === "team"
        ? ["grant-candidates", "team", orgId, teamId]
        : ["grant-candidates", "org", orgId],
    queryFn: async () => {
      if (spaceKind === "team" && teamId) {
        const result = await fetchTeamMembers(orgId, teamId);
        return result.members.map((m) => ({
          userId: m.userId,
          username: m.username,
        }));
      }
      const result = await fetchOrgMembers(orgId);
      return result.members.map((m) => ({
        userId: m.userId,
        username: m.username,
      }));
    },
  });
}
