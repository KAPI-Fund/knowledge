import { apiFetch } from "@knowledge/api-client";
import { z } from "zod";

import { getCsrfToken } from "../auth/csrf";
import {
  canvasResponseSchema,
  canvasSummarySchema,
  type CanvasDocument,
  type CanvasResponse,
  type CanvasSummary,
} from "./types";

function csrfHeaders(): Record<string, string> {
  return { "x-csrf-token": getCsrfToken() };
}

export function listCanvases(): Promise<CanvasSummary[]> {
  return apiFetch("/api/canvases", { method: "GET" }, z.array(canvasSummarySchema));
}

export function createCanvas(title?: string): Promise<CanvasResponse> {
  return apiFetch(
    "/api/canvases",
    {
      method: "POST",
      headers: csrfHeaders(),
      body: JSON.stringify({ title }),
    },
    canvasResponseSchema,
  );
}

export function getCanvas(id: string): Promise<CanvasResponse> {
  return apiFetch(`/api/canvases/${id}`, { method: "GET" }, canvasResponseSchema);
}

export function saveCanvas(
  id: string,
  body: { title: string; document: CanvasDocument },
): Promise<CanvasResponse> {
  return apiFetch(
    `/api/canvases/${id}`,
    {
      method: "PUT",
      headers: csrfHeaders(),
      body: JSON.stringify(body),
    },
    canvasResponseSchema,
  );
}

export function deleteCanvas(id: string): Promise<{ deleted: boolean }> {
  return apiFetch(
    `/api/canvases/${id}`,
    { method: "DELETE", headers: csrfHeaders() },
    z.object({ deleted: z.boolean() }),
  );
}

const extractResultSchema = z.object({
  status: z.enum(["ok", "error"]),
  title: z.string(),
  markdown: z.string(),
  error: z.string().nullable(),
});

export function extractUrl(url: string) {
  return apiFetch(
    "/api/canvas/extract-url",
    {
      method: "POST",
      headers: csrfHeaders(),
      body: JSON.stringify({ url }),
    },
    extractResultSchema,
  );
}

const searchResultSchema = z.object({
  status: z.enum(["ok", "error"]),
  query: z.string(),
  markdown: z.string(),
  error: z.string().nullable(),
});

export function searchWeb(query: string) {
  return apiFetch(
    "/api/canvas/search",
    {
      method: "POST",
      headers: csrfHeaders(),
      body: JSON.stringify({ query }),
    },
    searchResultSchema,
  );
}
