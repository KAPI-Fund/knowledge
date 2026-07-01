import { describe, expect, it, vi } from "vitest";

import { runCanvasNode } from "./stream";

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
