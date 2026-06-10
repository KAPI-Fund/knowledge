import { useQuery } from "@tanstack/react-query";
import { z } from "zod";

import { apiFetch } from "@knowledge/api-client";
import { normalizeAppError } from "../../lib/app-error";

const meSchema = z.object({
  user: z
    .object({
      id: z.string(),
      username: z.string(),
      role: z.string(),
    })
    .nullable()
    .optional(),
});

async function getSession() {
  try {
    return await apiFetch("/api/auth/me", { method: "GET" }, meSchema);
  } catch (error) {
    const normalized = normalizeAppError(error);
    if (normalized.kind === "auth") {
      return { user: null };
    }
    throw error;
  }
}

export function useSession() {
  const session = useQuery({
    queryKey: ["session"],
    queryFn: getSession,
  });

  return { user: session.data?.user ?? null };
}
