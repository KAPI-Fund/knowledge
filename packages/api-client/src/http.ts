import type { ZodType } from "zod";

export async function apiFetch<T>(
  path: string,
  init: RequestInit,
  schema: ZodType<T>,
): Promise<T> {
  const response = await fetch(path, {
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      ...(init.headers ?? {}),
    },
    ...init,
  });
  const json = await response.json();
  return schema.parse(json);
}
