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
index-generation retention, standalone publication recovery, cleanup fault recovery, backend-neutral
canonical latest repair, and application artifact checksum chain are implemented on `main`.

Successful production indexing still writes legacy mutable application artifacts during the compatibility
window, then publishes an immutable application generation and atomically selects it through checksum-bound
`.athanor/state/index-current.json` schema v2.

All standard `IndexStateStore::load()` callers resolve the validated pointer-selected immutable state. A
present invalid pointer fails closed; legacy paths are used only when the pointer is absent. Canonical
latest repair independently discovers the authoritative newest committed generation and never trusts a
mutable latest pointer as its source of truth.

## Identity And Integrity Contract

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

`index-current.v2` additionally binds the generation to:

- the SHA-256 digest of the immutable read-model manifest;
- the SHA-256 digest of the immutable index-state document.

The manifest contains the exact digest map for `diagnostics.jsonl`, `entities.jsonl`, `facts.jsonl`, and
`relations.jsonl`. Missing, extra, nested, symlinked, or non-regular entries fail closed. A mismatched
generation or digest fails closed. Legacy marker/state/latest formats and `index-current.v1` remain readable
only at explicit migration boundaries.

## Current Layout

```text
.athanor/
  store/canonical/jsonl/
    latest.json                            # schema + snapshot + generation
    snapshots/<snapshot>/                  # immutable committed canonical generation
  generated/
    current/jsonl/                         # legacy mutable compatibility output
    index-generations/
      gen_<snapshot>/jsonl/                # sealed immutable transactional read model
      .cleanup-gen_<snapshot>-<pid>-<ns>/  # staged confirmed cleanup
  state/
    index-state.json                       # legacy mutable compatibility state
    index-state-gen_<snapshot>.json        # immutable transactional state
    .cleanup-gen_<snapshot>-<pid>-<ns>.json
    index-current.json                     # v2 identity + manifest/state digests
    index-publication.json                 # coordinator journal
    index-current-publication.json         # immutable-copy/pointer bridge journal
    index-publication.lock                 # shared index and repair mutation lock
```

New `index-current.json` writes use schema `athanor.index_current.v2`, deterministic relative paths,
`read_model_manifest_sha256`, and `index_state_sha256`. Loading or writing v2 requires both target artifacts
to exist, match schema/snapshot/generation, and pass the complete checksum chain. Schema v1 is accepted only
as a temporary identity-only migration pointer and must not contain checksum fields.

JSONL `latest.json` uses schema `athanor.canonical_latest.v1`. Normal canonical queries still accept a
legacy snapshot-only pointer, but the repair port treats it as non-normalized so `repair-latest` upgrades it
explicitly.

## Publication Ordering

1. Write `index-current-publication.json`.
2. Write `index-publication.json` and stage mutable compatibility artifacts.
3. Atomically publish the canonical batch.
4. Finalize compatibility artifacts and clear the coordinator journal.
5. Validate compatibility identities.
6. Copy through sibling staging paths into deterministic immutable paths.
7. Validate immutable identities and require any reused generation to match the compatibility source.
8. Seal the immutable manifest with the exact four-file SHA-256 map.
9. Hash the sealed manifest and immutable state document.
10. Validate the complete v2 pointer contract and atomically replace `index-current.json`.
11. Clear the bridge journal.

Readers observe either the previous complete generation or the new checksum-bound generation. They never
infer coherence by comparing independent mutable locations. An unpointed generation may exist while the
bridge journal is pending, but migrated readers cannot select it before the v2 pointer switch.

## Reader Resolution

```rust
athanor_app::publication::resolve_read_model_path(root)
athanor_app::publication::resolve_index_state_path(root)
```

Both resolvers validate `index-current.json` when present and fall back only when it is absent. For v2 they
verify the manifest digest, exact file set, every JSONL file digest, and the immutable state digest before
returning either path.

`IndexStateStore` uses this resolver for standard project-layout reads, migrating incremental indexing,
coverage, impact, context, check, capabilities, and validate-changed without duplicating pointer logic.

The repository currently has no production service that parses the exported JSONL read model directly;
canonical query services read through `CanonicalSnapshotStore`. The read-model resolver remains public for
external consumers and future application read models.

## Recovery Rules

All publication and destructive repair mutations use `.athanor/state/index-publication.lock`.

- Coordinator recovery finalizes committed canonical publication or restores compatibility artifacts and
  aborts an unfinished snapshot.
- A committed bridge journal recreates or reuses immutable artifacts, validates them, seals the manifest,
  rewrites `index-current.v2`, and clears the journal.
- Reused read-model data files and immutable state must match the validated compatibility source before
  recovery may calculate new digests.
