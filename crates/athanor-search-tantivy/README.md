# `athanor-search-tantivy`

This is the Tantivy-based implementation of `SearchIndex` for Athanor.

It provides fast BM25 lexical query support for Athanor entities, storing:
- `id` (exact, stored)
- `title` (tokenized, stored, indexed)
- `body` (tokenized, indexed, not stored)
- `payload` (JSON string, stored)
