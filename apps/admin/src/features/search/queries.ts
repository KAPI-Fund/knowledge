import { useMutation } from "@tanstack/react-query";

import { searchProject } from "../shared/api";

export function useProjectSearchMutation() {
  return useMutation({
    mutationFn: searchProject,
  });
}
