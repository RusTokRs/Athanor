# JSONL Store Adapter

Crate: `athanor-store-jsonl`

Ports:

- `KnowledgeStore`
- `CanonicalSnapshotStore`

## Purpose

Persists canonical Athanor objects to local JSONL snapshot directories.

The adapter lets indexing load previous canonical snapshots without reading generated read-model output.

## Inputs

Writes canonical objects received through `KnowledgeStore`:

- entities
- facts
- relations
- diagnostics

Reads prior snapshots by snapshot id or by `latest.json`.

## Outputs

Stores snapshot files under the configured root:

```text
latest.json
snapshots/<snapshot-id>/
  entities.jsonl
  facts.jsonl
  relations.jsonl
  diagnostics.jsonl
  manifest.json
```

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Writes only inside the configured store directory.

## Limitations

- Snapshot ids are local sequential ids.
- Query methods are process-local and currently inspect snapshots known to the running store instance.
- The adapter is intended as a development/offline canonical store, not as a concurrent database.

## Tests

```bash
cargo test -p athanor-store-jsonl
```
