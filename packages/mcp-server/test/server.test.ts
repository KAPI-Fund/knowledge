import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { fileURLToPath } from "node:url";

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

const here = fileURLToPath(new URL("./", import.meta.url));
const entry = `${here}../dist/src/index.js`;

let httpServer: Server;
let port: number;
const seenAuthHeaders: string[] = [];

beforeAll(async () => {
  httpServer = createServer((req: IncomingMessage, res: ServerResponse) => {
    seenAuthHeaders.push(String(req.headers["authorization"] ?? ""));
    if (req.method === "GET" && req.url === "/api/health") {
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: true, service: "knowledge-server" }));
      return;
    }
    if (req.method === "GET" && req.url === "/api/projects") {
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ projects: [{ id: "p1", name: "Demo", rootPath: "/tmp/demo" }] }));
      return;
    }
    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: "not found" }));
  });
  await new Promise<void>((resolve) => {
    httpServer.listen(0, "127.0.0.1", () => resolve());
  });
  const addr = httpServer.address();
  if (!addr || typeof addr === "string") throw new Error("no port");
  port = addr.port;
});

afterAll(async () => {
  await new Promise<void>((resolve) => httpServer.close(() => resolve()));
});

describe("mcp-server stdio", () => {
  it("lists 8 tools and calls knowledge_projects with Bearer auth", async () => {
    const transport = new StdioClientTransport({
      command: process.execPath,
      args: [entry],
      env: {
        ...process.env,
        KNOWLEDGE_API_BASE_URL: `http://127.0.0.1:${port}`,
        KNOWLEDGE_API_TOKEN: "test-token",
      },
    });
    const client = new Client({ name: "test-client", version: "0.0.0" }, { capabilities: {} });
    await client.connect(transport);

    try {
      const list = await client.listTools();
      const names = list.tools.map((tool) => tool.name).sort();
      expect(names).toEqual([
        "knowledge_files",
        "knowledge_graph",
        "knowledge_projects",
        "knowledge_read_file",
        "knowledge_rescan_sources",
        "knowledge_reviews",
        "knowledge_search",
        "knowledge_status",
      ]);

      const result = await client.callTool({ name: "knowledge_projects", arguments: {} });
      const text =
        Array.isArray(result.content) && result.content[0]?.type === "text"
          ? String(result.content[0].text)
          : "";
      expect(text).toContain("Demo");
      expect(seenAuthHeaders).toContain("Bearer test-token");
    } finally {
      await client.close();
    }
  }, 15_000);

  it("rejects knowledge_files without project_id", async () => {
    const transport = new StdioClientTransport({
      command: process.execPath,
      args: [entry],
      env: {
        ...process.env,
        KNOWLEDGE_API_BASE_URL: `http://127.0.0.1:${port}`,
        KNOWLEDGE_API_TOKEN: "test-token",
      },
    });
    const client = new Client({ name: "test-client", version: "0.0.0" }, { capabilities: {} });
    await client.connect(transport);
    try {
      await expect(
        client.callTool({ name: "knowledge_files", arguments: {} }),
      ).rejects.toThrow(/project_id is required/);
    } finally {
      await client.close();
    }
  }, 15_000);
});
