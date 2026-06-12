import { afterEach, describe, expect, it, vi } from "vitest";

import { parseSseBuffer, streamChatMessage } from "./stream";

describe("parseSseBuffer", () => {
  it("parses complete events and keeps the partial tail", () => {
    const buffer =
      'event: delta\ndata: {"text":"Hel"}\n\nevent: delta\ndata: {"text":"lo"}\n\nevent: done\ndata: {"mes';
    const { events, rest } = parseSseBuffer(buffer);
    expect(events).toEqual([
      { event: "delta", data: '{"text":"Hel"}' },
      { event: "delta", data: '{"text":"lo"}' },
    ]);
    expect(rest).toBe('event: done\ndata: {"mes');
  });

  it("ignores comment-only blocks", () => {
    const { events, rest } = parseSseBuffer(": keep-alive\n\n");
    expect(events).toEqual([]);
    expect(rest).toBe("");
  });
});

describe("streamChatMessage", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("surfaces the server error body on non-2xx responses", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ error: "provider configuration is incomplete" }), {
          status: 400,
          headers: { "content-type": "application/json" },
        }),
      ),
    );
    const onError = vi.fn();

    await streamChatMessage(
      { projectId: "p1", conversationId: "c1", content: "hi" },
      { onDelta: vi.fn(), onDone: vi.fn(), onError },
    );

    expect(onError).toHaveBeenCalledWith("provider configuration is incomplete");
  });

  it("falls back to the status code when the error body is not JSON", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(new Response("boom", { status: 502 })),
    );
    const onError = vi.fn();

    await streamChatMessage(
      { projectId: "p1", conversationId: "c1", content: "hi" },
      { onDelta: vi.fn(), onDone: vi.fn(), onError },
    );

    expect(onError).toHaveBeenCalledWith("chat request failed with status 502");
  });
});
