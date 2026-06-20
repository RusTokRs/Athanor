# JSONL Store Adapter

Crate: `athanor-store-jsonl`

Ports:

- `KnowledgeStore`
- `CanonicalSnapshotStore`

## Purpose

Provides a durable local canonical object store backed by JSONL snapshot directories.

It is intended for development and offline indexing workflows where Athanor needs to carry canonical entities, facts, relations, and diagnostics across CLI runs without using generated read models as the previous canonical source.

## Storage Layout

The default CLI stores canonical snapshots under:

```text
.athanor/store/canonical/jsonl/
  latest.json
  snapshots/
    <snapshot-id>/
      entities.jsonl
      facts.jsonl
      relations.jsonl
      diagnostics.jsonl
      manifest.json
```

## Inputs

Receives canonical objects through the `KnowledgeStore` write methods.

Reads previous snapshots through `CanonicalSnapshotStore`.

## Outputs

Persists:

- `Entity`
- `Fact`
- `Relation`
- `Diagnostic`

The adapter does not create new domain knowledge itself.

## Commands And Network

- Does not run external commands.
- Does not use the network.
- Writes only inside the configured store directory.

## Limitations

- Snapshot ids are local sequential ids derived from existing snapshot directories.
- Query methods read from snapshots known to the current process.
- This is a JSONL development store, not a concurrent multi-process database.

## Tests

```bash
cargo test -p athanor-store-jsonl
```
