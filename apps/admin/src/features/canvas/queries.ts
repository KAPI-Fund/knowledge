import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useCallback } from "react";

import { createCanvas, deleteCanvas, getCanvas, listCanvases, saveCanvas } from "./api";
import type { CanvasDocument, CanvasResponse, CanvasSummary } from "./types";

export const canvasKeys = {
  all: ["canvases"] as const,
  list: () => ["canvases", "list"] as const,
  detail: (id: string) => ["canvases", "detail", id] as const,
};

// Reflect a saved canvas into the detail and list caches so readers stay in
// sync with the server. Shared by the debounced mutation and the call-time
// save below.
function applyCanvasSave(qc: QueryClient, id: string, data: CanvasResponse) {
  qc.setQueryData(canvasKeys.detail(id), data);
  qc.setQueryData<CanvasSummary[]>(canvasKeys.list(), (prev) => {
    if (!prev) {
      return prev;
    }
    const patched = prev.map((summary) =>
      summary.id === id
        ? { ...summary, title: data.title, updated_at: data.updated_at }
        : summary,
    );
    return [...patched].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
  });
}

export function useCanvasList() {
  return useQuery({ queryKey: canvasKeys.list(), queryFn: listCanvases });
}

export function useCanvas(id: string | undefined) {
  return useQuery({
    queryKey: canvasKeys.detail(id ?? ""),
    queryFn: () => getCanvas(id as string),
    enabled: Boolean(id),
  });
}

export function useCreateCanvas() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (title?: string) => createCanvas(title),
    onSuccess: () => qc.invalidateQueries({ queryKey: canvasKeys.all }),
  });
}

export function useSaveCanvas(id: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: { title: string; document: CanvasDocument }) => saveCanvas(id, body),
    onSuccess: (data) => applyCanvasSave(qc, id, data),
  });
}

// Save to a canvas id chosen at call time and keep the caches in sync the same
// way useSaveCanvas does. The debounced autosave uses the mutation above, but
// the flush-driven saves (before an SSE run/chat, and when leaving a canvas)
// target an id decided at call time. Routing them through here keeps the detail
// cache current -- otherwise, because the canvas page ignores refetches once an
// id is loaded, re-entering that canvas would render the pre-save document and
// the next edit would overwrite the newer server copy.
export function useCanvasCacheSave() {
  const qc = useQueryClient();
  return useCallback(
    async (id: string, body: { title: string; document: CanvasDocument }) => {
      const data = await saveCanvas(id, body);
      applyCanvasSave(qc, id, data);
      return data;
    },
    [qc],
  );
}

export function useDeleteCanvas() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteCanvas(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: canvasKeys.all }),
  });
}
