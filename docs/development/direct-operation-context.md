---
id: doc://docs/development/direct-operation-context.md
kind: developer_guide
language: en
source_language: en
status: verified
---
# Direct CLI And MCP Operation Context

## Scope

This work extends the shared cancellation and deadline contract from daemon reads to direct user-facing read
execution.

Covered direct CLI commands:

- `ath context`;
- `ath explain`;
- `ath overview`;
- `ath impact`;
- `ath change-map`;
- `ath search`;
- `ath check`;
- `ath graph export`;
- `ath graph related`;
- `ath graph path`;
- `ath graph hubs`;
- `ath graph pagerank`;
- `ath graph cycles`.

Covered MCP lifecycle:

- concurrent JSON-RPC request processing over stdio;
- request-scoped cancellation authority for read tools;
- `notifications/cancelled` and legacy `$/cancelRequest` notifications;
- cancellation of all outstanding reads when stdin reaches EOF;
- request deadlines through the existing `deadline_unix_ms` tool argument;
- stable `cancelled` and `deadline_exceeded` JSON-RPC error data.

Transactional MCP tools such as `index` continue to use the established legacy dispatcher and deadline
semantics. They are intentionally excluded from notification-driven cancellation until their rollback and
durable-success semantics are reviewed independently.

## Direct CLI Contract

Focused entry parsers preserve the established command arguments and text/JSON rendering. They add:

```text
--deadline-unix-ms <UNIX_MILLISECONDS>
```

A default may also be supplied through:

```text
ATHANOR_DEADLINE_UNIX_MS=<UNIX_MILLISECONDS>
```

The explicit command-line deadline has precedence over the environment. Malformed values fail closed before
project access. Each intercepted command owns a process-local operation id of the form
`cli:<command>:<pid>` and one live `CancellationHandle`.

Legacy async reads are polled together with the operation deadline, the shared cancellation flag, and
`tokio::signal::ctrl_c()`. Preflight checks prevent already-expired work; postflight checks reject late success
after cancellation.

## Blocking Worker Boundary

Standard graph builders run on Tokio's blocking pool after the committed snapshot has been loaded through a
context-aware store read. The operation wrapper polls cancellation and deadline state while the builder runs.

A blocking task cannot be force-stopped safely. Therefore cancellation records a terminal operation error but
the wrapper drains the worker before returning. The completed graph value is discarded and cannot become a
late successful response. This provides:

- no detached graph worker after command termination;
- no CPU-bound graph construction on an async runtime worker;
- stable `cancelled`/`deadline_exceeded` output;
- bounded resource ownership even when cancellation occurs during PageRank or traversal.

Cancellation latency is currently limited by one graph build. Finer-grained cooperative checks inside graph
algorithms remain future work.

`ath check --strict` uses the same non-orphaning blocking boundary for synchronous API contract diffing. The
normal diagnostic query remains an async operation-aware read. Strict non-API scope behavior and exit codes
remain compatible with the legacy CLI.

## MCP Contract

The MCP crate keeps its public `run_mcp_server(PathBuf)` API. Its new crate root includes the established tool
schemas and tool dispatcher as a compatibility module, while a focused lifecycle layer owns stdio routing.

The server continues reading input while tool requests are active. Read requests are registered by their
JSON-RPC string or numeric id. The operation id is `mcp:<serialized-request-id>`, so duplicate active request
identities fail closed through the core cancellation lease.

For read tools, the lifecycle loop polls the tool future every 25 ms together with cancellation and deadline
checks. Cancellation drops the read future and returns a structured operation error instead of a late tool
success. Normal tool failures retain the existing MCP `isError` content response. Transactional tools retain
the compatibility path.

When stdin closes, every registered read lease is cancelled and request tasks are drained before the response
writer exits.

## Compatibility

- Existing direct CLI flags and rendering are preserved for covered commands.
- Unsupported graph subcommands and manual Rustok commands continue through the legacy dispatcher.
- Existing MCP tool schemas and names are reused.
- JSON-RPC responses may complete out of input order because request ids provide correlation.
- Notifications without an id remain response-free.
- Request ids used for cancellable reads must be strings or numbers.

## Remaining Work

The following remain outside this code slice:

- direct CLI operation lifecycle for manual Rustok read commands and architecture graph commands;
- finer-grained cooperative cancellation inside graph algorithms, filesystem discovery, and search-index
  rebuild work;
- transactional MCP notification cancellation;
- hosted Linux/Windows formatting, test, Clippy, stdio, Ctrl-C, disconnect, and worker-drain evidence.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p ath --test direct_read_cli --locked
cargo test -p ath --test direct_graph_cli --locked
cargo test -p ath --test direct_check_cli --locked
cargo test -p ath direct_operation --locked
cargo test -p ath direct_graph_cli --locked
cargo test -p ath direct_check_cli --locked
cargo test -p athanor-app graph_operation --locked
cargo test -p athanor-transport-mcp lifecycle --locked
cargo test -p athanor-transport-mcp --locked
cargo clippy -p ath --all-targets --locked -- -D warnings
cargo clippy -p athanor-app --all-targets --locked -- -D warnings
cargo clippy -p athanor-transport-mcp --all-targets --locked -- -D warnings
```

Required regressions:

- every focused direct read, graph, and check help page exposes `--deadline-unix-ms`;
- malformed CLI and environment deadlines fail before project access;
- an expired deadline does not spawn a blocking worker;
- cancellation drains the graph/API-diff worker and rejects its result;
- cancellation rejects a would-be late successful legacy future;
- MCP cancellation notification terminates a registered read and releases its lease;
- MCP deadline termination releases the request registry entry;
- transactional `index` remains outside the cancellable read scope;
- unsupported graph and non-read commands continue through the legacy dispatcher.

Hosted verification remains separate and is not claimed without recorded workflow or local toolchain evidence.
