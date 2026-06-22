# SurrealDB Store Adapter

Crate: `athanor-store-surrealdb`

Ports:

- `KnowledgeStore`
- `CanonicalSnapshotStore`

## Purpose

Provides a standalone database store adapter for Athanor, backed by SurrealDB.

It allows persisting, querying, and checking Athanor objects (entities, facts, relations, diagnostics) using SurrealDB's embedded file database (SurrealKV) or in-memory database.

## Inputs

Receives Athanor domain objects through `KnowledgeStore` and `CanonicalSnapshotStore`.

## Outputs

Persists:

- `Entity` to table `entity`
- `Fact` to table `fact`
- `Relation` to table `relation`
- `Diagnostic` to table `diagnostic`
- `Snapshot` to table `snapshot`

## Commands And Network

- Does not run external commands.
- Supports both local memory (`mem://`) and persistent local file storage (`surrealkv://path`).
- Does not require network access for embedded memory/file modes.

## Limitations

- Currently relies on `surrealdb` engine-any connect interfaces.

## Tests

```bash
cargo test -p athanor-store-surrealdb
```
