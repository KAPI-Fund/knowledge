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

import { listCanvases } from "./api";
import { useCanvas, useCanvasList } from "./queries";

function makeWrapper() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
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
