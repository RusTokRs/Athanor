---
id: doc://docs/architecture/store-conformance.md
kind: architecture
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Store Conformance and Transaction Boundaries

Athanor treats canonical snapshots as a backend-neutral contract. Memory, JSONL, and SurrealDB
stores must agree on what readers can observe while a snapshot moves through begin, write, prepare,
commit, and abort.

## Shared contract

`athanor-store-conformance` provides reusable async checks for:

- exact and latest committed snapshot selection;
- stable-key, relation, and diagnostic query behavior;
- invisibility of uncommitted and prepared snapshots;
- complete observable publication after commit;
- rejection of abort after commit;
- removal of aborted snapshots without changing `LatestCommitted`.

The dedicated `Store Conformance` workflow runs package tests for Memory, JSONL, and the embedded
SurrealDB engine on every pull request and push to `main`.

## SurrealDB transaction boundary

The locked backend is SurrealDB `2.6.5`. Athanor uses the SurrealDB 2.x manual-transaction path:
one `.query()` containing `BEGIN`, bulk `INSERT` statements, and `COMMIT`. The returned response is
passed through `check()` so a statement-level error is surfaced instead of being mistaken for a
successful request.

`KnowledgeStore::put_snapshot` serializes all entities, facts, relations, and diagnostics before
opening the transaction. Non-empty object arrays are inserted inside one backend transaction. A
duplicate record or any other statement error rolls the transaction back. A regression test submits
a duplicate entity ID, requires the batch to fail, and then retries the same ID successfully to prove
that the failed transaction left no partial record.

The snapshot counter is initialized idempotently and incremented with one atomic `UPDATE ONLY`
statement. Snapshot records carry the numeric sequence, and latest-snapshot selection orders by that
sequence before falling back to the historical record ID order. A concurrency test uses separate
process-local writer gates over the same backend client and requires 32 unique allocations.

## Embedded persistent ownership

SurrealKV protects a persistent database directory with an operating-system exclusive lock. Athanor
therefore treats embedded persistent storage as a single-owner process model: the first connection
owns the directory and a second independent connection to the same `surrealkv://` path must fail
closed.

The public store facade translates the confirmed lock message into `CoreError::Busy`. The same
retryable category is used for the confirmed SurrealKV messages `Transaction write conflict`,
`Transaction retry required`, and `Transaction conflict:`. Data validation, duplicate-record,
serialization, and other statement failures remain non-retryable adapter errors.

A persistent-path regression opens one connection, attempts a second independent connection to the
same directory, and requires `Busy`. This proves exclusive embedded ownership and error
classification. It does not prove concurrent remote-server transaction behavior.

## Remote server conformance

Remote support is opt-in through the crate feature `remote`, which enables the SurrealDB WebSocket
protocol without changing the default embedded dependency graph. The dedicated CI job starts the
matching `surrealdb/surrealdb:v2.6.5` server in an ephemeral Docker container and passes
`ws://127.0.0.1:8000` through `ATHANOR_SURREAL_REMOTE_URI`.

Remote integration tests are ignored by default. This keeps workspace `--all-features` tests
self-contained while allowing the dedicated job to execute them explicitly with `--ignored`.
Configured remote checks cover:

- 32 concurrent snapshot allocations split across two independent SDK connections;
- uniqueness of every allocated snapshot ID;
- publication by one connection and canonical loading by another;
- cross-connection visibility of both entity and fact records after commit.

These checks prove shared-server visibility and independent-client allocation behavior once the
hosted job succeeds. They do not deterministically force a server transaction conflict.

## Deadline- and cancellation-bounded retry policy

Only context-aware write and publication methods retry `CoreError::Busy`. The schedule is bounded to
four delays: 10, 25, 50, and 100 milliseconds. Before every attempt the operation deadline and
cancellation state are checked. If the remaining budget cannot fit the next delay, the current `Busy`
error is returned without sleeping past the deadline. Non-retryable errors fail on the first attempt.

`athanor-core` exposes a cloneable `CancellationHandle` through
`OperationContextCancellation::cancellation_handle()`. The process-local cancellation state is keyed
by the stable operation id and excluded from `OperationContext` serialization. Callers that create
handles must therefore use operation identities that are unique while work is active. Daemon request
contexts use `daemon.<command>.<request_id>`; watcher index jobs are serialized by the single-active
index guard.

