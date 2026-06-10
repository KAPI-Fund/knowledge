import type { ZodType } from "zod";

export class ApiClientError extends Error {
  readonly status: number;
  readonly body: unknown;

  constructor(status: number, body: unknown, message: string) {
    super(message);
    this.name = "ApiClientError";
    this.status = status;
    this.body = body;
  }
}

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
  const json = parseResponseBody(text);

  if (!response.ok) {
    throw new ApiClientError(
      response.status,
      json,
      extractErrorMessage(json, response.statusText),
    );
  }

  return schema.parse(json);
}

function parseResponseBody(text: string) {
  if (!text) {
    return undefined;
  }

  try {
    return JSON.parse(text) as unknown;
  } catch {
    return text;
  }
}

function extractErrorMessage(body: unknown, fallback: string) {
  if (body && typeof body === "object" && "error" in body) {
    const message = body.error;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }

  if (typeof body === "string" && body.trim()) {
    return body;
  }

  return fallback || "Request failed";
}
