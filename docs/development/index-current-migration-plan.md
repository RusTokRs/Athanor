---
id: doc://docs/development/index-current-migration-plan.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Index Current Migration Plan

## Status

The writer-side migration bridge, pointer-first state readers, transactional repair inspection, explicit
index-generation retention, standalone publication recovery, cleanup fault recovery, and backend-neutral
canonical latest repair are implemented on `main`.

Successful production indexing still writes legacy mutable application artifacts during the compatibility
window, then publishes an immutable application generation and atomically selects it through
`.athanor/state/index-current.json`.

All standard `IndexStateStore::load()` callers resolve the validated pointer-selected immutable state. A
present invalid pointer fails closed; legacy paths are used only when the pointer is absent. Canonical
latest repair independently discovers the authoritative newest committed generation and never trusts a
mutable latest pointer as its source of truth.

## Identity Contract

Every transactional publication derives one backend-neutral identity:

```text
SnapshotId("snap_x") -> GenerationId("gen_snap_x")
```

The same pair must appear in:

- the JSONL canonical commit marker or SurrealDB committed snapshot record;
- compatibility read-model and index-state documents while they are still written;
- the immutable read-model manifest;
- the immutable index-state document;
- `index-current.json`;
- durable publication journals while recovery is pending;
- the normalized JSONL canonical latest pointer.

A mismatched generation fails closed. Legacy marker/state/latest formats remain readable only at explicit
migration boundaries.

## Current Layout

```text
.athanor/
  store/canonical/jsonl/
    latest.json                            # schema + snapshot + generation
    snapshots/<snapshot>/                  # immutable committed canonical generation
  generated/
    current/jsonl/                         # legacy mutable compatibility output
    index-generations/
      gen_<snapshot>/jsonl/                # immutable transactional read model
      .cleanup-gen_<snapshot>-<pid>-<ns>/  # staged confirmed cleanup
  state/
    index-state.json                       # legacy mutable compatibility state
    index-state-gen_<snapshot>.json        # immutable transactional state
    .cleanup-gen_<snapshot>-<pid>-<ns>.json
    index-current.json                     # single atomic application pointer
    index-publication.json                 # coordinator journal
    index-current-publication.json         # immutable-copy/pointer bridge journal
    index-publication.lock                 # shared index and repair mutation lock
```

`index-current.json` uses schema `athanor.index_current.v1` and deterministic relative paths. Loading or
writing it requires both target artifacts to exist and to match schema, snapshot, and generation.

JSONL `latest.json` uses schema `athanor.canonical_latest.v1`. Normal canonical queries still accept a
legacy snapshot-only pointer, but the repair port treats it as non-normalized so `repair-latest` upgrades
it explicitly.

## Publication Ordering

1. Write `index-current-publication.json`.
2. Write `index-publication.json` and stage mutable compatibility artifacts.
3. Atomically publish the canonical batch.
4. Finalize compatibility artifacts and clear the coordinator journal.
5. Validate compatibility identities.
6. Copy through sibling staging paths into deterministic immutable paths.
7. Validate immutable artifacts.
8. Atomically replace `index-current.json`.
9. Clear the bridge journal.

Readers observe either the previous complete generation or the new complete generation. They never infer
coherence by comparing independent mutable locations.

## Reader Resolution

```rust
athanor_app::publication::resolve_read_model_path(root)
athanor_app::publication::resolve_index_state_path(root)
```

Both resolvers validate `index-current.json` when present and fall back only when it is absent.
`IndexStateStore` uses this resolver for standard project-layout reads, migrating incremental indexing,
coverage, impact, context, check, capabilities, and validate-changed without duplicating pointer logic.

The repository currently has no production service that parses the exported JSONL read model directly;
canonical query services read through `CanonicalSnapshotStore`. The read-model resolver remains public
for external consumers and future application read models.

## Recovery Rules

All publication and destructive repair mutations use `.athanor/state/index-publication.lock`.

- Coordinator recovery finalizes committed canonical publication or restores compatibility artifacts and
  aborts an unfinished snapshot.
- A committed bridge journal recreates or reuses immutable artifacts, validates them, rewrites
  `index-current.json`, and clears the journal.
- A bridge journal for an uncommitted snapshot aborts that snapshot before clearing the journal.
- Existing immutable paths are never overwritten; conflicts fail closed.
- Symlinks and unsupported filesystem entries are rejected during immutable materialization.
- Cleanup tombstones are handled separately: a complete pair is deleted, a state-only tombstone is
  finalized, and a read-only tombstone with live state is rolled back to the live read-model path.
- Repeated publication and cleanup recovery is idempotent.

## Repair Inspection And Retention

`repair inspect` preserves canonical and `ath generate` report fields while adding transactional index
issues. It validates:

- pointer schema and deterministic paths;
- selected manifest/state schema, snapshot, and generation;
- equality with canonical latest when canonical latest is readable;
- pending bridge-journal identity;
- incomplete read-model/state pairs;
- orphaned generations;
- recoverable canonical-latest generations without an application pointer.

Retention is fail-closed:

