---
id: doc://docs/development/direct-operation-context.md
kind: developer_guide
language: en
source_language: en
status: draft
---
# Direct CLI And MCP Operation Context

## Scope

This document describes the shared cancellation and deadline contract used by direct CLI reads and the MCP stdio transport. The source implementation is present on the current branch; Rust-enabled formatting, tests, Clippy, and executable interoperability remain pending for the current HEAD.

Covered direct CLI command families include Context, Explain, Overview, Impact, Change Map, Search, Check, standard Graph reads, and RusTok audit/graph reads.

The MCP lifecycle covers:

- concurrent JSON-RPC request processing over stdio;
- explicit `RuntimeComposition` passed from the CLI composition root;
- request-scoped cancellation authority for read tools;
- `notifications/cancelled` and legacy `$/cancelRequest` notifications;
- cancellation of outstanding registered reads when stdin reaches EOF;
- request deadlines through `deadline_unix_ms`;
- stable `cancelled` and `deadline_exceeded` JSON-RPC error data.

The transactional MCP `index` tool receives a request operation and deadline, but it is intentionally excluded from notification-driven cancellation until rollback and durable-success semantics are reviewed independently.

## Direct CLI Contract

Focused command parsers preserve their command arguments and text/JSON rendering. Read commands accept:

```text
--deadline-unix-ms <UNIX_MILLISECONDS>
```

A default may also be supplied through:

```text
ATHANOR_DEADLINE_UNIX_MS=<UNIX_MILLISECONDS>
```

The explicit command-line deadline has precedence over the environment. Malformed values fail closed before project access. Each intercepted command owns a process-local operation id and one live `CancellationHandle`.

Async reads are polled together with operation deadline, shared cancellation state, and `tokio::signal::ctrl_c()`. Preflight checks prevent already-expired work; postflight checks reject late success after cancellation.

Context and Search use drained interceptors because either command may build a search index on a blocking worker. The terminal response is delayed until the worker has completed or cooperatively observed termination.

## Blocking Worker Boundary

Graph builders, synchronous RusTok report builders, and search-index rebuilds run on Tokio's blocking pool after the committed snapshot has been loaded through a context-aware store read. The operation wrapper polls cancellation and deadline state while a builder runs.

A blocking task cannot be force-stopped safely. Cancellation therefore records terminal operation state but the wrapper drains the worker before returning. The completed value is discarded and cannot become a late successful response. This provides:

- no detached graph, RusTok report, or search rebuild worker after command termination;
- no CPU-bound graph or index construction on an async runtime worker;
- stable `cancelled` and `deadline_exceeded` output;
- bounded resource ownership during cancellation.

## Cooperative Graph And Search Work

Operation-aware related traversal, shortest-path traversal, PageRank, and directed-cycle search poll `OperationContext::check_active()` at bounded work intervals.

The built-in filesystem source and application diff scanner also poll active state while traversing directories, reading files, and hashing content.

Search-index rebuild uses an operation-aware factory. A rebuild is written to a sibling staging directory and the current index is switched only after the staged writer commits and the operation is still active. Failed or cancelled rebuilds preserve the prior current index.

The production composition registers the operation-aware Tantivy factory. Direct Search, Context, daemon Search/Context, and MCP Search/Context receive it through explicit composition paths.

## Cooperative Projector Publication

Wiki and HTML projectors check cancellation while rendering. Runtime defaults add an outer publication transaction:

- the inner projector writes a complete result into an operation-specific sibling staging directory;
- cancellation is checked after inner files exist and before the current target is touched;
- a late cancellation removes staging and preserves the prior target;
- a failed staging-to-target rename restores the prior directory;
- backup cleanup after successful swap is best-effort and cannot convert durable success into an error.

## Cooperative RusTok Operations

The operation-aware RusTok architecture-context path and FFA/FBA/Page Builder audit and graph paths preserve report schemas and deterministic ordering while polling active state during indexing, selection, evidence collection, diagnostics, graph assembly, and final report construction.

The outer blocking-worker drain remains the final protection against detached work and late successful responses.

## MCP Composition Contract

The CLI owns production composition creation:

```text
athanor_runtime_defaults::production()
  -> athanor_transport_mcp::run_mcp_server_with_composition(...)
```

The transport does not install process-global runtime factories. Its active module boundary is:

```text
runtime.rs
  -> server.rs
  -> tools/schema.rs
  -> tools/dispatch.rs
```

`server.rs` wraps `RuntimeComposition` in `Arc` and clones it into each request task. `tools/dispatch.rs` invokes only composition-aware application entry points for Index, Explain, Search, Context, Impact, Change Map, RusTok architecture context, and Check.

The former textual `include!("lib.rs")` compatibility dispatcher and the MCP-local global bootstrap have been removed.

## MCP Request Lifecycle

The server continues reading input while ordinary tool requests are active. Read requests are registered by serialized JSON-RPC id. The operation id is `mcp:<serialized-request-id>`, so duplicate active request identities fail closed through the cancellation lease.

Search, Context, Change Map, and RusTok architecture context use a drained registry path because they may own cooperative or blocking cleanup. Cancellation marks the shared operation but does not release the request lease or emit a response until the future has completed cleanup. Terminal cancellation/deadline state takes precedence over simultaneous worker error.

Normal tool failures remain successful MCP tool results with `isError: true`. Cancellation and deadline termination remain JSON-RPC errors. When stdin closes, every registered read lease is cancelled and request tasks are drained before the response writer exits.

## Compatibility

- Existing MCP tool names and input schemas are preserved.
- JSON-RPC responses may complete out of input order because request ids provide correlation.
- Notifications without an id remain response-free.
- Explicit `null` ids receive responses with `id: null`.
- Existing legacy pure application APIs may remain public for library compatibility, but the active MCP dispatcher does not call them.

## Remaining Work

- Decide transactional notification cancellation for MCP Index after rollback and durable-success review.
- Resolve control-plane responsiveness under full ordinary-request saturation (`MCP-004`).
- Execute hosted Linux/Windows formatting, tests, Clippy, stdio, disconnect, cancellation, and worker-drain regressions.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-transport-mcp --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo test -p athanor-app --test mcp_runtime_bootstrap_inventory --locked
cargo clippy -p athanor-transport-mcp --all-targets --locked -- -D warnings
```
