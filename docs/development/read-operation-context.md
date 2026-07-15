---
id: doc://docs/development/read-operation-context.md
kind: developer_guide
language: en
source_language: en
status: verified
---
# Read-Only Operation Context

## Scope

Athanor read-only ports and the daemon cache/query layer now share the same operation identity,
cancellation, and deadline contract already used by write jobs.

The first daemon slice covers:

- repository overview;
- entity explanation;
- indexed search;
- cached context generation (`diff=false`).

`Context` with `diff=true`, `ChangeMap`, direct CLI query execution, and MCP lifecycle propagation remain
follow-up work.

## Core Port Contract

`athanor-core` exports non-breaking extension traits for existing implementations:

```rust
CanonicalSnapshotStoreOperationExt
KnowledgeStoreQueryOperationExt
EntityResolverOperationExt
FactQueryStoreOperationExt
SearchIndexOperationExt
```

Each default extension performs:

1. `OperationContext::check_active()` before backend work;
2. the established legacy read method;
3. `check_active()` after backend work and before returning success.

This means a legacy implementation that completes after cancellation cannot produce a false successful
response. Backends with long-running internal loops may add finer-grained polling in their implementation;
the compatibility extension provides preflight and postflight safety without changing existing trait
implementers.

## Daemon Routing

`daemon_connection` sends valid supported read commands through a focused read dispatcher. Every other
command is delegated unchanged to the established daemon dispatcher.

For each intercepted request the dispatcher:

1. derives an `OperationContext` from command, request id, and optional deadline;
2. creates a cancellable daemon job and binds its registry token to that operation;
3. marks the job running before query execution;
4. runs the context-aware query/cache path;
5. enforces a hard Tokio timeout when a deadline exists;
6. checks cancellation/deadline at cache, store, search, and response boundaries;
7. finishes the job as succeeded, cancelled, or failed;
8. removes the cancellation token through normal job finalization.

A concurrent `Cancel` request can therefore move a running read job to `cancelling`, signal the shared core
operation, and produce public `cancelled` response details once the cooperative read boundary observes it.
A hard timeout produces a `CoreError::DeadlineExceeded` and public `deadline_exceeded` response instead of
an ambiguous successful payload.

## Cache Semantics

Snapshot, search-index, overview, and context cache hits are checked before return. Cache misses are checked
before backend initialization, after canonical snapshot load, after synchronous index open/rebuild, and
before cache insertion.

Search failures may still fall back to context generation without direct matches, preserving historical
behaviour. `Cancelled` and `DeadlineExceeded` are never treated as fallback-eligible search failures.

## Synchronous Boundary

Search-index open/rebuild is currently synchronous. The dispatcher hard timeout cannot preempt a thread
while that synchronous function is actively blocking; cancellation/deadline is checked immediately before
and after it. A later sandbox/process-runner slice may move expensive synchronous index construction behind
a cancellable worker boundary.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-core read_operation --locked
cargo test -p athanor-app search_operation --locked
cargo test -p athanor-app daemon_read_dispatch --locked
cargo test -p athanor-app daemon --locked
cargo clippy -p athanor-core --all-targets --locked -- -D warnings
cargo clippy -p athanor-app --all-targets --locked -- -D warnings
```

Required regressions:

- a pre-cancelled read is rejected before backend work;
- cancellation observed during backend work rejects a would-be successful result;
- an expired deadline is rejected before backend work;
- a running daemon search can be cancelled through the ordinary job cancellation registry;
- the public response code is `cancelled` and the job ends `cancelled`;
- a hard deadline returns `deadline_exceeded` and the job ends failed;
- successful compatibility read APIs remain available without an explicit context.

Hosted compile, test, Clippy, transport, and operating-system evidence remains separate and is not claimed
without recorded runs.
