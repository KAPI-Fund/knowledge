import { useMutation } from "@tanstack/react-query";

import { login } from "../shared/api";

export function useLoginMutation() {
  return useMutation({
    mutationFn: login,
  });
}
