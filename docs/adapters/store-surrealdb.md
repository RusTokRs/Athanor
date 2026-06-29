---
id: doc://docs/adapters/store-surrealdb.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000251
source_language: en
status: verified
---
# SurrealDB Store Adapter

Crate: `athanor-store-surrealdb`

Ports:

- `KnowledgeStore`
- `CanonicalSnapshotStore`

## Purpose

Persists canonical Athanor objects to a SurrealDB database using embedded engines (in-memory or SurrealKV file persistence).

## Inputs

Writes canonical objects received through `KnowledgeStore`:

- entities
- facts
- relations
- diagnostics

Reads snapshot details through `CanonicalSnapshotStore` using SurrealQL.

## Database Tables

SurrealDB maps Athanor entities to structured document tables:

- **`entity`**: Contains all indexed entities, queried by `stable_key` or `kind`.
- **`fact`**: Stores facts associated with entities.
- **`relation`**: Holds relations between entities.
- **`diagnostic`**: Records diagnosed issues.
- **`snapshot`**: Tracks committed snapshot records and status.
- **`meta`**: Stores a persistent counter under `snapshot_counter` to generate sequential snapshot IDs.

## Commands And Network

- Does not run external commands.
- Supports offline database operations. Connects using `mem://` or `surrealkv://path`.

## Limitations

- Multi-tenant and remote cloud cluster connections are deferred.

## Tests

```bash
cargo test -p athanor-store-surrealdb
```
