import { z } from "zod";

export const currentUserSchema = z.object({
  id: z.string(),
  username: z.string(),
  role: z.string(),
});

export function parseCurrentUser(input: unknown) {
  return currentUserSchema.parse(input);
}
