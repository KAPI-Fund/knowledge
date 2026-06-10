import { afterEach, describe, expect, it, vi } from "vitest";
import { z } from "zod";

import { ApiClientError, apiFetch } from "./http";

describe("apiFetch", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("preserves the default json content type when custom headers are provided", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      statusText: "OK",
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
      ok: true,
      status: 204,
      statusText: "No Content",
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

  it("throws ApiClientError for non-2xx JSON responses", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 400,
        statusText: "Bad Request",
        text: async () => JSON.stringify({ error: "unknown project" }),
      }),
    );

    await expect(
      apiFetch("/api/projects/missing", { method: "GET" }, z.object({ ok: z.boolean() })),
    ).rejects.toMatchObject({
      name: "ApiClientError",
      status: 400,
      message: "unknown project",
    } satisfies Partial<ApiClientError>);
  });
});
