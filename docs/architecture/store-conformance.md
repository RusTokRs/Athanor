---
id: doc://docs/architecture/store-conformance.md
kind: architecture
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Store Conformance and Write Coordination

Athanor treats canonical snapshots as a backend-neutral contract. Memory, JSONL, and SurrealDB
stores must agree on what readers can observe while a snapshot moves through begin, write, prepare,
commit, and abort.

## Shared contract

`athanor-store-conformance` provides reusable async checks for:

- exact and latest committed snapshot selection;
- stable-key, relation, and diagnostic query behavior;
- invisibility of uncommitted and prepared snapshots;
- complete `SnapshotBatch` publication after commit;
- rejection of abort after commit;
- removal of aborted snapshots without changing `LatestCommitted`.

The dedicated `Store Conformance` workflow runs the package tests for Memory, JSONL, and the real
embedded SurrealDB engine on every pull request and push to `main`.

## SurrealDB process-local write gate

SurrealDB 2.x executes separate SDK requests in separate backend transactions. Athanor therefore
wraps the existing adapter with one async write gate shared by cloned `SurrealKnowledgeStore`
handles. The gate serializes snapshot allocation, canonical writes, prepare, commit, and abort in one
Athanor process. Reads remain concurrent and continue to expose committed snapshots only.

This closes the in-process lost-update race in the read-modify-write snapshot counter and gives the
application one coordinated writer boundary. It does not claim cross-process exclusion or an atomic
multi-statement `put_snapshot` transaction. Those remain required before P0.4 can be completed.

## Remaining transactional work

- replace the process-local gate with a backend transaction or lease that coordinates independent
  processes;
- execute the whole SurrealDB `SnapshotBatch` in one native transaction;
- add conflict and rollback injection tests for partial backend failures;
- introduce one generation identity and one final publication pointer spanning canonical data,
  state, and read models.
