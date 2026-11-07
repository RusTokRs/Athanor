---
id: doc://docs/adapters/store-surrealdb.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000272
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
- Uses the SurrealDB 4.0 SDK for offline database operations (`mem://` or `surrealkv://path`).
- With the opt-in `remote` feature, connects via WebSocket to a compatible SurrealDB 4 server. The Store Conformance CI runs against `surrealdb/surrealdb:v4.0.0`.
- On connect, defines the six required tables with `DEFINE TABLE IF NOT EXISTS` so transaction inserts cannot fail on a fresh v4 database. Existing schemas are preserved; remote credentials must allow schema initialization.
- Before switching an existing SurrealDB 2 persistent store to 4, back up its data and validate a supported migration; do not assume the on-disk format is directly compatible.
- Snapshot and batch records use typed database record IDs. Reads restore the original Athanor IDs before returning canonical domain objects, preserving the JSONL contract.

## Limitations

- Multi-tenant and remote cloud cluster connections are deferred.

## Tests

```bash
cargo test -p athanor-store-surrealdb
```
