import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { createCanvas, deleteCanvas, getCanvas, listCanvases, saveCanvas } from "./api";
import type { CanvasDocument, CanvasSummary } from "./types";

export const canvasKeys = {
  all: ["canvases"] as const,
  list: () => ["canvases", "list"] as const,
  detail: (id: string) => ["canvases", "detail", id] as const,
};

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
    onSuccess: (data) => {
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
    },
  });
}

export function useDeleteCanvas() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteCanvas(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: canvasKeys.all }),
  });
}