- pointed and journal-referenced generations are always protected;
- a canonical-latest generation without a pointer is protected until pointer recovery;
- incomplete, pending, corrupt, stale, or canonically unresolved state blocks cleanup;
- retention has a separate `keep` count;
- dry-run returns a SHA-256 token derived from canonical root, `keep`, and the exact ordered orphan set;
- destructive cleanup requires the matching token;
- the two artifacts are sibling-renamed to tombstones before deletion;
- legacy `keep_generated` and `keep_canonical` flags never select transactional index generations.

```rust
let plan = cleanup_index_generations(IndexGenerationCleanupOptions {
    root: root.clone(),
    dry_run: true,
    keep: 1,
    confirmation_token: None,
})?;

cleanup_index_generations(IndexGenerationCleanupOptions {
    root,
    dry_run: false,
    keep: 1,
    confirmation_token: plan.confirmation_token,
})?;
```

## Backend-Neutral Canonical Latest Repair

The core port is `CanonicalLatestPointer`:

```rust
load_latest_identity()
discover_latest_identity()
validate_latest_identity(identity)
repair_latest_identity(identity)
```

`load_latest_identity` reads what the backend currently exposes. `discover_latest_identity` determines
the authoritative newest committed generation without trusting the mutable selector.

Backend semantics:

- **JSONL:** authoritative discovery scans immutable snapshot directories, selects the lexically newest
  zero-padded snapshot id, then requires a valid canonical manifest and commit marker v2 containing the
  matching `GenerationId`. Repair atomically rewrites `latest.json` under the JSONL writer lock.
- **Memory:** latest is derived from committed records. Repair is validation-only and accepts only the
  actual newest committed generation.
- **SurrealDB:** latest is derived from committed rows. Repair is validation-only and preserves exact
  committed-generation checks.

The repair API prefers an explicit snapshot, then a fully validated `IndexCurrent`, then backend discovery.
Any requested target must equal backend discovery, so repair cannot rewind visibility to an older committed
snapshot. Apply acquires the project publication lock, validates, repairs, reloads, and verifies the exact
identity.

## Standalone Repair Commands

Only new transactional repair commands are intercepted by the small CLI entry wrapper; existing Clap
commands are delegated unchanged.

```bash
ath repair index-retention . --dry-run --keep 1 --json
ath repair index-retention . --keep 1 --confirmation-token 'sha256:...'

ath repair recover-index . --dry-run --json
ath repair recover-index .

ath repair recover-index-cleanup . --dry-run --json
ath repair recover-index-cleanup .

ath repair repair-latest . --dry-run --json
ath repair repair-latest .
ath repair repair-latest . --snapshot snap_jsonl_00000042
```

`recover-index` rechecks journals under the publication lock before mutation. `recover-index-cleanup`
processes only pre-existing confirmed tombstones. `repair-latest` accepts an explicit snapshot only when
backend discovery confirms it is authoritative.

## Compatibility Window

- canonical visibility remains authoritative for commit status;
- `index-current.json` selects the coherent application generation;
- in-repository state readers are pointer-first;
- external callers can use validated resolvers;
- legacy mutable read-model/state paths remain written temporarily;
- legacy JSONL latest remains queryable but is normalized by repair or the next successful publication.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-core latest_pointer --locked
cargo test -p athanor-app index_current --locked
cargo test -p athanor-app repair_latest --locked
cargo test -p athanor-app repair_cleanup_recovery --locked
cargo test -p ath --test repair_cli --locked
cargo test -p athanor-store-memory latest_pointer --locked
cargo test -p athanor-store-jsonl latest --locked
cargo test -p athanor-store-surrealdb latest_pointer --locked
cargo test -p athanor-app index_current_runtime_tests --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo clippy -p athanor-app --all-targets -- -D warnings
cargo clippy -p ath --all-targets -- -D warnings
```

Regression coverage includes:

- complete pointer publication and pointer-first reads;
- journal recovery before and after canonical commit;
- exact retention-token enforcement;
- corruption matrix for pointer/artifact/journal identity;
- full, read-only, and state-only cleanup tombstones;
- live/tombstone conflicts and repeated cleanup recovery;
- missing, malformed, legacy, and stale JSONL latest pointers;
- JSONL repair normalization and idempotence;
- Memory/SurrealDB derived latest validation;
- explicit old-snapshot rewind rejection;
- CLI help, JSON reports, and failure exit codes.

## Next Slice: Application Artifact Checksums

The final generation-layer code slice must:

1. Add deterministic checksums for every immutable JSONL read-model file.
2. Add a checksum for the immutable index-state document.
3. Bind expected digests into the immutable manifest and `index-current.json`.
4. Verify digests before returning selected artifact paths.
5. Make repair and recovery fail closed on digest mismatch, missing files, or path substitution.
6. Add tamper, missing-file, repeated recovery, and migration-window regressions.
7. Keep hosted compile/test/Clippy and remote backend evidence separate from code completion.

## Definition of Done for Migration Completion

- Every application reader resolves one validated `IndexCurrent` before opening state or read-model files.
- Canonical exact/latest identity and application pointer identity agree in JSONL and SurrealDB tests.
- Recovery is idempotent at every journal, pointer, and cleanup fault point.
- Repair distinguishes current, recoverable, stale, incomplete, orphaned, and staged-cleanup generations.
- Immutable application artifacts are checksum-bound before legacy mutable writes are removed.
