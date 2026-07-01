import { describe, expect, it } from "vitest";

import { canvasDocumentSchema, canvasResponseSchema } from "./types";

describe("canvas schemas", () => {
  it("parses a minimal document", () => {
    const doc = canvasDocumentSchema.parse({
      nodes: [],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    });
    expect(doc.nodes).toEqual([]);
    expect(doc.viewport.zoom).toBe(1);
  });

  it("parses a canvas response with a note node", () => {
    const res = canvasResponseSchema.parse({
      id: "c1",
      title: "Board",
      document: {
        nodes: [{ id: "n1", type: "note", x: 0, y: 0, w: 280, h: 160, data: { markdown: "hi" } }],
        edges: [],
        viewport: { x: 0, y: 0, zoom: 1 },
      },
      created_at: "t1",
      updated_at: "t2",
    });
    expect(res.document.nodes[0].type).toBe("note");
    expect(res.document.nodes[0].data.markdown).toBe("hi");
  });
});
