import { useQuery } from "@tanstack/react-query";

import { listProjects } from "../shared/api";

export function useProjectsQuery() {
  return useQuery({
    queryKey: ["projects"],
    queryFn: listProjects,
  });
}
