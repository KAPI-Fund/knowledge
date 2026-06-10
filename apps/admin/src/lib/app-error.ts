import { ApiClientError } from "@knowledge/api-client";

export type AppErrorKind = "auth" | "forbidden" | "not_found" | "failed";

export type AppError = {
  kind: AppErrorKind;
  status: number;
  message: string;
};

export function normalizeAppError(error: unknown): AppError {
  if (error instanceof ApiClientError) {
    if (error.status === 401) {
      return { kind: "auth", status: 401, message: error.message };
    }
    if (error.status === 403) {
      return { kind: "forbidden", status: 403, message: error.message };
    }
    if (error.status === 404 || isUnknownProject(error) || isUnknownTask(error)) {
      return { kind: "not_found", status: error.status, message: error.message };
    }

    return { kind: "failed", status: error.status, message: error.message };
  }

  if (error instanceof Error) {
    return { kind: "failed", status: 500, message: error.message };
  }

  return { kind: "failed", status: 500, message: "Unexpected request failure" };
}

function isUnknownProject(error: ApiClientError) {
  return error.status === 400 && error.message === "unknown project";
}

function isUnknownTask(error: ApiClientError) {
  return error.status === 400 && error.message === "unknown task";
}