- A bridge journal for an uncommitted snapshot aborts that snapshot before clearing the journal.
- Existing immutable paths are never overwritten with conflicting content; conflicts fail closed.
- Symlinks, nested directories, unknown extra files, and unsupported filesystem entries are rejected.
- Cleanup tombstones are handled separately: a complete pair is deleted, a state-only tombstone is
  finalized, and a read-only tombstone with live state is rolled back to the live read-model path.
- Repeated publication, checksum sealing, pointer publication, and cleanup recovery are idempotent.

## Repair Inspection And Retention

`repair inspect` preserves canonical and `ath generate` report fields while adding transactional index
issues. It validates:

- pointer schema and deterministic paths;
- selected manifest/state schema, snapshot, and generation;
- v2 manifest/state digests and every manifest-listed JSONL digest;
- exact equality between the checksum file map and the actual direct file set;
- equality with canonical latest when canonical latest is readable;
- pending bridge-journal identity;
- incomplete read-model/state pairs;
- orphaned generations;
- recoverable canonical-latest generations without an application pointer.

Checksum, missing-file, extra-file, and filesystem-type failures are reported as
`index_current_checksum_mismatch`. Retention treats that issue as unsafe publication state.

Retention is fail-closed:

- pointed and journal-referenced generations are always protected;
- a canonical-latest generation without a pointer is protected until pointer recovery;
- incomplete, pending, corrupt, stale, checksum-invalid, or canonically unresolved state blocks cleanup;
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

`load_latest_identity` reads what the backend currently exposes. `discover_latest_identity` determines the
authoritative newest committed generation without trusting the mutable selector.

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
snapshot. A tampered v2 application generation cannot nominate canonical latest because `IndexCurrent::load`
verifies its checksum chain first.

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
- `index-current.v2` selects and checksum-binds the coherent application generation;
- in-repository state readers are pointer-first;
- external callers can use validated resolvers;
- legacy mutable read-model/state paths remain written temporarily;
- `index-current.v1` remains identity-readable only during the migration window;
- legacy JSONL latest remains queryable but is normalized by repair or the next successful publication.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-core latest_pointer --locked
cargo test -p athanor-app artifact_checksum --locked
cargo test -p athanor-app index_current --locked
cargo test -p athanor-app repair_latest --locked
cargo test -p athanor-app repair_cleanup_recovery --locked
cargo test -p athanor-app index_current_runtime_tests --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p ath --test repair_cli --locked
cargo test -p ath --test index_checksum_cli --locked
cargo test -p ath --test index_checksum_recovery_cli --locked
cargo test -p athanor-store-memory latest_pointer --locked
cargo test -p athanor-store-jsonl latest --locked
cargo test -p athanor-store-surrealdb latest_pointer --locked
cargo clippy -p athanor-app --all-targets --locked -- -D warnings
cargo clippy -p ath --all-targets --locked -- -D warnings
```

Regression coverage includes:

- complete v2 pointer publication and pointer-first reads;
- journal recovery before and after canonical commit;
- upgrade of a pre-checksum generation and repeated idempotent recovery;
- manifest/data/state tamper detection;
- missing and extra read-model file rejection;
- source-to-immutable mismatch rejection before sealing;
- exact retention-token enforcement;
- corruption matrix for pointer/artifact/journal identity;
- full, read-only, and state-only cleanup tombstones;
- live/tombstone conflicts and repeated cleanup recovery;
- missing, malformed, legacy, and stale JSONL latest pointers;
- JSONL repair normalization and idempotence;
- Memory/SurrealDB derived latest validation;
- explicit old-snapshot rewind rejection;
- CLI help, JSON reports, and failure exit codes.

## Code Completion And Remaining Evidence

The generation-layer implementation is code-complete:

1. backend-neutral immutable generation identity;
2. one atomic application current pointer;
3. backend latest repair;
4. pointer/cleanup fault recovery;
5. checksum-bound application artifacts.

Remaining release evidence is external to this code slice: hosted compile/test/Clippy across the feature
matrix, Windows filesystem behaviour, embedded and remote SurrealDB runs, required checks, and branch
protection.

## Definition of Done for Migration Completion

- Every application reader resolves one validated `IndexCurrent` before opening state or read-model files.
- Canonical exact/latest identity and application pointer identity agree in JSONL and SurrealDB tests.
- Recovery is idempotent at every journal, checksum, pointer, and cleanup fault point.
- Repair distinguishes current, recoverable, stale, incomplete, orphaned, checksum-invalid, and
  staged-cleanup generations.
- Immutable application artifacts are checksum-bound before legacy mutable writes are removed.