SurrealDB backoff polls `OperationContext::check_active()` at intervals no larger than five
milliseconds. Cancellation stops before the next backend attempt and interrupts a pending backoff
with bounded latency. It does not abort a backend request that is already executing.

## Daemon cancellation bridge

The application `CancellationToken` owns shared state containing an optional core
`CancellationHandle`. The operation-aware daemon scheduler binds the token before inserting it into
the daemon cancellation registry. Cancelling either the registry clone or the running-task clone sets
both the application flag and the core operation state.

Index, coordinated generation, wiki projection, and HTML report jobs use this scheduler path. For
index jobs, the exact bound `OperationContext` is passed to `begin_snapshot`, `put_snapshot`,
`prepare_snapshot`, and `commit_snapshot`. Rollback uses a plain abort path so cleanup is not skipped
because the user cancelled the originating operation.

The compatibility scheduler without an operation context remains available for tests and legacy
jobs. Read-only daemon commands, CLI lifecycle cancellation, and MCP cancellation are not covered by
this bridge yet.

## Prepared publication handle

`athanor-app` now exposes `PreparedSnapshot` and the backend-neutral
`PreparedSnapshotPublication` extension over `KnowledgeStore`.

The explicit protocol is:

1. `prepare_publication(snapshot, context)` checks the request before prepare, runs the backend
   context-aware prepare method, and returns an opaque handle containing the snapshot identity;
2. `publish_prepared(handle, context)` runs the context-aware commit path;
3. `abort_prepared(handle)` deliberately uses the plain abort path so rollback remains possible after
   deadline expiry or user cancellation.

Once the backend prepare succeeds, `prepare_publication` returns the handle even if cancellation races
immediately afterward. A second post-prepare cancellation check would discard the only typed cleanup
authority after durable lifecycle state had changed. Publication still checks the context before
commit, while rollback remains independent of the cancelled request budget.

The handle serializes as the snapshot identity, which permits future recovery journals to persist it
without encoding backend-specific details. Existing stores remain compatible because the extension
delegates to their current lifecycle methods.

`AthanorStore` forwards every context-aware write/publication method to its inner backend. Before this
fix, the trait defaults on the wrapper could bypass SurrealDB retry and cancellation overrides. A
recording-store regression requires batch write, prepare, and publish to use the context-aware backend
methods while rollback uses plain abort.

A real JSONL regression now exercises the typed protocol end to end. Publishing a prepared snapshot
must advance `LatestCommitted`; preparing and aborting a later snapshot must leave the previously
published snapshot selected. Memory and SurrealDB typed-protocol regressions remain to be added to the
same matrix.

The current index publication coordinator still stores a raw snapshot id in its journal and calls
`commit_snapshot` directly. Migrating that coordinator to persist and consume `PreparedSnapshot` is
the next required step; the new API alone does not make data and the commit marker atomic.

## Prepare semantics

SurrealDB snapshot metadata records `prepared` separately from `committed`. Once prepared, a snapshot
rejects subsequent individual writes and batch writes. Prepare is idempotent before commit; committed
snapshots remain immutable and cannot be aborted.

## Guarantees not claimed yet

This slice does not make the whole lifecycle one transaction:

- counter allocation and snapshot-record creation are separate backend requests, so a crash can leave
  a harmless sequence gap;
- the atomic batch transaction does not include the later commit marker;
- the index coordinator has not yet migrated from raw `SnapshotId` to `PreparedSnapshot`;
- abort removes canonical rows transactionally, then deletes snapshot metadata in a separate request;
- a real remote write conflict has not yet been reproduced and observed through the public facade;
- an already executing backend request is not force-aborted by the retry wrapper;
- read-only daemon commands, CLI, and MCP are not yet connected to the same cancellation lifecycle;
- canonical data, generated state, and read models still do not switch through one generation pointer.

P0.4 remains incomplete until coordinator migration, hosted remote evidence, an observed remote
conflict, commit-marker publication, fault injection, and generation-level recovery are covered by
tests.