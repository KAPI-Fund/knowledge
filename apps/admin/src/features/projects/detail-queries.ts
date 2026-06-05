import { useQuery } from "@tanstack/react-query";

import { getProjectDetail } from "../shared/api";

export function useProjectDetailQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-detail", projectId],
    queryFn: () => getProjectDetail(projectId),
  });
}
