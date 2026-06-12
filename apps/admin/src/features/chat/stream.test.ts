import { describe, expect, it } from "vitest";

import { parseSseBuffer } from "./stream";

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
