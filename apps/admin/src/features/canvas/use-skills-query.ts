import { useQuery } from "@tanstack/react-query";

import { fetchSkills } from "../shared/api";

export function useSkillsQuery() {
  return useQuery({
    queryKey: ["skills"],
    queryFn: fetchSkills,
    staleTime: 5 * 60_000,
  });
}
