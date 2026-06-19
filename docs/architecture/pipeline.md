# Indexing Pipeline

Status: implemented, reusable app-layer pipeline with app-layer adapter registry.

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
7. `IndexPipeline` stores the canonical objects for the current run through `KnowledgeStore`.
8. `athanor-app` exports JSONL read models to `.athanor/generated/current/jsonl`.

## Pipeline Assembly

`athanor-app` now exposes:

- `IndexPipeline`: orchestration for source discovery, extraction, linking, checking, and store writes.
- `AdapterRegistry`: ordered factories for source, extractor, linker, and checker adapters.
- `RuntimeBuilder`: app-layer runtime assembly for a project root and registry.

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
- writing JSONL files
- writing `manifest.json`

`RuntimeBuilder` and `AdapterRegistry` are responsible for adapter assembly:

- keeping the built-in adapter list out of CLI code
- preserving adapter order
- allowing tests, daemon code, and future plugins to share the same assembly point

`IndexPipeline` is responsible for orchestration:

- discovering sources
- running extractors
- running linkers
- running checkers
- storing entities/facts/relations/diagnostics
- committing the snapshot

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
- Linkers and checkers run over the full extracted set, not an affected subset.
- JSONL export is implemented in the app layer and should move to a projector/utility.

## Next Good Step

Move JSONL export behind a projector or shared utility so generated read models are no longer hand-written by the CLI-facing indexing service.
