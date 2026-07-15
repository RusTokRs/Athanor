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
stores must agree on what readers can observe while a snapshot moves through allocation, write,
prepare, atomic publication, recovery, and abort.

## Shared contract

`athanor-store-conformance` provides reusable async checks for:

- exact and latest committed snapshot selection;
- stable-key entity lookup;
- backend-neutral fact filtering by subject, object, kind, extractor, and limit;
- relation and diagnostic filtering;
- invisibility of uncommitted and prepared snapshots;
- complete observable publication after commit;
- rejection of republish and abort after commit;
- removal of aborted snapshots without changing `LatestCommitted`;
- atomic replacement of partial staged data by one complete committed batch.

The core-owned `FactQuery`/`FactQueryStore` implementation is shared by every
`CanonicalSnapshotStore`, so canonical backends cannot diverge on filter or limit semantics.
Memory, JSONL, and embedded SurrealDB run the same query, lifecycle, and atomic-publication suites.
Remote server tests remain opt-in and are evidence only after a successful hosted run.

## Atomic canonical data and marker

`athanor-core::AtomicSnapshotPublication` owns three related backend capabilities:

1. context-aware snapshot allocation when the backend persists allocation records;
2. bounded recovery of stale uncommitted/unprepared allocation records;
3. publication of a complete `SnapshotBatch` and its committed marker through one backend boundary.

The publication context is checked before a durable boundary and is not rechecked after success.
A successful commit must never be converted into a cancellation error and then rolled back.

### Memory

Memory is the reference implementation. One mutex section replaces staged contents with the complete
batch and marks the snapshot committed. Exact reads fail before that section; exact/latest reads expose
the complete generation after it. Memory has no durable allocation record, so orphan allocation
recovery is a no-op.

### JSONL

JSONL writes data, indexes, manifest, and `athanor.canonical_commit.v1` into a hidden staging
directory. One rename publishes the exact generation. Atomic manifests declare
`commit_marker_schema`; exact/latest reads reject a missing, malformed, wrong-schema, or
foreign-snapshot marker. Legacy manifests without that declaration remain readable, while an
undeclared marker is still validated when present.

`latest.json` remains a separate pointer. A pointer finalization error may occur after exact commit,
but the generation remains exact-readable and non-abortable. JSONL allocation updates only its
sequence/process state and does not persist an empty snapshot generation, so process death before
publication does not leave a durable allocation record.

### SurrealDB

SurrealDB replaces all rows for one snapshot and sets `prepared = true, committed = true` in one
SurrealQL transaction. Statement failure rolls back both rows and marker. The facade retries the
entire boundary only for classified `Busy` conflicts.

Context-aware allocation creates the snapshot record with:

- `allocation_operation_id` from `OperationContext`;
- `allocation_created_at_unix_ms`;
- `committed = false` and `prepared = false`.

Before the next context-aware allocation for the same repo, the backend performs a bounded orphan
sweep:

- only records carrying allocation metadata are eligible;
- records must be at least 24 hours old;
- records must still be `committed = false` and `prepared = false`;
- the destructive delete repeats repo, age, committed, and prepared predicates;
- at most 128 records are considered per allocation call;
- committed and prepared records are never removed;
- repeated cleanup is idempotent.

The public recovery method also accepts an explicit cutoff and limit for deterministic repair/tests.
Legacy untagged records are intentionally not removed automatically because ownership and age cannot
be proven fail-closed.

## Deferred canonical data barrier

The production `AthanorStore` buffers a context-aware complete `SnapshotBatch` only in shared
process-local state. While that batch is pending:

- `put_snapshot_with_context` does not write backend rows;
- `prepare_snapshot_with_context` does not create a durable prepared marker;
- deferred `IndexPipeline` output has no exact JSONL generation or prepared directory;
- compatibility commit flushes the pending batch through `AtomicSnapshotPublication`;
- coordinator publication discards the compatibility batch and publishes the explicit validated
  `IndexPipelineOutput` batch;
- abort clears the pending batch and delegates cancellation-independent backend cleanup.

The remaining pre-journal durable state is allocation authority. JSONL has no durable empty
generation, while SurrealDB now tags and bounds recovery of stale context-owned allocation records.

## Production coordinator

Production indexing is routed through `index_runtime.rs` and `index_publication_atomic.rs`:

1. recover an interrupted publication under the project publication lock;
2. run deferred extraction/linking/checking with only process-local canonical data;
3. write `athanor.index_publication.v2`;
4. stage read model and index state;
5. build the complete canonical batch from pipeline output;
6. call the backend atomic data+marker boundary;
7. exact-probe the journal snapshot after an error;
8. retain journal/application artifacts for exact committed recovery;
9. otherwise roll back artifacts, clear the journal, and abort;
10. finalize backups and clear the journal after success.

Recovery uses exact canonical identity rather than only `LatestCommitted`. This preserves a JSONL
exact commit when a separate latest-pointer update fails.

## Recovery preflight

Before any application-artifact delete or rename, recovery verifies:

- expected directory/file types;
- parseable read-model and index-state schemas;
- non-empty snapshot identities;
- current/staging identity equals the journal snapshot where applicable;
- read-model and state backups identify the same previous generation;
- historical state backup schema has a strict numeric `athanor.index_state.vN` form.

Type, schema, parse, or identity mismatch fails closed. Deterministic regressions cover journal,
prepare, publish, finalize, clear, malformed artifacts, absent backups, repeated recovery, exact
commit/latest-pointer failure, and combined publish/rollback/abort errors.

## Guarantees not claimed yet

The implementation does not yet claim:

- one immutable generation pointer covering canonical data, index state, and read models;
- automatic cleanup of legacy SurrealDB allocation records without ownership metadata;
- automatic JSONL latest-pointer repair through the backend-neutral recovery API;
- cryptographic content integrity for application artifacts;
- deterministic remote write-conflict evidence;
- force interruption/outcome recovery for an SDK request already executing;
- full cancellation propagation for read-only daemon, CLI, and MCP operations;
- hosted compile, test, formatting, Clippy, AppSec, installer, or release evidence while Actions runs
  remain unavailable.

P0.4 remains partial until hosted evidence exists, the prepared transition layer is removed, and one
immutable generation pointer coordinates canonical data, state, and read models.
