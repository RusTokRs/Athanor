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

Writer-side migration bridge, pointer-first index-state readers, repair inspection, and an explicit
index-generation retention API are implemented on `main`. New successful production indexing runs retain
the legacy mutable application artifacts for compatibility, publish an immutable application generation,
and atomically select it through a separate index pointer.

All standard `IndexStateStore::load()` callers now resolve the validated pointer-selected immutable
state. The application also exports validated read-model and index-state path resolvers through
`athanor_app::publication`. A present invalid pointer fails closed; the legacy paths are used only when
no index pointer exists.

`repair inspect` augments the established canonical and `ath generate` inspection with validation of
`index-current.json`, its selected artifacts, pending bridge-journal state, incomplete index generations,
and unpointed immutable index generations. The public `cleanup_index_generations` API provides a separate
two-step retention protocol; the existing `keep_generated` counter never selects transactional index
generations.

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

## Repair Inspection And Retention Safety

`repair inspect` returns schema `athanor.repair_inspect.v2` and preserves the previous canonical and
`ath generate` report fields. Index publication findings are emitted as separate repair issues, so they
cannot be confused with `.athanor/generated/generations` findings.

Inspection verifies:

- pointer schema, snapshot-derived generation, deterministic read-model path, and deterministic state
  path;
- selected manifest and state existence plus schema/snapshot/generation identity;
- equality between the selected pointer snapshot and the latest canonical snapshot;
- a present index pointer has a readable canonical latest pointer and resolvable authoritative snapshot;
- pending `index-current-publication.json` schema and snapshot-derived generation;
- read-model generations without matching immutable state, and state files without matching read-model
  generations;
- generations referenced by neither the current pointer nor a pending journal;
- an unpointed generation matching canonical latest is classified as `recoverable_index_generation`, not
  as a removable orphan.

Current retention policy is fail-closed:

- the pointed generation is always protected;
- a generation referenced by a pending bridge journal is always protected;
- a canonical-latest generation without a pointer is protected until pointer recovery;
- incomplete, pending, corrupt, stale, or canonically unresolved publication state blocks cleanup;
- the explicit API accepts its own `keep` count and returns structured `removed` and `retained` rows;
- dry-run returns a SHA-256 confirmation token derived from the canonical root, retention count, and exact
  ordered orphan set;
- destructive cleanup requires the matching token, so changed filesystem state invalidates approval;
- read-model and index-state members are staged by sibling renames before deletion;
- existing `keep_canonical`, `keep_generated`, and `generated_only` flags never select immutable index
  generations.

The library contract is:

```rust
use athanor_app::{cleanup_index_generations, IndexGenerationCleanupOptions};

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

CLI wiring is intentionally the next isolated commit because `apps/ath/src/main.rs` is still a large
monolithic dispatch file and must be changed from a complete verified source snapshot.

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
cargo test -p athanor-app index_state --locked
cargo test -p athanor-app valid_pointer_generation_is_clean --locked
cargo test -p athanor-app pointer_without_canonical_latest_blocks_cleanup --locked
cargo test -p athanor-app dry_run_token_is_required_for_the_exact_plan --locked
cargo test -p athanor-app corruption_matrix_fails_closed --locked
cargo test -p athanor-app canonical_latest_generation_is_recoverable_not_removable --locked
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
- a present pointer with incomplete artifacts fails closed;
- repair considers a complete pointed generation clean;
- repair reports but retains an unpointed generation;
- a pending bridge journal blocks cleanup;
- a present index pointer without a valid canonical latest snapshot blocks cleanup;
- the corruption matrix covers unsupported pointer schema, path mismatch, stale snapshot, generation
  mismatch, missing manifest, missing state, malformed journal, and half-published generations;
- destructive retention requires a token from the exact dry-run plan;
- canonical-latest generations without a pointer are treated as recoverable.

## Next Slice: CLI Wiring And Standalone Recovery

The next implementation slice must:

1. Expose the explicit index retention plan/apply protocol through `ath repair` without reusing
   `--keep-generated`.
2. Add CLI help, JSON, token mismatch, and text-rendering tests.
3. Add repair recovery for a committed pending bridge journal without requiring a full indexing run.
4. Add fault injection around the pair-rename/tombstone cleanup boundary and idempotent tombstone cleanup.
5. Update the main P0.4 checklist after the CLI and standalone recovery paths are verified.
6. After all external consumers have a compatibility release using the resolvers, stop writing the
   mutable compatibility paths and collapse publication to one immutable coordinator.

## Definition of Done for Migration Completion

- Every application reader resolves one validated `IndexCurrent` before opening state or read-model
  files.
- Canonical exact/latest identity and application pointer identity agree in tests for JSONL and
  SurrealDB backends.
- Recovery is idempotent at every journal and pointer fault point.
- Repair can distinguish current, recoverable, stale, incomplete, and orphaned index generations.
- Legacy mutable read-model and state writes are removed only after the fallback has no remaining
  in-repository consumers.
