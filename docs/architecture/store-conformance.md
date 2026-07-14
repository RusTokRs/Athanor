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
publish, and abort.

## Shared contract

`athanor-store-conformance` provides reusable async checks for:

- exact and latest committed snapshot selection;
- stable-key, relation, and diagnostic query behavior;
- invisibility of uncommitted and prepared snapshots;
- complete observable publication after commit;
- rejection of abort after commit;
- removal of aborted snapshots without changing `LatestCommitted`.

The dedicated `Store Conformance` workflow is configured for Memory, JSONL, and embedded SurrealDB.
Server-dependent remote tests remain isolated from the normal workspace and `--all-features` graph.

## SurrealDB transaction boundary

The locked backend is SurrealDB `2.6.5`. Athanor uses one SurrealQL query containing `BEGIN`, bulk
`INSERT` statements, and `COMMIT` for a complete `SnapshotBatch`. The response is passed through
`check()` so statement-level errors cannot be mistaken for success.

The batch contains entities, facts, relations, and diagnostics. A duplicate-record regression
requires the transaction to fail without partial rows, then retries the same identifiers
successfully. Snapshot sequence allocation uses one atomic `UPDATE ONLY`; latest selection orders by
the numeric sequence before the historical record-id fallback.

Counter allocation, snapshot-record creation, batch insertion, and the later commit marker are still
separate backend requests. The batch transaction is therefore not yet an atomic generation
publication protocol.

## Embedded persistent ownership

SurrealKV protects a persistent database directory with an operating-system exclusive lock. Athanor
treats embedded persistent storage as a single-owner process model. A second independent connection
to the same `surrealkv://` directory must fail closed.

Confirmed ownership and transaction-conflict messages map to retryable `CoreError::Busy`. Data
validation, duplicate records, serialization failures, and unrelated statement errors remain
non-retryable adapter failures.

## Remote server conformance

Remote support is opt-in through the `remote` crate feature. The configured hosted job starts
`surrealdb/surrealdb:v2.6.5`, then explicitly runs ignored tests against
`ATHANOR_SURREAL_REMOTE_URI`.

The remote suite uses two independent SDK connections and checks:

- 32 concurrent snapshot allocations produce unique identifiers;
- one connection can publish a snapshot loaded by the other;
- entity and fact records are visible across clients after commit.

These checks become evidence only after a successful hosted run. They do not deterministically force
a real server write conflict.

## Deadline- and cancellation-bounded retry

Only context-aware write and publication methods retry `CoreError::Busy`. The delay schedule is
10, 25, 50, and 100 milliseconds. Every attempt checks deadline and process-local cancellation
state. Backoff polling uses intervals no larger than five milliseconds.

`CancellationHandle` registration acts as an exclusive process-local lease for one
`operation_id`. Independent live authorities with the same id receive `CoreError::Conflict`; callers
clone an existing handle when one operation has multiple owners. The id becomes reusable after all
clones are dropped.

The daemon operation-aware scheduler binds the application `CancellationToken` to the same core
handle. Index, generation, wiki, and HTML jobs use this path. Repeating the same token/id binding is
idempotent, while a different token with the same live id fails closed.

## Prepared publication protocol

`athanor-core` owns `PreparedSnapshot` and `PreparedSnapshotPublication` beside `KnowledgeStore`.
`athanor-app` preserves source compatibility through a re-export.

The protocol is:

1. `prepare_publication(snapshot, context)` crosses the backend prepare boundary and returns a typed
   cleanup authority;
2. `publish_prepared(handle, context)` performs context-aware canonical publication;
3. `abort_prepared(handle)` uses a cancellation-independent plain abort path.

A cancellation race immediately after successful prepare must still return the typed handle. Losing
that handle would strand durable prepared state without cleanup authority.

Memory, JSONL, and embedded SurrealDB regressions require publish to advance the latest committed
view and require aborting a later prepared snapshot to preserve the previous latest generation.

## Typed index publication coordinator

Production indexing is routed through `index_runtime.rs`. The deferred pipeline prepares canonical
data first, then the runtime constructs `PreparedSnapshot` and calls the guarded
`publish_prepared_index` boundary.

The coordinator:

1. writes `athanor.index_publication.v2`;
2. stages the JSONL read model and index state;
3. publishes canonical data through `publish_prepared`;
4. finalizes application backups;
5. clears the durable journal.

Journal v1 remains readable and is normalized to the typed v2 representation. Unknown schemas and
fields fail closed. Journal paths must equal the expected project artifacts, and publication ids
cannot contain path separators.

Startup calls guarded `recover_interrupted_publication` before loading previous state. A committed
journal keeps the new generation and removes stale staging/backups. An uncommitted journal restores
available backups, removes matching uncommitted current artifacts, and calls `abort_prepared`.

The public `index` API remains stable. The former monolithic `crates/athanor-app/src/index.rs` has
been deleted after its incremental, validation, cancellation, and equivalence tests were migrated to
focused production-runtime modules.

## Recovery preflight and fault matrix

The guard validates every recovery-controlled artifact before any delete or rename:

- current, staged, and backup read models must be directories when present;
- current, staged, and backup index states must be regular files when present;
- read-model manifests must be parseable, use `athanor.jsonl_manifest.v1`, and carry a non-empty
  snapshot identity;
- new current and staged index state must use the active `INDEX_STATE_SCHEMA`;
- historical state backups may use an earlier numeric `athanor.index_state.vN` schema, including a
  validated version suffix, so recovery remains possible across an application upgrade;
- staged artifacts must identify the journal snapshot;
- committed current artifacts must identify the journal snapshot;
- uncommitted current artifacts that would be replaced by backups must identify the journal snapshot;
- read-model and index-state backups must identify the same previous generation when both exist.

A type, schema, parse, or identity mismatch fails closed before destructive mutation. The durable
journal and current artifacts remain present for diagnosis and repair.

Deterministic regressions cover:

- journal persistence failure before a recovery record exists;
- read-model prepare failure;
- index-state prepare failure;
- cancellation immediately before canonical publish;
- read-model finalize failure after canonical commit;
- index-state finalize failure after canonical commit;
- journal clear failure after canonical commit;
- malformed read-model and index-state backup types;
- committed and uncommitted current identity mismatches;
- mixed read-model/index-state backup generations;
- recovery with no backups for an uncommitted generation;
- repeated committed and uncommitted recovery after cleanup;
- simultaneous canonical publish, application rollback, and canonical abort failures.

For post-commit finalize failures, the canonical snapshot remains latest committed. After the
transient filesystem fault is repaired, recovery finalizes the same generation instead of reverting
it. A second recovery call after journal cleanup is a no-op. Combined failures preserve the original
publish error together with rollback and abort causes.

## Guarantees not claimed yet

The current implementation does not claim:

- one backend transaction for canonical data and the commit marker;
- one immutable generation pointer covering canonical data, state, and read models;
- cryptographic content integrity for application artifacts;
- deterministic remote write-conflict evidence;
- force interruption of an SDK request already executing;
- complete cancellation propagation for read-only daemon, CLI, and MCP operations;
- hosted compile, test, formatting, Clippy, AppSec, installer, or release evidence while Actions runs
  remain unavailable.

P0.4 remains incomplete until hosted evidence, backend-neutral fact queries, data-plus-marker
publication, generation-pointer switching, remote conflict evidence, and pointer-level fault
injection are complete.
