import { useMutation } from "@tanstack/react-query";

import { login, register } from "../shared/api";

export function useLoginMutation() {
  return useMutation({
    mutationFn: login,
  });
}

export function useRegisterMutation() {
  return useMutation({
    mutationFn: register,
  });
}
