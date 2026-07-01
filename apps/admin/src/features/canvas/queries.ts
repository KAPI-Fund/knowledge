import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { createCanvas, deleteCanvas, getCanvas, listCanvases, saveCanvas } from "./api";
import type { CanvasDocument } from "./types";

export const canvasKeys = {
  all: ["canvases"] as const,
  detail: (id: string) => ["canvases", id] as const,
};

export function useCanvasList() {
  return useQuery({ queryKey: canvasKeys.all, queryFn: listCanvases });
}

export function useCanvas(id: string | undefined) {
  return useQuery({
    queryKey: id ? canvasKeys.detail(id) : canvasKeys.all,
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
    onSuccess: (data) => qc.setQueryData(canvasKeys.detail(id), data),
  });
}

export function useDeleteCanvas() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteCanvas(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: canvasKeys.all }),
  });
}
