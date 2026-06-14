import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { createApiToken, listApiTokens, revokeApiToken } from "../shared/api";

export function useApiTokensQuery() {
  return useQuery({
    queryKey: ["api-tokens"],
    queryFn: listApiTokens,
  });
}

function useApiTokensInvalidation() {
  const queryClient = useQueryClient();
  return async () => {
    await queryClient.invalidateQueries({ queryKey: ["api-tokens"] });
  };
}

export function useCreateApiTokenMutation() {
  const invalidate = useApiTokensInvalidation();
  return useMutation({
    mutationFn: createApiToken,
    onSuccess: async () => {
      await invalidate();
    },
  });
}

export function useRevokeApiTokenMutation() {
  const invalidate = useApiTokensInvalidation();
  return useMutation({
    mutationFn: revokeApiToken,
    onSuccess: async () => {
      await invalidate();
    },
  });
}
