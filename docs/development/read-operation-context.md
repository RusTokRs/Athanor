---
id: doc://docs/development/read-operation-context.md
kind: developer_guide
language: en
source_language: en
status: implemented
---
# Read-Only Operation Context

## Scope

Athanor read-only ports and the daemon query layer share the operation identity, cancellation, and
deadline contract used by write jobs.

The daemon slice covers repository overview, entity explanation, indexed search, cached and
diff-aware context generation, and change-map analysis. CLI, daemon and MCP active callers pass a
`RuntimeComposition` explicitly.

## Core Port Contract

`athanor-core` exports extension traits for existing backend implementations:

```rust
CanonicalSnapshotStoreOperationExt
KnowledgeStoreQueryOperationExt
EntityResolverOperationExt
FactQueryStoreOperationExt
SearchIndexOperationExt
```

Each default extension checks `OperationContext::check_active()` before backend work and again before
returning success. A backend result completed after cancellation or deadline expiry therefore cannot
be exposed as a successful application response. Backends with long-running internal loops may add
finer-grained polling.

## Application Services

Context and ChangeMap operation-aware application APIs require explicit runtime dependencies:

```rust,ignore
context_project_with_composition_and_operation_context(
    options,
    &composition,
    &operation,
).await?;

change_map_project_with_composition_and_operation_context(
    options,
    &composition,
    &operation,
).await?;
```

The former no-composition Context, ChangeMap and Search operation wrappers are removed. RusTok
operation execution is owned by `rustok_composition_operation.rs`; the former duplicate
`rustok_operation.rs` owner is physically deleted.

The postflight operation check rejects a result completed after cancellation or deadline expiry.
Synchronous filesystem discovery and bounded graph construction are not preempted inside a running
thread; the operation boundary is checked before success is exposed.

## Daemon Routing

`daemon_connection` routes reads through focused layers:

1. derived reads intercept diff-aware Context and ChangeMap;
2. the cache/query dispatcher handles Overview, Explain, Search and cached Context;
3. every other command is delegated to the bounded command dispatcher.

For each intercepted request the dispatcher derives an `OperationContext`, creates a cancellable
daemon job, binds the registry cancellation token, enforces a hard Tokio timeout when a deadline is
present, checks the operation before response exposure, and finalizes the job deterministically.

A concurrent `Cancel` request can move a running read job to `cancelling`, signal the shared core
operation, and produce public `cancelled` response details. A hard timeout produces
`CoreError::DeadlineExceeded` and the public `deadline_exceeded` response.

## Cache Semantics

Snapshot, search-index, overview and context cache hits are checked before return. Cache misses are
checked before backend initialization, after canonical snapshot load, after synchronous index
open/rebuild, and before cache insertion.

Search failures may fall back to context generation without direct matches. `Cancelled` and
`DeadlineExceeded` are never fallback-eligible search failures.

## Remaining Migration

`COMP-003C2B2C2B` removes the remaining Store facade and snapshot Search compatibility paths, then
migrates the read-service families that still accept optional or implicit composition. New code must
not call `store::init_store` or no-composition snapshot Search helpers.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-core read_operation --locked
cargo test -p athanor-app search_operation --locked
cargo test -p athanor-app derived_read_operation --locked
cargo test -p athanor-app --test context_composition_inventory --locked
cargo test -p athanor-app daemon_read_dispatch --locked
cargo test -p athanor-app daemon_derived_read_dispatch --locked
cargo test -p athanor-app daemon --locked
cargo clippy -p athanor-core --all-targets --locked -- -D warnings
cargo clippy -p athanor-app --all-targets --locked -- -D warnings
```

Required regressions cover preflight cancellation, cancellation observed during backend work,
expired deadlines, daemon search cancellation, stable public cancellation/deadline codes, and
composition-only derived read routing.

Hosted compile, test, Clippy, transport and operating-system evidence remains separate and is not
claimed without recorded runs.
