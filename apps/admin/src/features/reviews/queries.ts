import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { listProjectReviews, sweepProjectReviews, updateProjectReview } from "../shared/api";

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

export function useSweepReviewsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: sweepProjectReviews,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: ["project-reviews", variables.projectId],
        }),
        queryClient.invalidateQueries({
          queryKey: ["project-tasks", variables.projectId],
        }),
        queryClient.invalidateQueries({
          queryKey: ["project-audit", variables.projectId],
        }),
        queryClient.invalidateQueries({
          queryKey: ["project-detail", variables.projectId],
        }),
      ]);
    },
  });
}
