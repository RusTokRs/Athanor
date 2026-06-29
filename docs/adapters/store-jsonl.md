---
id: doc://docs/adapters/store-jsonl.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
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
  path_index.json
  stable_key_index.json
  manifest.json
```

## Secondary Indexes
To optimize performance and scale for large repositories, Athanor builds and writes two secondary index files when a snapshot is committed:
- **`stable_key_index.json`**: Maps each canonical entity's `stable_key` to its internal `entity_id`. This allows rapid target resolutions during query commands without scanning the full entities file.
- **`path_index.json`**: Maps each source file path to the lists of all associated canonical object IDs (entities, facts, relations, diagnostics) declared in or owned by that file.

Additionally, the JSONL reader uses a streaming line-buffered approach (`BufReader::read_line` with a reusable buffer) to parse items line-by-line/chunk-by-chunk to eliminate memory and IO spikes on huge snapshots.

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
