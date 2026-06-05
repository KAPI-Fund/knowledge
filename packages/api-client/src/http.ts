import type { ZodType } from "zod";

export async function apiFetch<T>(
  path: string,
  init: RequestInit,
  schema: ZodType<T>,
): Promise<T> {
  const { headers, ...rest } = init;
  const response = await fetch(path, {
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      ...(headers ?? {}),
    },
    ...rest,
  });
  if (response.status === 204) {
    return schema.parse(undefined);
  }

  const text = await response.text();
  const json = text ? JSON.parse(text) : undefined;
  return schema.parse(json);
}
