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
- stable-key entity lookup;
- backend-neutral fact filtering by subject, object, kind, extractor, and limit;
- relation and diagnostic filtering;
- invisibility of uncommitted and prepared snapshots;
- complete observable publication after commit;
- rejection of abort after commit;
- removal of aborted snapshots without changing `LatestCommitted`;
- atomic replacement of partial staged canonical data by one complete committed batch.

The core-owned `FactQuery` and `FactQueryStore` are additive to `KnowledgeStore`. Every
`CanonicalSnapshotStore` receives the same blanket committed-only implementation, so canonical
backends cannot silently diverge on filter or limit semantics. `athanor-app::query` re-exports the
request and trait as part of its stable read-only surface.

Memory, JSONL, and embedded SurrealDB run the same query, lifecycle, and atomic-publication
conformance functions. Dedicated backend regressions additionally cover filesystem marker behavior,
transaction rollback, and remote independent-reader visibility. Server-dependent remote tests remain
isolated from the normal workspace and `--all-features` graph.

## Atomic canonical data and commit marker

`athanor-core::AtomicSnapshotPublication` is an additive capability for stores that can publish a
complete `SnapshotBatch` and its committed marker through one backend-specific atomic boundary.
The context-aware default checks cancellation and deadline before entering that boundary and does not
re-check them after success, because durable publication has already happened and must not be
mistaken for a rollback candidate.

### Memory

Memory provides the reference implementation. One mutex critical section replaces any partial
staged contents with the complete batch and marks the snapshot committed. Before that section,
exact reads fail with `SnapshotNotCommitted`; after it, exact and latest reads expose only the new
complete generation. Republish and abort of the committed snapshot fail closed.

### JSONL

JSONL writes all JSONL data, indexes, manifest, and `athanor.canonical_commit.v1` marker into a
hidden staging directory. One directory rename publishes the exact committed generation. Any
previous prepared directory is removed before this boundary, and process state becomes committed
immediately after the rename.

Atomic manifests declare `commit_marker_schema`. Exact and latest reads validate the canonical
manifest schema and snapshot identity. When the declaration is present, missing, malformed,
wrong-schema, or foreign-snapshot `commit.json` fails closed. A legacy manifest without the
declaration remains readable; if an undeclared marker is present it is still validated rather than
ignored.

`latest.json` remains a separate pointer layer. A pointer finalization error may be returned after the
exact generation is already committed, but that snapshot remains readable by exact id and cannot be
aborted. This prevents a post-commit pointer error from being misclassified as an uncommitted
canonical generation.

### SurrealDB

SurrealDB replaces all rows for one snapshot and sets the snapshot record to
`prepared = true, committed = true` inside one `BEGIN ... COMMIT` query. The transaction deletes old
entity/fact/relation/diagnostic rows, inserts the complete replacement batch, and updates the marker.
The response is passed through `check()`, so any statement error rolls back both rows and marker.

The public facade classifies only confirmed transaction conflicts as `CoreError::Busy` and retries
the complete atomic boundary with the existing bounded `10/25/50/100 ms` schedule. Non-conflict
data or serialization failures remain non-retryable. Embedded regressions require duplicate-record
failure to leave the snapshot uncommitted and permit a later valid atomic publication.

The configured remote two-client test performs the same atomic publication through one connection,
then loads the committed entity/fact data and executes `FactQueryStore` through the independent
reader. This becomes evidence only after a successful hosted run.

## SurrealDB writer safety

The locked backend is SurrealDB `2.6.5`. Snapshot sequence allocation uses one atomic
`UPDATE ONLY`; latest selection orders by numeric sequence before the historical record-id fallback.
SurrealKV protects a persistent database directory with an operating-system exclusive lock, so
embedded persistent storage remains a single-owner process model. A second independent connection
to the same directory must fail closed.

Confirmed ownership and transaction-conflict messages map to retryable `CoreError::Busy`. Data
validation, duplicate records, serialization failures, and unrelated statement errors remain
non-retryable adapter failures.

## Remote server conformance

Remote support is opt-in through the `remote` crate feature. The configured hosted job starts
`surrealdb/surrealdb:v2.6.5`, then explicitly runs ignored tests against
`ATHANOR_SURREAL_REMOTE_URI`.

The remote suite uses two independent SDK connections and checks:

- 32 concurrent snapshot allocations produce unique identifiers;
- one connection atomically publishes a complete snapshot batch and marker;
- the independent connection loads the committed entity/fact data;
- the independent reader retrieves the committed fact through `FactQueryStore`.

These checks become evidence only after a successful hosted run. They do not deterministically force
a real server write conflict or resolve the semantics of a request whose outcome becomes ambiguous
after transport failure.

## Deadline- and cancellation-bounded retry

Only context-aware write and publication methods retry `CoreError::Busy`. The delay schedule is
10, 25, 50, and 100 milliseconds. Every retry attempt checks deadline and process-local cancellation
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

This compatibility protocol remains active in the production index coordinator. The next cutover
must pass the complete `SnapshotBatch` into `AtomicSnapshotPublication` instead of writing canonical
rows before the durable application journal exists.

## Typed index publication coordinator

Production indexing is routed through `index_runtime.rs`. The current deferred pipeline writes and
prepares canonical data first, then constructs `PreparedSnapshot` and calls the guarded
`publish_prepared_index` boundary.

The coordinator:

1. writes `athanor.index_publication.v2`;
2. stages the JSONL read model and index state;
3. publishes the previously prepared canonical snapshot;
4. finalizes application backups;
5. clears the durable journal.

Journal v1 remains readable and is normalized to the typed v2 representation. Unknown schemas and
fields fail closed. Journal paths must equal the expected project artifacts, and publication ids
cannot contain path separators.

Startup calls guarded `recover_interrupted_publication` before loading previous state. A committed
journal keeps the new generation and removes stale staging/backups. An uncommitted journal restores
available backups, removes matching uncommitted current artifacts, and calls `abort_prepared`.
Recovery currently determines canonical commit by the latest committed snapshot. The atomic cutover
must add exact-snapshot commit probing so a committed exact generation is recognized even if a
separate latest pointer failed.

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

Deterministic regressions cover journal/prepare/publish/finalize/clear failures, malformed artifact
types, schema and identity mismatches, mixed backup generations, absent backups, repeated recovery,
and simultaneous publish/rollback/abort failures.

For post-commit finalize failures, the canonical snapshot remains committed. After the transient
filesystem fault is repaired, recovery finalizes the same generation instead of reverting it. A
second recovery call after journal cleanup is a no-op. Combined failures preserve the original
publish error together with rollback and abort causes.

## Guarantees not claimed yet

The current implementation does not claim:

- production index-coordinator cutover to `AtomicSnapshotPublication`;
- exact canonical commit probing during publication recovery;
- one immutable generation pointer covering canonical data, state, and read models;
- cryptographic content integrity for application artifacts;
- deterministic remote write-conflict evidence;
- force interruption or outcome recovery for an SDK request already executing;
- complete cancellation propagation for read-only daemon, CLI, and MCP operations;
- hosted compile, test, formatting, Clippy, AppSec, installer, or release evidence while Actions runs
  remain unavailable.

P0.4 remains incomplete until hosted evidence, production coordinator cutover, exact-marker recovery,
generation-pointer switching, remote conflict evidence, and pointer-level fault injection are
complete.
