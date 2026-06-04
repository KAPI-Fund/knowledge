import { useMutation } from "@tanstack/react-query";

type LoginInput = {
  username: string;
  password: string;
};

export function useLoginMutation() {
  return useMutation({
    mutationFn: async (input: LoginInput) => input,
  });
}
