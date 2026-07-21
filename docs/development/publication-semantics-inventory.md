---
id: doc://docs/development/publication-semantics-inventory.md
kind: architecture_inventory
language: en
source_language: en
status: implemented
---
# Publication durability inventory

Status: implementation complete; execution evidence is pending.

## Invariant

A publication has three distinct phases:

1. **Stage** — build a complete candidate outside the visible target.
2. **Commit** — atomically rename the candidate into the visible target or publish an immutable generation.
3. **Maintenance** — remove backups, stale staging paths, or completed journals.

After the commit point succeeds, maintenance failure must not turn durable success into an ordinary publication failure. It must be reported as a warning or as an explicit recovery-visible maintenance state. Destructive cleanup and rollback operations are different: removal is their requested result, so cleanup failures remain errors.

## Inventory

| Owner | Operation | Commit point | Post-commit policy | Status |
| --- | --- | --- | --- | --- |
| `athanor-projector-support` | `publish_staged_directory` | staging directory renamed to target | backup cleanup warns, result stays successful | fixed in `main` |
| `athanor-projector-support` | `replace_output_file` | staging file renamed to target | backup cleanup warns, result stays successful | fixed in `main` |
| `athanor-app/daemon_endpoint.rs` | daemon endpoint replacement | generic `replace_output_file` commit | inherits best-effort backup cleanup | fixed in `main` |
| `athanor-app/runtime/plugin_trust_registry.rs` | adapter trust registry | staging file renamed to registry path | backup cleanup warns | already safe |
| `athanor-runtime-defaults/projector_operation.rs` | cancellable Wiki/HTML output | staging directory renamed to target | backup cleanup is best effort | already safe |
| `athanor-search-tantivy` | search index rebuild | staging index installed and reopened | backup cleanup is best effort; open failure rolls back | already safe |
| `athanor-store-jsonl/atomic_publication.rs` | immutable canonical generation | staging directory renamed to immutable snapshot path | no post-commit cleanup capable of changing result | already safe |
| `athanor-store-jsonl/pointer_publication.rs` | strict latest pointer and snapshot sequence | staged file renamed to target | valid previous file cleanup is best effort; invalid non-file target fails before commit | fixed by `PUB-005` |
| `athanor-app/index_publication.rs` | immutable read model/state generation | immutable targets and checksummed current pointer published | journal retained when pointer publication is incomplete | recovery-visible |
| `athanor-app/read_model.rs` | prepared current read model | staging directory renamed to current output | `finalize` warns on backup cleanup failure and returns the published report | fixed in `main` |
| `athanor-app/index_state.rs` | prepared current index state | staging file renamed to current state | `finalize` warns on backup cleanup failure and returns success | fixed in `main` |
| `athanor-app/project_registry.rs` | project registry state | staging file renamed to registry path | backup cleanup warns and the published registry remains successful | fixed in `main` |
| `athanor-app/index_publication_journal.rs` | publication journal | staging file renamed to journal path | backup cleanup warns and journal publication remains successful | fixed in `main` |
| `athanor-app/repair_retention.rs` | confirmed generation deletion | both live artifacts renamed to tombstones | tombstone removal is the requested destructive operation and remains strict | intentional strict cleanup |
| `athanor-app/repair_cleanup_recovery.rs` | cleanup recovery | tombstones removed or restored | failures remain recovery-visible and retryable | intentional strict recovery |
| `apps/athd` | log rotation | active log renamed into rotation chain | operational rotation, not durable state publication | out of PUB scope |

## JSONL store structure

`athanor-store-jsonl` no longer uses the `strict_latest.rs -> include!("lib.rs") -> include!("store.rs")` compatibility chain. The crate has explicit modules for lifecycle, canonical reads, latest identity, pointer publication, snapshot I/O, indexes, commit markers and state orchestration. Both latest and snapshot-sequence writes call the same pointer publication implementation.

## Failure regression coverage

Implemented in `publication_failure_semantics.rs`:

- [x] staged build failure preserves the previous visible directory and removes temporary siblings;
- [x] failure after the previous target moved to backup restores that target and removes publication siblings;
- [x] an unpublished immutable candidate is removed by `Drop`;
- [x] a target created after staging wins the race and the rejected candidate is removed.

Implemented in `athanor-projector-support` unit tests:

- [x] a deterministic `cfg(test)` fault hook injects backup cleanup failure after the commit rename;
- [x] publication still returns success and exposes the new target;
- [x] the previous target remains in backup for recovery and a warning is emitted.

Implemented in `athanor-store-jsonl` tests:

- [x] pointer cleanup failure after commit returns success, exposes the new pointer and retains the backup;
- [x] a non-file pointer target fails before pointer commit;
- [x] strict latest writing, legacy query compatibility, repair normalization, sequence allocation and index persistence survive the module split;
- [x] module-boundary regression forbids compatibility includes and duplicate pointer writers.

Implemented in `publication_semantics_inventory.rs`:

- [x] all four `PUB-004` app owners retain their warning path;
- [x] the shared JSONL pointer helper owns cleanup semantics for both pointer clients;
- [x] former post-commit cleanup error strings and JSONL include wrappers must not reappear.

Implemented in `publication_recovery_matrix.rs`:

- [x] failure while staging the second retention tombstone requires rollback of the first rename;
- [x] destructive tombstone cleanup remains strict and recovery-visible;
- [x] complete, partial and conflicting cleanup-recovery states remain represented in the source matrix;
- [x] recovery fails closed when live artifacts conflict with tombstones.

Remaining verification work:

- [ ] execute the rollback, post-commit cleanup, recovery, JSONL conformance and retention matrices against the same commit.

## Regression commands

```bash
cargo test -p athanor-app --test publication_semantics_inventory --locked
cargo test -p athanor-app --test publication_recovery_matrix --locked
cargo test -p athanor-projector-support --test publication_failure_semantics --locked
cargo test -p athanor-projector-support --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-store-jsonl --test module_boundaries --locked
cargo test -p athanor-store-jsonl --test refactored_store --locked
cargo test -p athanor-store-jsonl --locked
```

The inventory is not considered verified until these commands, workspace tests, formatting, and Clippy have executed against the same commit.
