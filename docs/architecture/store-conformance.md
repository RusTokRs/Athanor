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
retryable category is used for the confirmed SurrealKV messages `Transaction write conflict` and
`Transaction retry required`. `CoreError::Busy` is transport-neutral and reports `is_retryable() ==
true`. Data validation, duplicate-record, serialization, and other statement failures remain
`CoreError::Adapter` and are not retryable.

A persistent-path regression opens one connection, attempts a second independent connection to the
same directory, and requires `Busy`. This proves exclusive embedded ownership and error
classification. It does not prove concurrent remote-server transaction behavior.

## Prepare semantics

SurrealDB snapshot metadata records `prepared` separately from `committed`. Once prepared, a snapshot
rejects subsequent individual writes and batch writes. Prepare is idempotent before commit; committed
snapshots remain immutable and cannot be aborted.

## Guarantees not claimed yet

This slice does not make the whole lifecycle one transaction:

- counter allocation and snapshot-record creation are separate backend requests, so a crash can leave
  a harmless sequence gap;
- the atomic batch transaction does not include the later commit marker;
- abort removes canonical rows transactionally, then deletes snapshot metadata in a separate request;
- embedded SurrealKV intentionally prevents a second owner, while two independent connections to a
  remote persistent SurrealDB server are not covered yet;
- canonical data, generated state, and read models still do not switch through one generation pointer.

P0.4 remains incomplete until remote independent-writer conflicts, commit-marker publication, fault
injection, and generation-level recovery are covered by tests.
