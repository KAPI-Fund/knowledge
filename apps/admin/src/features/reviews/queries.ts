import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  listProjectReviews,
  resolveProjectReviews,
  sweepProjectReviews,
  updateProjectReview,
} from "../shared/api";

export function useProjectReviewsQuery(
  projectId: string,
  input: { status: string; itemType: string; limit: number },
) {
  return useQuery({
    queryKey: ["project-reviews", projectId, input.status, input.itemType, input.limit],
    queryFn: () => listProjectReviews({ projectId, ...input }),
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

export function useResolveReviewsMutation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: resolveProjectReviews,
    onSuccess: async (_result, variables) => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["project-reviews", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-audit", variables.projectId] }),
        queryClient.invalidateQueries({ queryKey: ["project-detail", variables.projectId] }),
      ]);
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
