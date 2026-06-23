---
id: doc://docs/adapters/transport-mcp.md
kind: adapter
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000039
status: verified
---

# Model Context Protocol (MCP) Transport Adapter

The `athanor-transport-mcp` adapter exposes Athanor's query and execution functions as Model Context Protocol (MCP) tools over stdio (stdin/stdout).

Implements: MCP Stdio JSON-RPC 2.0 Server.

## CLI Subcommand
To start the MCP server:
```bash
ath mcp [path/to/project/root]
```
By default, if the path is omitted, it will use the current working directory.

## Integrating with LLM Clients

### Claude Desktop
Add the following config to your `claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "athanor": {
      "command": "path/to/ath",
      "args": ["mcp", "path/to/your/project/root"]
    }
  }
}
```

### Cursor or Windsurf
You can configure a new MCP stdio server in Cursor's settings:
- **Name**: Athanor
- **Type**: stdio
- **Command**: `path/to/ath mcp path/to/your/project/root`

### Antigravity
A pre-configured `antigravity.json` file is provided in the workspace root. It runs the local Athanor server on the current workspace using cargo:
```json
{
  "mcpServers": {
    "athanor": {
      "command": "cargo",
      "args": ["run", "--bin", "ath", "--quiet", "--", "mcp", "."]
    }
  }
}
```

### Codex
A pre-configured `.codex.json` file is also provided in the workspace root:
```json
{
  "mcp": {
    "servers": {
      "athanor": {
        "command": "cargo",
        "args": ["run", "--bin", "ath", "--quiet", "--", "mcp", "."]
      }
    }
  }
}
```

## Exposed MCP Tools

| Tool Name | Description | Arguments |
| --- | --- | --- |
| `index` | Runs full index pipeline. | `validate_only?: boolean` |
| `explain` | Explain a canonical entity detail, facts, relations, and diagnostics. | `stable_key: string` |
| `search` | Performs Tantivy BM25 search over workspace knowledge. | `query: string`, `limit?: integer` |
| `context` | Generates a task-focused context pack from the latest snapshot. | `task: string`, `level?: string`, limits... |
| `impact` | Calculates the direct/transitive blast radius of changes. | `target?: string`, `diff?: boolean`, `max_depth?: integer` |
| `check` | Returns scoped diagnostic reports. | `scope: "api" | "docs" | "env" | "scripts" | "deployment" | "runbooks"` |

## Logging
To avoid corrupting the stdio protocol JSON-RPC stream, all debugging output, logging, and error tracking are redirected to `stderr`.
