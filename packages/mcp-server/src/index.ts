#!/usr/bin/env node
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ErrorCode,
  ListToolsRequestSchema,
  McpError,
} from "@modelcontextprotocol/sdk/types.js";

import { KnowledgeApiClient } from "./api-client.js";
import {
  formatFileTree,
  formatGraph,
  formatReviews,
  formatSearchResults,
  truncateText,
} from "./formatters.js";

const VERSION = "0.1.0";
const MAX_TEXT_BYTES = 120_000;

const client = new KnowledgeApiClient();

const server = new Server(
  { name: "knowledge-server", version: VERSION },
  { capabilities: { tools: {} } },
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: "knowledge_status",
      description:
        "Check whether knowledge-server is reachable and whether the configured Bearer token can list projects.",
      inputSchema: { type: "object", properties: {}, additionalProperties: false },
    },
    {
      name: "knowledge_projects",
      description: "List projects visible to the configured Bearer token.",
      inputSchema: { type: "object", properties: {}, additionalProperties: false },
    },
    {
      name: "knowledge_files",
      description:
        "List files in a project. project_id is required (the multi-tenant server has no notion of a current project).",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
          root: {
            type: "string",
            enum: ["wiki", "sources", "all"],
            description: "Tree root to list. Defaults to wiki.",
          },
          recursive: { type: "boolean", description: "Whether to list recursively. Defaults to true." },
          max_files: { type: "number", description: "Maximum files returned." },
        },
        required: ["project_id"],
        additionalProperties: false,
      },
    },
    {
      name: "knowledge_read_file",
      description:
        "Read a text file from a project. Only public project paths such as wiki/ and raw/sources/ are allowed.",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
          path: { type: "string", description: "Project-relative file path, for example wiki/index.md." },
        },
        required: ["project_id", "path"],
        additionalProperties: false,
      },
    },
    {
      name: "knowledge_reviews",
      description:
        "List Review tab items from a project. Defaults to unresolved items so agents can help manage pending review work.",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
          status: {
            type: "string",
            enum: ["unresolved", "resolved", "all"],
            description: "Review status filter. Defaults to unresolved.",
          },
          type: { type: "string", description: "Optional review item type filter." },
          limit: { type: "number", description: "Maximum review items returned." },
        },
        required: ["project_id"],
        additionalProperties: false,
      },
    },
    {
      name: "knowledge_search",
      description: "Search a project using the same hybrid keyword + vector retrieval used by the UI.",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
          query: { type: "string", description: "Search query." },
          top_k: { type: "number", description: "Maximum results." },
          include_content: { type: "boolean", description: "Include full page content in results." },
        },
        required: ["project_id", "query"],
        additionalProperties: false,
      },
    },
    {
      name: "knowledge_graph",
      description: "Query the project knowledge graph.",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
          q: { type: "string", description: "Optional text filter." },
          node_type: { type: "string", description: "Optional node type filter." },
          limit: { type: "number", description: "Maximum nodes." },
        },
        required: ["project_id"],
        additionalProperties: false,
      },
    },
    {
      name: "knowledge_rescan_sources",
      description:
        "Trigger a project source folder rescan, using the project's Source Watch rules.",
      inputSchema: {
        type: "object",
        properties: {
          project_id: { type: "string", description: "Project UUID." },
        },
        required: ["project_id"],
        additionalProperties: false,
      },
    },
  ],
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const args = asObject(request.params.arguments ?? {});
  try {
    switch (request.params.name) {
      case "knowledge_status": {
        const health = await client.health();
        let projects: Awaited<ReturnType<typeof client.projects>> | { error: string };
        try {
          projects = await client.projects();
        } catch (err) {
          projects = { error: err instanceof Error ? err.message : String(err) };
        }
        return textResult(JSON.stringify({ ...health, ...projects }, null, 2));
      }
      case "knowledge_projects": {
        return textResult(JSON.stringify(await client.projects(), null, 2));
      }
      case "knowledge_files": {
        const response = await client.files(requireProjectId(args), {
          root: enumArg(args.root, ["wiki", "sources", "all"] as const, "wiki"),
          recursive: boolArg(args.recursive, true),
          maxFiles: numberArg(args.max_files),
        });
        return textResult(formatFileTree(response.files, response.truncated));
      }
      case "knowledge_read_file": {
        const projectId = requireProjectId(args);
        const relPath = stringArg(args.path, "path");
        const { path, content } = await client.fileContent(projectId, relPath);
        return textResult(`# ${path}\n\n${truncateText(content, MAX_TEXT_BYTES)}`);
      }
      case "knowledge_reviews": {
        const reviews = await client.reviews(requireProjectId(args), {
          status: enumArg(args.status, ["unresolved", "resolved", "all"] as const, "unresolved"),
          type: optionalStringArg(args.type),
          limit: numberArg(args.limit),
        });
        return textResult(formatReviews(reviews));
      }
      case "knowledge_search": {
        const projectId = requireProjectId(args);
        const query = stringArg(args.query, "query");
        const search = await client.search(projectId, query, {
          topK: numberArg(args.top_k),
          includeContent: boolArg(args.include_content, false),
        });
        return textResult(formatSearchResults(query, search));
      }
      case "knowledge_graph": {
        const graph = await client.graph(requireProjectId(args), {
          q: optionalStringArg(args.q),
          nodeType: optionalStringArg(args.node_type),
          limit: numberArg(args.limit),
        });
        return textResult(formatGraph(graph.nodes, graph.edges));
      }
      case "knowledge_rescan_sources": {
        return textResult(JSON.stringify(await client.rescan(requireProjectId(args)), null, 2));
      }
      default:
        throw new McpError(ErrorCode.MethodNotFound, `Unknown tool: ${request.params.name}`);
    }
  } catch (err) {
    if (err instanceof McpError) throw err;
    throw new McpError(
      ErrorCode.InternalError,
      err instanceof Error ? err.message : String(err),
    );
  }
});

function textResult(text: string) {
  return { content: [{ type: "text" as const, text }] };
}

function asObject(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  return value as Record<string, unknown>;
}

function requireProjectId(args: Record<string, unknown>): string {
  const value = args.project_id;
  if (typeof value !== "string" || value.trim() === "") {
    throw new McpError(ErrorCode.InvalidParams, "project_id is required");
  }
  return value;
}

function stringArg(value: unknown, name: string): string {
  if (typeof value !== "string" || value.trim() === "") {
    throw new McpError(ErrorCode.InvalidParams, `${name} is required`);
  }
  return value;
}

function optionalStringArg(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() !== "" ? value : undefined;
}

function boolArg(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function numberArg(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function enumArg<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback;
}

async function main(): Promise<void> {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  const base = process.env.KNOWLEDGE_API_BASE_URL ?? "http://127.0.0.1:4001";
  console.error(`knowledge-mcp v${VERSION} connected to ${base}`);
}

main().catch((err) => {
  console.error("Failed to start knowledge-mcp:", err);
  process.exit(1);
});
