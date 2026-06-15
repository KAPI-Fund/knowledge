# @knowledge/mcp-server

Standalone Model Context Protocol server that exposes a knowledge-server project
to MCP-compatible agent clients (Claude Code, Codex CLI, Cursor, etc.) using a
per-user API token minted from the admin UI.

## Tools

| Tool | Description |
|---|---|
| `knowledge_status` | Verify server reachability and that the token can list projects. |
| `knowledge_projects` | List projects the token can see. |
| `knowledge_files` | List files in a project (wiki / sources / all). |
| `knowledge_read_file` | Read a single text file from a project. |
| `knowledge_reviews` | List Review tab items. |
| `knowledge_search` | Hybrid keyword + vector search. |
| `knowledge_graph` | Knowledge-graph summary. |
| `knowledge_rescan_sources` | Trigger a Source Watch rescan. |

## Installation

From the repo root:

```bash
npm install
npm run build --workspace @knowledge/mcp-server
```

The built binary lands at `packages/mcp-server/dist/src/index.js` and is exposed
as `knowledge-mcp` once you `npm link` (optional).

## Configuration

The server reads two environment variables:

- `KNOWLEDGE_API_BASE_URL` — defaults to `http://127.0.0.1:4001`.
- `KNOWLEDGE_API_TOKEN` — required for any tool other than `knowledge_status`.
  Mint a token in the admin UI: **Admin Console → API Tokens → Mint Token**.

## Claude Code

Add to your Claude Code MCP config (`.mcp.json` or via `/mcp` settings):

```json
{
  "mcpServers": {
    "knowledge": {
      "command": "node",
      "args": ["/absolute/path/to/packages/mcp-server/dist/src/index.js"],
      "env": {
        "KNOWLEDGE_API_BASE_URL": "http://127.0.0.1:4001",
        "KNOWLEDGE_API_TOKEN": "<your-token>"
      }
    }
  }
}
```

## Codex CLI

```toml
[mcp_servers.knowledge]
command = "node"
args = ["/absolute/path/to/packages/mcp-server/dist/src/index.js"]
env = { KNOWLEDGE_API_BASE_URL = "http://127.0.0.1:4001", KNOWLEDGE_API_TOKEN = "<your-token>" }
```
