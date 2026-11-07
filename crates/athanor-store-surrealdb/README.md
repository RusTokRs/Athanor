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
- Uses the SurrealDB 4.0 SDK for local memory (`mem://`) and persistent local file storage (`surrealkv://path`).
- The optional `remote` feature supports WebSocket connections to a compatible SurrealDB 4 server; CI tests two independent connections.
- Connect initializes the six required tables with idempotent `DEFINE TABLE IF NOT EXISTS` statements before transactional writes; the connecting account needs database schema permissions.
- Does not require network access for embedded memory/file modes.
- Before upgrading an existing persistent store from SurrealDB 2, back up its data and validate migration with the server's documented procedures; do not assume direct file-format compatibility across major versions.

## Limitations

- Currently relies on `surrealdb` engine-any connect interfaces.

## Tests

SurrealDB 4's embedded engine can overflow the default Rust test-thread stack. CI sets a 16 MiB
minimum; use the same setting for local embedded tests (PowerShell: set `$env:RUST_MIN_STACK`
first). Applications embedding this crate must give their runtime's initial and worker threads
sufficient stack; `ath` and `athd` configure 16 MiB when built with `store-surreal`.

```bash
RUST_MIN_STACK=16777216 cargo test -p athanor-store-surrealdb --locked
```
