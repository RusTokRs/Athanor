---
id: doc://docs/adapters/transport-mcp.md
kind: adapter
language: en
source_language: en
status: draft
---
# Model Context Protocol (MCP) Transport Adapter

The `athanor-transport-mcp` adapter exposes Athanor's query and execution functions as Model Context Protocol (MCP) tools over stdio (stdin/stdout).

Implements: MCP Stdio JSON-RPC 2.0 Server.

The current implementation is complete in source but remains pending Rust-enabled formatting, test, and Clippy execution for the current HEAD.

## CLI Subcommand

To start the MCP server:

```bash
ath mcp [path/to/project/root]
```

By default, if the path is omitted, it uses the current working directory.

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

Configure a stdio MCP server with:

- **Name**: Athanor
- **Command**: `path/to/ath mcp path/to/your/project/root`

### Antigravity

The workspace `antigravity.json` runs the local server through Cargo:

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

The workspace `.codex.json` contains the equivalent Cargo-backed MCP server entry.

## Session And Protocol Contract

The transport accepts only JSON-RPC `2.0`. Syntactically valid envelopes with a missing or different `jsonrpc` version fail as invalid requests.

The MCP session progresses through three phases:

```text
AwaitingInitialize
  -> AwaitingInitialized
  -> Ready
```

`initialize` must request MCP protocol `2024-11-05`. Tools are unavailable until the client sends `notifications/initialized`; the legacy `initialized` spelling remains accepted for compatibility.

A request with no `id` is a notification and receives no response. An explicitly present `null` id remains an explicit request id and receives a response with `id: null`.

JSON-RPC protocol failures use their standard numeric categories:

| Code | Meaning |
| --- | --- |
| `-32700` | Parse error |
| `-32600` | Invalid request or invalid session transition |
| `-32601` | Method not found |
| `-32602` | Invalid method parameters |
| `-32002` | MCP server not initialized |
| `-32603` | Athanor cancellation/deadline or unexpected internal application failure |

Normal MCP tool execution failures remain successful JSON-RPC responses with an MCP tool result containing `isError: true`. Cancellation and deadline termination remain JSON-RPC errors and include stable Athanor error details where available.

## Resource And Concurrency Contract

The stdio lifecycle is intentionally bounded:

- at most 32 ordinary requests are in flight;
- the serialized response queue holds at most 32 responses;
- completed request tasks are reaped during the input loop rather than retained until stdin closes;
- a full response queue applies backpressure to request completion;
- stdout has one async writer owner, preserving one JSON document per line;
- stdin EOF cancels registered reads, drains request tasks, closes the response queue, and then joins the writer.

Initialization and initialized notifications are processed in input order before ordinary concurrent tool requests. Read-tool cancellation continues to support `notifications/cancelled` and the legacy `$/cancelRequest` notification.

## Exposed MCP Tools

Every tool accepts an optional `deadline_unix_ms` field: a future Unix timestamp in milliseconds. Omitting it preserves an unbounded operation deadline, while transport concurrency and response buffering remain bounded.

| Tool Name | Description | Arguments |
| --- | --- | --- |
| `index` | Runs full index pipeline. | `validate_only?: boolean` |
| `explain` | Explains one canonical entity, facts, relations, and diagnostics. | `stable_key: string` |
| `search` | Performs Tantivy BM25 search over workspace knowledge. | `query: string`, `limit?: integer` |
| `context` | Generates a task-focused context pack from the latest snapshot. | `task: string`, `level?: string`, limits... |
| `impact` | Calculates the direct/transitive blast radius of changes. | `target?: string`, `diff?: boolean`, `max_depth?: integer` |
| `change_map` | Returns bounded evidence-backed change locations and relation chains. | `task?: string`, `target?: string`, `diff?: boolean`, limits... |
| `rustok_architecture_context` | Resolves compact RusTok ownership, contracts, interactions, tests, diagnostics, and evidence for an intent. | `intent: string`, `module?: string`, limits... |
| `check` | Returns scoped diagnostic reports. | `scope: "api" | "docs" | "env" | "scripts" | "deployment" | "runbooks"` |

## Logging

To avoid corrupting the stdio JSON-RPC stream, all startup messages, diagnostics, and request-task failures are written to `stderr`. Only protocol responses are written to `stdout`.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-transport-mcp --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo clippy -p athanor-transport-mcp --all-targets --locked -- -D warnings
```
