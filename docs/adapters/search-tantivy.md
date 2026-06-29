---
id: doc://docs/adapters/search-tantivy.md
kind: adapter
language: en
last_verified_snapshot: snap_jsonl_00000261
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

## Agent-Facing Query Contract

Agents should not inspect the Tantivy directory or generated JSONL directly. The app-layer
`ath search <query>` command is the bounded access path. Its JSON output uses
`athanor.search.v1` and includes:

- the original query, requested limit, returned count, and truncation status
- an omitted-result lower bound when the limit hides additional matches
- canonical entity ids and stable keys
- source anchors and ownership metadata for each result
- BM25 score, serialized entity kind, name, and optional title

Full snapshot rebuilds add all canonical entity documents in one batch and commit before opening
the reader. This avoids per-document segment reloads retaining obsolete memory-mapped files on
Windows.

Incremental writer instances disable background segment merging. This prevents Windows readers
from blocking deletion of memory-mapped segment files; later full snapshot rebuilds compact the
disposable index.

## Configuration
Add `athanor-search-tantivy` to workspace dependencies. It is used as a coordinate read-model alongside JSONL, wiki, and HTML reports.

## Side Effects
Writes files to the local filesystem inside the project's `.athanor` folder.

## Test
```bash
cargo test -p athanor-search-tantivy
```
