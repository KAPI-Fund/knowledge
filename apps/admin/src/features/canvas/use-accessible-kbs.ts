import { useQueries } from "@tanstack/react-query";

import { fetchOrgProjects } from "../shared/tenancy-api";
import { useSpacesQuery } from "../spaces/use-spaces";
import type { KbItem } from "./kb-grouping";

interface Source {
  spaceId: string;
  /** Index of the owning org in the spaces list; -1 for the personal space. */
  orgIndex: number;
}

interface AccessibleKnowledgeBases {
  items: KbItem[];
  isLoading: boolean;
  isError: boolean;
}

/**
 * Aggregate every knowledge base the caller can see across all of their spaces.
 *
 * `/api/projects` is space-scoped: without a spaceId it only returns the caller's
 * personal projects. To surface org- and team-level knowledge bases we fan out one
 * project query per space (personal + each org). An org space already includes its
 * teams' projects via the backend UNION, so we group the returned projects by their
 * own `spaceKind`: personal -> "Personal", org -> the org name, team -> the team name.
 */
export function useAccessibleKnowledgeBases(): AccessibleKnowledgeBases {
  const spaces = useSpacesQuery();
  const data = spaces.data;

  const sources: Source[] = [];
  if (data) {
    if (data.personal.spaceId) {
      sources.push({ spaceId: data.personal.spaceId, orgIndex: -1 });
    }
    data.orgs.forEach((org, index) => sources.push({ spaceId: org.spaceId, orgIndex: index }));
  }

  const results = useQueries({
    queries: sources.map((source) => ({
      queryKey: ["kb-projects", source.spaceId],
      queryFn: () => fetchOrgProjects(source.spaceId),
      enabled: Boolean(data),
    })),
  });

  const teamsById = new Map((data?.teams ?? []).map((team) => [team.id, team]));

  const items: KbItem[] = [];
  const seen = new Set<string>();
  results.forEach((result, index) => {
    const source = sources[index];
    for (const project of result.data ?? []) {
      if (seen.has(project.id)) continue;
      seen.add(project.id);

      let groupKey: string;
      let groupLabel: string;
      let groupOrder: number;
      if (project.spaceKind === "team") {
        const team = project.teamId ? teamsById.get(project.teamId) : undefined;
        groupKey = `team:${project.teamId}`;
        groupLabel = team?.name ?? project.teamSlug ?? "Team";
        // Cluster a team right after its owning org group.
        groupOrder = source.orgIndex + 1.5;
      } else if (project.spaceKind === "org") {
        const org = data?.orgs[source.orgIndex];
        groupKey = `org:${org?.id ?? source.spaceId}`;
        groupLabel = org?.name ?? "Organization";
        groupOrder = source.orgIndex + 1;
      } else {
        groupKey = "personal";
        groupLabel = "Personal";
        groupOrder = 0;
      }

      items.push({
        id: project.id,
        name: project.name,
        role: project.role,
        groupKey,
        groupLabel,
        groupOrder,
      });
    }
  });

  return {
    items,
    isLoading: spaces.isLoading || results.some((result) => result.isLoading),
    isError: spaces.isError || results.some((result) => result.isError),
  };
}
