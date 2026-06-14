import { describe, expect, it } from "vitest";

import { KnowledgeApiClient, normalizeBaseUrl } from "../src/api-client.js";

describe("normalizeBaseUrl", () => {
  it("trims trailing slashes", () => {
    expect(normalizeBaseUrl("http://127.0.0.1:4001///")).toBe("http://127.0.0.1:4001");
  });

  it("falls back to the default when empty", () => {
    expect(normalizeBaseUrl("")).toBe("http://127.0.0.1:4001");
  });
});

describe("KnowledgeApiClient.projects", () => {
  it("sends Bearer token and parses the list", async () => {
    const calls: Array<{ url: string; init?: RequestInit }> = [];
    const fetchImpl = (async (url: string | URL | Request, init?: RequestInit) => {
      calls.push({ url: String(url), init });
      return new Response(
        JSON.stringify({ projects: [{ id: "p1", name: "Demo", rootPath: "/tmp/demo" }] }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    }) as typeof fetch;

    const client = new KnowledgeApiClient({
      baseUrl: "http://localhost:4001/",
      token: "secret",
      fetchImpl,
    });
    const result = await client.projects();

    expect(calls[0]?.url).toBe("http://localhost:4001/api/projects");
    expect((calls[0]?.init?.headers as Record<string, string>).Authorization).toBe("Bearer secret");
    expect(result.projects[0]?.id).toBe("p1");
  });
});

describe("KnowledgeApiClient.health", () => {
  it("does not send Authorization", async () => {
    const seen: Array<RequestInit | undefined> = [];
    const fetchImpl = (async (_url: unknown, init?: RequestInit) => {
      seen.push(init);
      return new Response(JSON.stringify({ ok: true, service: "knowledge-server" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }) as typeof fetch;

    const client = new KnowledgeApiClient({ token: "secret", fetchImpl });
    await client.health();

    expect((seen[0]?.headers as Record<string, string> | undefined)?.Authorization).toBeUndefined();
  });
});

describe("KnowledgeApiClient.search", () => {
  it("posts JSON body", async () => {
    let body = "";
    const fetchImpl = (async (_url: unknown, init?: RequestInit) => {
      body = String(init?.body ?? "");
      return new Response(
        JSON.stringify({
          mode: "hybrid",
          tokenHits: 2,
          vectorHits: 1,
          results: [{ path: "wiki/a.md", title: "A", snippet: "hit", score: 0.5, vectorScore: 0.9 }],
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    }) as typeof fetch;

    const client = new KnowledgeApiClient({ token: "secret", fetchImpl });
    const result = await client.search("p1", "query", { topK: 5 });

    expect(JSON.parse(body)).toEqual({ query: "query", topK: 5 });
    expect(result.results[0]?.path).toBe("wiki/a.md");
  });
});

describe("KnowledgeApiClient.rescan", () => {
  it("posts to the :rescan suffix endpoint", async () => {
    const calls: string[] = [];
    const fetchImpl = (async (url: unknown) => {
      calls.push(String(url));
      return new Response(JSON.stringify({ taskId: "task-1", status: "queued" }), {
        status: 202,
        headers: { "content-type": "application/json" },
      });
    }) as typeof fetch;

    const client = new KnowledgeApiClient({ token: "secret", fetchImpl });
    await client.rescan("p1");

    expect(calls[0]).toBe("http://127.0.0.1:4001/api/projects/p1/sources:rescan");
  });
});

describe("KnowledgeApiClient error handling", () => {
  it("wraps HTTP errors with the response message", async () => {
    const fetchImpl = (async () =>
      new Response(JSON.stringify({ error: "not a project member" }), {
        status: 403,
        headers: { "content-type": "application/json" },
      })) as typeof fetch;

    const client = new KnowledgeApiClient({ token: "secret", fetchImpl });
    await expect(client.projects()).rejects.toThrow(/403.*not a project member/);
  });
});
