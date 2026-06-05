import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { listProjectReviews, updateProjectReview } from "../shared/api";

export function useProjectReviewsQuery(projectId: string) {
  return useQuery({
    queryKey: ["project-reviews", projectId],
    queryFn: () => listProjectReviews(projectId),
  });
}

export function useUpdateReviewMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: updateProjectReview,
    onSuccess: async (_result, variables) => {
      await queryClient.invalidateQueries({
        queryKey: ["project-reviews", variables.projectId],
      });
    },
  });
}
