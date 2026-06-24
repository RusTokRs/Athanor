# `athanor-search-tantivy`

This is the Tantivy-based implementation of `SearchIndex` for Athanor.

It provides fast BM25 lexical query support for Athanor entities, storing:
- `id` (exact, stored)
- `title` (tokenized, stored, indexed)
- `body` (tokenized, indexed, not stored)
- `payload` (JSON string, stored)

The index directory is a disposable read model under `.athanor/generated/current/search`.
Agent-facing access goes through `ath search <query>`, which returns bounded
`athanor.search.v1` reports with canonical entity ids, stable keys, source anchors, ownership
metadata, and truncation metadata.

Full snapshot rebuilds add all canonical entity documents and commit once before opening the
search reader. This avoids per-document segment reloads and Windows file-lock failures while
keeping incremental single-document updates available through the `SearchIndex` port.
Incremental writer instances disable background segment merging because loaded Tantivy readers
can retain memory-mapped segment files on Windows; the next full snapshot rebuild compacts the
disposable index.
