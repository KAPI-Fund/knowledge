import { describe, expect, it, vi } from "vitest";

import { runCanvasNode, streamCanvasChat } from "./stream";

function sseStream(chunks: string[]) {
  return new ReadableStream({
    start(controller) {
      const enc = new TextEncoder();
      for (const c of chunks) controller.enqueue(enc.encode(c));
      controller.close();
    },
  });
}

describe("runCanvasNode", () => {
  it("emits deltas then done", async () => {
    const body = sseStream([
      'event: delta\ndata: {"text":"Hel"}\n\n',
      'event: delta\ndata: {"text":"lo"}\n\n',
      'event: done\ndata: {"versionId":"v1","content":"Hello","createdAt":"t"}\n\n',
    ]);
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve(new Response(body, { status: 200 }))),
    );

    const deltas: string[] = [];
    let done: unknown = null;
    await runCanvasNode("c1", "n1", {
      onDelta: (t) => deltas.push(t),
      onDone: (p) => (done = p),
      onError: () => {},
    });
    expect(deltas.join("")).toBe("Hello");
    expect(done).toEqual({ versionId: "v1", content: "Hello", createdAt: "t" });
    vi.unstubAllGlobals();
  });
});

describe("streamCanvasChat", () => {
  const noopHandlers = { onDelta: () => {}, onDone: () => {}, onError: () => {} };

  it("sends the placement origin in the request body when provided", async () => {
    const fetchMock = vi.fn((_input: unknown, _init?: RequestInit) =>
      Promise.resolve(new Response(sseStream(["event: done\ndata: {}\n\n"]), { status: 200 })),
    );
    vi.stubGlobal("fetch", fetchMock);

    await streamCanvasChat("c1", "hi", ["a"], noopHandlers, { x: 120, y: 240 });

    const init = fetchMock.mock.calls[0][1] as RequestInit;
    expect(JSON.parse(init.body as string)).toEqual({
      message: "hi",
      selectedNodeIds: ["a"],
      x: 120,
      y: 240,
    });
    vi.unstubAllGlobals();
  });

  it("omits coordinates when no origin is given", async () => {
    const fetchMock = vi.fn((_input: unknown, _init?: RequestInit) =>
      Promise.resolve(new Response(sseStream(["event: done\ndata: {}\n\n"]), { status: 200 })),
    );
    vi.stubGlobal("fetch", fetchMock);

    await streamCanvasChat("c1", "hi", [], noopHandlers);

    const init = fetchMock.mock.calls[0][1] as RequestInit;
    expect(JSON.parse(init.body as string)).toEqual({ message: "hi", selectedNodeIds: [] });
    vi.unstubAllGlobals();
  });
});
