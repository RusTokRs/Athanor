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

Writer-side migration bridge implemented on `main`. New successful production indexing runs retain the
legacy mutable application artifacts for compatibility, publish an immutable application generation,
and atomically select it through a separate index pointer.

This pointer is intentionally independent from `.athanor/generated/current.json`, which belongs to the
on-demand `ath generate` command and coordinates JSONL, wiki, and HTML projection generations.

## Identity Contract

Every transactional index publication derives one backend-neutral identity:

```text
SnapshotId("snap_x") -> GenerationId("gen_snap_x")
```

The same `snapshot` and `generation` pair must appear in:

- the canonical JSONL commit marker or SurrealDB committed snapshot record;
- `.athanor/generated/current/jsonl/manifest.json` during the compatibility window;
- `.athanor/state/index-state.json` during the compatibility window;
- the immutable read-model manifest;
- the immutable index-state document;
- `.athanor/state/index-current.json`;
- both durable publication journals while recovery is pending.

A present mismatched generation fails closed. Legacy committed records that predate generation identity
remain readable only at explicitly documented migration boundaries.

## Current Layout

```text
.athanor/
  generated/
    current/jsonl/                         # legacy mutable compatibility output
    index-generations/
      gen_<snapshot>/jsonl/                # immutable transactional read model
  state/
    index-state.json                       # legacy mutable compatibility state
    index-state-gen_<snapshot>.json        # immutable transactional state
    index-current.json                     # single atomic application pointer
    index-publication.json                 # canonical/read-model/state coordinator journal
    index-current-publication.json         # immutable-copy/pointer bridge journal
```

`index-current.json` uses schema `athanor.index_current.v1` and contains deterministic relative paths.
Loading or writing the pointer requires both target artifacts to exist and to match the pointer schema,
snapshot, and generation identities.

## Publication Ordering

The migration bridge preserves the established coordinator and adds a second durable boundary:

1. Write `index-current-publication.json` before entering the legacy publication coordinator.
2. Write `index-publication.json` and stage the mutable read model and index state.
3. Atomically publish the canonical snapshot batch.
4. Finalize mutable compatibility artifacts and clear `index-publication.json`.
5. Validate the mutable artifact identities.
6. Copy them through sibling staging paths into deterministic immutable generation paths.
7. Validate the immutable artifacts.
8. Atomically replace `index-current.json`.
9. Clear `index-current-publication.json`.

Readers using `index-current.json` therefore observe either the previous complete generation or the new
complete generation. They never need to infer coherence by comparing two mutable paths.

## Recovery Rules

Recovery always runs under `.athanor/state/index-publication.lock` before a new index pipeline starts.

- If `index-publication.json` exists, the original coordinator first finalizes a committed canonical
  publication or restores previous mutable artifacts and aborts the unfinished snapshot.
- If `index-current-publication.json` then identifies a committed exact canonical snapshot, recovery
  recreates or reuses the immutable artifacts, validates them, rewrites the pointer, and clears the
  bridge journal.
- If only the bridge journal exists and the exact snapshot is not committed, recovery aborts that
  orphaned snapshot before clearing the journal. This covers a crash between the two journal writes.
- Existing immutable paths are never overwritten. A conflicting path is validated and rejected rather
  than repaired in place.
- Symlinks and unsupported filesystem entries are rejected while materializing an immutable read model.

## Compatibility Window

The legacy paths remain published because existing runtime and external readers still consume them.
They are source artifacts for the immutable bridge, not the long-term selection contract.

During this window:

- canonical visibility remains authoritative for commit status;
- `index-current.json` is the coherent application-generation selector for migrated readers;
- existing callers may continue to read mutable paths;
- the bridge journal makes a committed-but-not-yet-pointed generation recoverable.

## Verification

Targeted checks:

```bash
cargo test -p athanor-app index_current --locked
cargo test -p athanor-app index_current_runtime_tests --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo clippy -p athanor-app --all-targets -- -D warnings
```

The runtime coverage verifies:

- a normal production index writes a valid immutable generation and pointer;
- a committed bridge journal recreates missing immutable artifacts and the pointer;
- a bridge journal written before the legacy journal aborts an uncommitted orphan snapshot;
- a no-change index keeps the existing pointer stable.

## Next Slice: Pointer-First Readers

The next implementation slice must:

1. Resolve persisted incremental state through `index-current.json` when the pointer exists.
2. Resolve application JSONL reads through the pointer instead of the mutable compatibility path.
3. Keep an explicit fallback to legacy paths only when no pointer exists.
4. Add corruption tests for missing targets, schema mismatch, snapshot mismatch, and generation mismatch.
5. Update repair inspection and cleanup so pointed generations are retained and unpointed immutable
   generations are reported as orphans.
6. After all in-repository readers are migrated and one compatibility release has passed, stop writing
   the mutable compatibility paths and collapse publication to one immutable coordinator.

## Definition of Done for Migration Completion

- Every application reader resolves one validated `IndexCurrent` before opening state or read-model
  files.
- Canonical exact/latest identity and application pointer identity agree in tests for JSONL and
  SurrealDB backends.
- Recovery is idempotent at every journal and pointer fault point.
- Repair can distinguish current, recoverable, stale, and orphaned index generations.
- Legacy mutable read-model and state writes are removed only after the fallback has no remaining
  in-repository consumers.
