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

The application `TransientKnowledgeStore` implements the same replacement semantics so internal
composition and test paths satisfy the production store capability bound.

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

## Prepared publication compatibility

`athanor-core` still owns `PreparedSnapshot` and `PreparedSnapshotPublication`. Journal v1/v2 and
existing cleanup/fault fixtures retain this typed handle as an abort authority.

The production coordinator no longer uses `publish_prepared` as its final canonical boundary. It
constructs a complete `SnapshotBatch` from `IndexPipelineOutput` and invokes
`publish_snapshot_batch_with_context` after the durable journal and application staging exist.
Prepared handles remain useful before that boundary: a journal-write, read-model prepare, state
prepare, cancellation, or uncommitted atomic failure can still abort the snapshot through the plain
cancellation-independent cleanup path.

## Production atomic index coordinator

Production indexing remains routed through `index_runtime.rs`, while `index_publication_atomic.rs`
now owns the active publication and recovery functions. The former guard and inner coordinator stay
compiled as an explicitly allowed compatibility layer for journal types and legacy fault tests.

The active coordinator:

1. validates the prepared handle against the pipeline output;
2. writes `athanor.index_publication.v2`;
3. stages the JSONL read model and index state;
4. builds the complete canonical `SnapshotBatch` from output entities, facts, relations, and
   diagnostics;
5. calls `AtomicSnapshotPublication`;
6. probes the journal snapshot by exact canonical id if publication returns an error;
7. keeps the journal and staged application artefacts when the exact generation is already committed;
8. otherwise rolls application artefacts back, clears the journal, and aborts the snapshot;
9. finalizes application backups and clears the journal after a clean atomic publish.

Recovery also probes the journal snapshot by exact id. It no longer relies only on
`LatestCommitted`, so a JSONL exact generation remains recognized as committed when a separate
`latest.json` finalization fails. Committed recovery keeps the matching read model/state generation
and removes stale backups; uncommitted recovery restores backups and aborts the snapshot.

A dedicated regression blocks `latest.json` after exact JSONL publication. The coordinator must
return the pointer error while retaining the journal, exact generation, read-model manifest, and
index state. Abort must fail with `Conflict`; after removing the transient pointer fault, recovery
cleans the journal without reverting application artefacts, and repeated recovery is a no-op.

The remaining crash window is earlier: the deferred pipeline still writes and prepares uncommitted
canonical rows before the durable publication journal is created. A process crash in that interval
can strand prepared state without a journal. The next cutover must make deferred execution return the
complete batch without calling `put_snapshot_with_context` or `prepare_publication` first.

## Recovery preflight and fault matrix

Recovery validates every controlled application artifact before any delete or rename:

- current, staged, and backup read models must be directories when present;
- current, staged, and backup index states must be regular files when present;
- read-model manifests must be parseable, use `athanor.jsonl_manifest.v1`, and carry a non-empty
  snapshot identity;
- new current and staged index state must use the active `INDEX_STATE_SCHEMA`;
- historical state backups may use an earlier numeric `athanor.index_state.vN` schema with a
  validated suffix;
- staged artifacts must identify the journal snapshot;
- committed current artifacts must identify the journal snapshot;
- uncommitted current artifacts that would be replaced by backups must identify the journal snapshot;
- read-model and index-state backups must identify the same previous generation when both exist.

A type, schema, parse, or identity mismatch fails closed before destructive mutation. The durable
journal and current artifacts remain present for diagnosis and repair.

Deterministic regressions cover journal/prepare/atomic-publish/finalize/clear failures, malformed
artifact types, schema and identity mismatches, mixed backup generations, absent backups, repeated
recovery, exact-commit/latest-pointer failure, and simultaneous publish/rollback/abort failures.

## Guarantees not claimed yet

The current implementation does not claim:

- zero canonical mutation before the durable application publication journal;
- one immutable generation pointer covering canonical data, state, and read models;
- automatic repair of a failed JSONL `latest.json` pointer from the backend-neutral recovery API;
- cryptographic content integrity for application artifacts;
- deterministic remote write-conflict evidence;
- force interruption or outcome recovery for an SDK request already executing;
- complete cancellation propagation for read-only daemon, CLI, and MCP operations;
- hosted compile, test, formatting, Clippy, AppSec, installer, or release evidence while Actions runs
  remain unavailable.

P0.4 remains incomplete until deferred prewrite is removed, hosted evidence exists, one generation
pointer is introduced, remote conflict behavior is evidenced, and pointer-level fault injection is
complete.
