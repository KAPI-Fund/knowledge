import { afterEach, describe, expect, it, vi } from "vitest";

import { listCanvases, saveCanvas } from "./api";

function jsonResponse(body: unknown) {
  return Promise.resolve(
    new Response(JSON.stringify(body), {
      status: 200,
      headers: { "content-type": "application/json" },
    }),
  );
}

afterEach(() => vi.unstubAllGlobals());

describe("canvas api", () => {
  it("listCanvases returns summaries", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => jsonResponse([{ id: "c1", title: "A", updated_at: "t" }])),
    );
    const list = await listCanvases();
    expect(list[0].id).toBe("c1");
  });

  it("saveCanvas PUTs title + document and sends csrf header", async () => {
    const fetchMock = vi.fn(() =>
      jsonResponse({
        id: "c1",
        title: "B",
        document: { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } },
        created_at: "t1",
        updated_at: "t2",
      }),
    );
    vi.stubGlobal("fetch", fetchMock);
    await saveCanvas("c1", {
      title: "B",
      document: { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } },
    });
    const [url, init] = fetchMock.mock.calls[0];
    expect(String(url)).toContain("/api/canvases/c1");
    expect(init.method).toBe("PUT");
    expect(init.headers["x-csrf-token"]).toBeDefined();
  });
});
