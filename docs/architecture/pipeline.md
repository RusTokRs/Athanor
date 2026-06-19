# Indexing Pipeline

Status: implemented, initial vertical slice.

Athanor currently has a minimal but complete knowledge pipeline:

```text
SourceProvider
  -> Extractor
  -> Linker
  -> Checker
  -> KnowledgeStore
  -> JSONL read model
```

The CLI entry point is:

```bash
cargo run -p ath -- index .
```

## Current Flow

1. `athanor-source-fs` discovers project files and returns `SourceFile` values.
2. `athanor-extractor-basic` creates file entities and `file_discovered` facts.
3. `athanor-extractor-markdown` creates documentation page/section entities and `doc_section_found` facts.
4. `athanor-linker-markdown` creates `contains` relations between files, documentation pages, and sections.
5. `athanor-checker-markdown` creates documentation structure diagnostics.
6. `athanor-store-memory` stores the canonical objects for the current run.
7. `athanor-app` exports JSONL read models to `.athanor/generated/current/jsonl`.

## Generated Files

```text
.athanor/generated/current/jsonl/
  entities.jsonl
  facts.jsonl
  relations.jsonl
  diagnostics.jsonl
  manifest.json
```

Generated files are read models. They are not the source of truth and may be deleted and rebuilt.

## Current Limitations

- The pipeline is assembled directly in `athanor-app/src/index.rs`.
- Snapshot IDs are in-memory and restart from `snap_memory_00000001` on every CLI run.
- There is no runtime builder or configurable registry yet.
- Linkers and checkers run over the full extracted set, not an affected subset.
- JSONL export is implemented in the app layer and should move to a projector/utility.

## Next Good Step

Introduce an `IndexPipeline` or runtime builder that owns ordered lists of sources, extractors, linkers, checkers, stores, and projectors.
