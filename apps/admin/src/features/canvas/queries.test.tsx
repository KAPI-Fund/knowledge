import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

vi.mock("./api", () => ({
  listCanvases: vi.fn(),
  getCanvas: vi.fn(),
  saveCanvas: vi.fn(),
  createCanvas: vi.fn(),
  deleteCanvas: vi.fn(),
}));

import { listCanvases, saveCanvas } from "./api";
import { canvasKeys, useCanvas, useCanvasList, useSaveCanvas } from "./queries";
import type { CanvasSummary } from "./types";

function makeClient() {
  return new QueryClient({ defaultOptions: { queries: { retry: false } } });
}

function makeWrapper(client = makeClient()) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

describe("canvas query keys", () => {
  it("does not serve the list cache to an empty-id detail query", async () => {
    vi.mocked(listCanvases).mockResolvedValue([
      { id: "c1", title: "A", updated_at: "2026-01-01T00:00:00Z" },
    ]);
    const { result } = renderHook(
      () => ({ list: useCanvasList(), detail: useCanvas(undefined) }),
      { wrapper: makeWrapper() },
    );
    await waitFor(() => expect(result.current.list.data).toHaveLength(1));
    expect(result.current.detail.data).toBeUndefined();
  });
});

describe("useSaveCanvas list-cache refresh", () => {
  it("patches the summary and re-sorts by updated_at after a save", async () => {
    const client = makeClient();
    client.setQueryData<CanvasSummary[]>(canvasKeys.list(), [
      { id: "c2", title: "Two", updated_at: "2026-06-01T12:00:00Z" },
      { id: "c1", title: "Old", updated_at: "2026-05-01T00:00:00Z" },
    ]);
    vi.mocked(saveCanvas).mockResolvedValue({
      id: "c1",
      title: "New title",
      document: { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } },
      created_at: "2026-05-01T00:00:00Z",
      updated_at: "2026-06-02T00:00:00Z",
    });

    const { result } = renderHook(() => useSaveCanvas("c1"), {
      wrapper: makeWrapper(client),
    });
    await result.current.mutateAsync({
      title: "New title",
      document: { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } },
    });

    const list = client.getQueryData<CanvasSummary[]>(canvasKeys.list());
    expect(list?.map((c) => c.id)).toEqual(["c1", "c2"]);
    expect(list?.[0]).toMatchObject({ title: "New title", updated_at: "2026-06-02T00:00:00Z" });
  });
});
