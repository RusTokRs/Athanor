---
id: doc://docs/development/direct-operation-context.md
kind: developer_guide
language: en
source_language: en
status: implemented
---
# Direct CLI And MCP Operation Context

## Scope

This document describes the shared cancellation and deadline contract used by direct CLI operations
and the MCP stdio transport. The source implementation is present in `main`; formatting, tests,
Clippy, and executable interoperability remain pending for the current HEAD.

Covered direct CLI command families include Context, Explain, Overview, Impact, Change Map, Search,
Check, standard Graph reads, RusTok audit/graph reads, and Index.

The MCP lifecycle provides:

- explicit `RuntimeComposition` supplied by the CLI composition root;
- concurrent JSON-RPC request execution with bounded request and response capacity;
- request-scoped cancellation for read tools and transactional Index;
- `notifications/cancelled` and legacy `$/cancelRequest` notifications;
- cancellation of registered operations when stdin reaches EOF;
- request deadlines through `deadline_unix_ms`;
- stable cancellation and deadline error categories;
- durable Index success when cancellation races after canonical commit;
- a control-plane path that remains readable while ordinary request and response capacity is full.

## Direct CLI Contract

Focused command parsers preserve command arguments and text/JSON rendering. Operation-aware commands
accept:

```text
--deadline-unix-ms <UNIX_MILLISECONDS>
```

A default may also be supplied through:

```text
ATHANOR_DEADLINE_UNIX_MS=<UNIX_MILLISECONDS>
```

The explicit command-line deadline has precedence over the environment. Malformed values fail closed
before project access. Each intercepted command owns a process-local operation identity and one live
`CancellationHandle`.

Async reads are polled together with operation deadline, shared cancellation state, and
`tokio::signal::ctrl_c()`. Preflight checks reject already-terminated work; postflight checks reject
late read success after cancellation. Context and Search use drained interceptors because either may
own cooperative or blocking cleanup.

## Transactional Index Boundary

Index has a different terminal contract from read-only operations. Its durable commit point is
successful exact canonical snapshot publication through
`AtomicSnapshotPublication::publish_snapshot_batch_with_context`.

Before that boundary:

- source, extractor, linker, checker, and Store boundaries check
  `OperationContext::check_active()` before and after bounded work;
- cancellation or deadline failure rolls back prepared read-model and index-state artifacts;
- the uncommitted canonical allocation is aborted;
- publication journals are cleared when cleanup succeeds;
- no successful `IndexReport` is exposed.

At the commit race:

- backend `Cancelled` and `DeadlineExceeded` returns are reconciled by loading the exact snapshot;
- an absent or uncommitted snapshot preserves the terminal error and rollback path;
- an exact committed snapshot preserves durable success;
- an identity mismatch or failed state probe remains an error rather than guessing.

After durable commit:

- read-model and index-state finalization continues;
- the application current pointer is published or left journal-recoverable;
- cancellation state does not replace the successful `IndexReport`;
- cleanup retains existing recovery semantics.

The MCP server registers Index in the request cancellation registry, so notification cancellation
reaches the same operation. Unlike read-only operations, the transport awaits the Index application
future directly and performs no external polling or postflight after completion. Daemon Index uses
the same report and may transition from `cancelling` to `succeeded` after durable publication.

## Blocking And Cooperative Work

Graph builders, synchronous RusTok report builders, and search-index rebuilds run on Tokio's blocking
pool after a committed snapshot has been loaded. A blocking task cannot be force-stopped safely, so
the wrapper records terminal state, drains the worker, and discards a completed value after
cancellation.

Operation-aware graph traversal, filesystem discovery, diff scanning, search-index rebuild, and
RusTok report assembly poll `OperationContext::check_active()` at bounded intervals. Search rebuilds
use a sibling staging directory and switch the current index only while the operation remains active.

Wiki and HTML projectors write complete staged output before replacing the target. Cancellation before
the swap removes staging and preserves the prior target. Backup cleanup after a successful swap is
best effort and cannot convert durable success into failure.

## MCP Composition Contract

The CLI owns production composition creation:

```text
athanor_runtime_defaults::production()
  -> athanor_transport_mcp::run_mcp_server_with_composition(...)
```

The transport does not install process-global runtime factories. Active ownership is split across:

```text
runtime.rs
  -> server/lifecycle.rs
  -> server/protocol.rs
  -> server/operation.rs
  -> server/types.rs
  -> tools/schema.rs
  -> tools/dispatch.rs
```

`tools/dispatch.rs` calls composition-aware application entry points. Index specifically uses
`index_project_with_composition_and_operation_context`.

## MCP Request And Control-Plane Lifecycle

The server continues reading stdin while ordinary requests execute. Registered operations are keyed
by serialized JSON-RPC id. Duplicate live request identities fail closed through the cancellation
lease.

### Ordinary Request Capacity

Ordinary requests consume the bounded `RequestTasks` set. When the configured in-flight limit is
reached, an additional ordinary request receives retryable `server_busy` when response capacity is
available. Ordinary request tasks retain bounded `send().await` backpressure when publishing their
terminal response.

### Control Input Priority

The stdin branch appears before request-task reaping in the biased `tokio::select!`. A line that is
already readable is therefore processed before another completed ordinary task is reaped.

Notifications are dispatched before ordinary request admission. `notifications/cancelled`,
`$/cancelRequest`, and notification-only session messages do not consume a request slot and do not
produce responses.

### Saturated Response Queue

Responses produced directly by the reader loop—parse errors, initialize responses, and overload
rejections—must not await a full response queue. They use nonblocking admission:

- if capacity is available, the response is queued;
- if the queue is full or closed, the response is logged and dropped;
- the stdin reader continues, so cancellation notifications and EOF remain observable.

This fail-open rule is limited to inline control-loop responses. It does not make the response queue
unbounded and does not remove backpressure from ordinary request tasks.

### Disconnect

When stdin reaches EOF, the server marks input closed and cancels every registered operation before
waiting for request tasks to drain. This applies even when ordinary request slots are saturated.
After tasks finish, the response sender is dropped and the writer task flushes the remaining bounded
queue.

## Compatibility

- Existing MCP tool names and input schemas are preserved.
- Index output remains the public `athanor.index_report.v1` payload shared by CLI and daemon paths.
- JSON-RPC responses may complete out of input order because request ids provide correlation.
- Notifications without an id remain response-free.
- Explicit `null` ids receive responses with `id: null`.
- Overload rejection remains retryable when it can be admitted to the bounded response queue.

## Remaining Work

- Execute the Rust formatting, test, Clippy, stdio, disconnect, cancellation, saturation, and
  worker-drain matrix on one current commit.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-app pipeline_support --locked
cargo test -p athanor-app store_publication_cancellation --locked
cargo test -p athanor-app index_publication_fault_tests --locked
cargo test -p athanor-transport-mcp server::operation --locked
cargo test -p athanor-transport-mcp server::tests --locked
cargo test -p athanor-transport-mcp --test index_publication_cancellation_inventory --locked
cargo test -p athanor-transport-mcp --test control_plane_saturation_inventory --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo clippy -p athanor-app --all-targets --locked -- -D warnings
cargo clippy -p athanor-transport-mcp --all-targets --locked -- -D warnings
```
