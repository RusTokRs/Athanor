---
id: doc://docs/development/direct-operation-context.md
kind: developer_guide
language: en
source_language: en
status: verified
---
# Direct CLI And MCP Operation Context

## Scope

This slice extends the shared cancellation and deadline contract from daemon reads to direct user-facing
read execution.

Covered direct CLI commands:

- `ath context`;
- `ath explain`;
- `ath overview`;
- `ath impact`;
- `ath change-map`;
- `ath search`.

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

The focused entry parser preserves the established command arguments and text/JSON rendering. It adds:

```text
--deadline-unix-ms <UNIX_MILLISECONDS>
```

A default may also be supplied through:

```text
ATHANOR_DEADLINE_UNIX_MS=<UNIX_MILLISECONDS>
```

The command-line option takes the request deadline role; malformed environment values fail closed before
project access. Each intercepted command owns a process-local operation id of the form
`cli:<command>:<pid>` and one live `CancellationHandle`.

The CLI read future is polled together with:

- the operation deadline;
- the shared cancellation flag;
- `tokio::signal::ctrl_c()`.

Ctrl-C cancels the lease. A preflight check prevents work after an already-expired deadline, and a postflight
check prevents a completed legacy future from returning a late successful result after cancellation.

Commands outside the focused read set continue through the legacy CLI dispatcher unchanged.

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

- Existing direct CLI flags and rendering are preserved for the covered commands.
- Existing MCP tool schemas are reused from the legacy dispatcher.
- Existing MCP tools remain available under the same names.
- JSON-RPC responses may complete out of input order, which is valid because request ids provide correlation.
- Notifications without an id remain response-free.
- Request ids used for cancellable reads must be strings or numbers.

## Remaining Work

The following remain outside this code slice:

- direct CLI operation lifecycle for `check`, graph queries, and manual Rustok read commands;
- cancellation propagation inside synchronous filesystem discovery, graph construction, and search-index
  rebuild work;
- transactional MCP notification cancellation;
- hosted Linux/Windows formatting, test, Clippy, stdio, Ctrl-C, and disconnect evidence.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p ath --test direct_read_cli --locked
cargo test -p ath direct_read_cli --locked
cargo test -p athanor-transport-mcp lifecycle --locked
cargo test -p athanor-transport-mcp --locked
cargo clippy -p ath --all-targets --locked -- -D warnings
cargo clippy -p athanor-transport-mcp --all-targets --locked -- -D warnings
```

Required regressions:

- each focused direct read help page exposes `--deadline-unix-ms`;
- malformed CLI and environment deadlines fail before project access;
- an expired deadline does not poll the application future;
- cancellation rejects a would-be late successful result;
- a pending direct read is bounded by its operation deadline;
- MCP cancellation notification terminates a registered read and releases its lease;
- MCP deadline termination releases the request registry entry;
- transactional `index` remains outside the cancellable read scope;
- non-read direct CLI commands continue through the legacy dispatcher.

Hosted verification remains separate and is not claimed without recorded workflow or local toolchain evidence.
