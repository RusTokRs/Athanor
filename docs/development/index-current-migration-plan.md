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

Writer-side migration bridge and pointer-first index-state readers are implemented on `main`. New
successful production indexing runs retain the legacy mutable application artifacts for compatibility,
publish an immutable application generation, and atomically select it through a separate index
pointer.

All standard `IndexStateStore::load()` callers now resolve the validated pointer-selected immutable
state. The application also exports validated read-model and index-state path resolvers through
`athanor_app::publication`. A present invalid pointer fails closed; the legacy paths are used only when
no index pointer exists.

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

## Reader Resolution

The public application resolver contract is:

```rust
athanor_app::publication::resolve_read_model_path(root)
athanor_app::publication::resolve_index_state_path(root)
```

Both functions load and validate `index-current.json` when it exists. They return the legacy path only
when the pointer is absent. They do not hide pointer corruption, missing targets, unsupported schemas,
or identity mismatches.

`IndexStateStore` keeps compatibility writes directed at its configured path, but its standard project
layout read path is resolved through `resolve_index_state_path`. This migrates coverage, impact,
change-map, context, check, capabilities, validate-changed, and incremental indexing state reads
without duplicating pointer logic in each service.

The repository currently has no production service that parses the exported JSONL read model directly;
canonical query services read through `CanonicalSnapshotStore`. The read-model resolver is exported for
external consumers, repair, and future application read models.

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

The legacy paths remain published because existing external readers may still consume them. They are
source artifacts for the immutable bridge, not the long-term selection contract.

During this window:

- canonical visibility remains authoritative for commit status;
- `index-current.json` is the coherent application-generation selector for migrated readers;
- standard in-repository state readers are pointer-first;
- external callers can use the exported validated resolvers;
- the bridge journal makes a committed-but-not-yet-pointed generation recoverable.

## Verification

Targeted checks:

```bash
cargo test -p athanor-app index_current --locked
cargo test -p athanor-app index_state_pointer --locked
cargo test -p athanor-app index_current_runtime_tests --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo clippy -p athanor-app --all-targets -- -D warnings
```

The runtime coverage verifies:

- a normal production index writes a valid immutable generation and pointer;
- a committed bridge journal recreates missing immutable artifacts and the pointer;
- a bridge journal written before the legacy journal aborts an uncommitted orphan snapshot;
- a no-change index keeps the existing pointer stable;
- standard state reads prefer pointed immutable state;
- no pointer falls back to legacy state;
- a present pointer with incomplete artifacts fails closed.

## Next Slice: Repair And Retention

The next implementation slice must:

1. Update `repair inspect` to validate `index-current.json`, its target artifacts, and pending bridge
   journal state.
2. Report unpointed immutable index generations separately from `ath generate` generations.
3. Define retention for immutable index generations before allowing `repair cleanup` to remove them.
4. Keep the pointed generation and any generation referenced by a pending journal unconditionally.
5. Add corruption tests for unsupported pointer schema, path mismatch, snapshot mismatch, generation
   mismatch, missing manifest, and missing state.
6. After all external consumers have a compatibility release using the resolvers, stop writing the
   mutable compatibility paths and collapse publication to one immutable coordinator.

## Definition of Done for Migration Completion

- Every application reader resolves one validated `IndexCurrent` before opening state or read-model
  files.
- Canonical exact/latest identity and application pointer identity agree in tests for JSONL and
  SurrealDB backends.
- Recovery is idempotent at every journal and pointer fault point.
- Repair can distinguish current, recoverable, stale, and orphaned index generations.
- Legacy mutable read-model and state writes are removed only after the fallback has no remaining
  in-repository consumers.
