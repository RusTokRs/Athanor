# Indexing Pipeline

Status: implemented, reusable app-layer pipeline with app-layer adapter registry and JSONL read-model writer.

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
6. `RuntimeBuilder` builds the configured `IndexPipeline` from an `AdapterRegistry`.
7. `IndexPipeline` builds an affected subset from the extracted entities and facts for this run, then passes that subset to linkers and checkers alongside the full in-memory context.
8. `IndexPipeline` stores the canonical objects for the current run through `KnowledgeStore`.
9. `JsonlReadModelWriter` exports JSONL read models to `.athanor/generated/current/jsonl`.

## Pipeline Assembly

`athanor-app` now exposes:

- `IndexPipeline`: orchestration for source discovery, extraction, linking, checking, and store writes.
- `AdapterRegistry`: ordered factories for source, extractor, linker, and checker adapters.
- `RuntimeBuilder`: app-layer runtime assembly for a project root and registry.
- `JsonlReadModelWriter`: reusable JSONL export for generated read models.

The default built-in registry currently assembles:

```text
store:
  MemoryKnowledgeStore

sources:
  LocalFileSystemSource

extractors:
  FileExtractor
  MarkdownExtractor

linkers:
  MarkdownContainmentLinker

checkers:
  MarkdownStructureChecker
```

`ath index` is responsible for CLI-facing concerns:

- canonicalizing the project root
- creating the default runtime builder
- choosing the generated JSONL output path
- calling the read-model writer

`RuntimeBuilder` and `AdapterRegistry` are responsible for adapter assembly:

- keeping the built-in adapter list out of CLI code
- preserving adapter order
- allowing tests, daemon code, and future plugins to share the same assembly point

`IndexPipeline` is responsible for orchestration:

- discovering sources
- running extractors
- deriving the affected subset for downstream adapters
- running linkers over the affected subset with full extracted context available
- running checkers over the affected subset with full extracted context available
- storing entities/facts/relations/diagnostics
- committing the snapshot

`JsonlReadModelWriter` is responsible for generated read models:

- writing `entities.jsonl`, `facts.jsonl`, `relations.jsonl`, and `diagnostics.jsonl`
- writing `manifest.json`
- keeping JSONL and manifest behavior reusable outside CLI indexing

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

- Snapshot IDs are in-memory and restart from `snap_memory_00000001` on every CLI run.
- The registry is in-process only; external plugin discovery is not implemented yet.
- The current CLI still performs a full source discovery and extraction pass before building the affected subset.
- The affected subset is run-local; persisted incremental change detection against previous snapshots is not implemented yet.
- JSONL export is a reusable app-layer writer, not a full `Projector` port implementation yet.

## Next Good Step

Persist previous snapshot state and compute affected subsets from file/content changes instead of treating every extracted object in the current run as affected.
