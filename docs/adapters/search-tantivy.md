---
id: doc://docs/adapters/search-tantivy.md
kind: adapter
language: en
last_verified_snapshot: snap_jsonl_00000090
source_language: en
status: verified
---
# Tantivy Search Index Adapter

The Tantivy search index adapter implements the `SearchIndex` port to support fast BM25 lexical querying over Athanor's canonical entities.

Implements: `SearchIndex`

## Inspected Library
- **Library**: `tantivy = "0.26.1"`
- **Why**: BM25 scoring, highly optimized inverted index, and flexible tokenizers.

## Schema Configuration
The index defines four fields:
- **`id`** (`STRING | STORED`): Stores the canonical entity ID (exact matching, stored).
- **`title`** (`TEXT | STORED`): Stores the entity's name or title (indexed with versioned tokenizer, stored).
- **`body`** (`TEXT`): Concatenates the name, stable key, title, aliases, description, and summary (indexed, not stored).
- **`payload`** (`STORED`): Stores the serialized JSON representation of the `Entity` (stored, not indexed).

## Tokenizer & Versioning
- **Tokenizer**: Custom text analyzer named `"athanor_en_v1"`.
- **Filters**: `LowerCaser` (case insensitivity) and `Stemmer` (English language word stemming).

## Index Lifecycle & Rebuilds
- **Location**: Stored in a directory at `.athanor/generated/current/search`.
- **Synchronization**: Emits an `index_meta.json` file storing the currently indexed `snapshot_id`.
- **Rebuilding**: When `context` or `search` operations run, the orchestrator compares the metadata's snapshot ID to the latest canonical snapshot. If it is mismatched or missing, the index directory is deleted and rebuilt from scratch to ensure complete consistency without store pollution.

## Configuration
Add `athanor-search-tantivy` to workspace dependencies. It is used as a coordinate read-model alongside JSONL, wiki, and HTML reports.

## Side Effects
Writes files to the local filesystem inside the project's `.athanor` folder.

## Test
```bash
cargo test -p athanor-search-tantivy
```
