import { afterEach, describe, expect, it, vi } from "vitest";
import { z } from "zod";

import { apiFetch } from "./http";

describe("apiFetch", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("preserves the default json content type when custom headers are provided", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      status: 200,
      text: async () => JSON.stringify({ ok: true }),
    });
    vi.stubGlobal("fetch", fetchMock);

    await apiFetch(
      "/api/projects",
      {
        method: "POST",
        headers: {
          "x-csrf-token": "csrf-token",
        },
        body: JSON.stringify({ name: "demo" }),
      },
      z.object({ ok: z.boolean() }),
    );

    const init = fetchMock.mock.calls[0][1] as RequestInit;
    const headers = new Headers(init.headers);
    expect(headers.get("content-type")).toBe("application/json");
    expect(headers.get("x-csrf-token")).toBe("csrf-token");
  });

  it("parses empty successful responses without calling json", async () => {
    const jsonMock = vi.fn();
    const fetchMock = vi.fn().mockResolvedValue({
      status: 204,
      headers: new Headers(),
      json: jsonMock,
      text: async () => "",
    });
    vi.stubGlobal("fetch", fetchMock);

    const payload = await apiFetch(
      "/api/projects/project-1/sources/demo.md",
      {
        method: "DELETE",
      },
      z.any(),
    );

    expect(payload).toBeUndefined();
    expect(jsonMock).not.toHaveBeenCalled();
  });
});
