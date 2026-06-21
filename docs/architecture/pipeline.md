# Indexing Pipeline

Status: implemented, reusable app-layer pipeline with app-layer adapter registry, incremental merge, and JSONL read-model writer.

Athanor currently has a minimal but complete knowledge pipeline:

```text
SourceProvider
  -> Extractor
  -> Linker
  -> Checker
  -> JSONL KnowledgeStore
  -> JSONL read model
```

The CLI entry point is:

```bash
cargo run -p ath -- index .
cargo run -p ath -- index . --validate-only
```

## Current Flow

1. `athanor-source-fs` discovers project files and returns `SourceFile` values.
2. `athanor-extractor-basic` creates file entities and `file_discovered` facts.
3. `athanor-extractor-markdown` parses CommonMark/GFM heading events with `pulldown-cmark`, then creates documentation page/section entities and `doc_section_found` facts.
4. `athanor-extractor-openapi` dispatches OpenAPI 3.1 to `oas3` and 3.0 to a maintained-YAML legacy parser, then extracts operations, component schemas, and normalized request/response schema uses.
5. `athanor-extractor-rust` parses Rust files into module, function, and symbol entities plus `symbol_defined` facts.
6. `athanor-linker-markdown` creates `contains` relations between files, documentation pages, and sections.
7. `athanor-linker-api` links OpenAPI operations to matching Rust handlers, Markdown API documentation, and same-document request/response component schemas.
8. `athanor-checker-markdown` creates documentation structure diagnostics.
9. `athanor-checker-api` diagnoses OpenAPI operations without linked implementations or documentation and local component schema references that did not resolve.
10. `RuntimeBuilder` discovers adapter plugin manifests from `.athanor/adapters/*.json` and `.athanor/plugins/*/athanor-adapter.json`, then applies enabled adapter entries that match known app-layer factory ids.
11. `RuntimeBuilder` builds the configured `IndexPipeline` from an `AdapterRegistry`.
12. `IndexStateStore` classifies discovered files as changed, unchanged, or removed by comparing them with the previous state.
13. File additions or removals trigger a safe full rebuild so absence diagnostics cannot remain stale.
14. `IndexPipeline` extracts changed files only when a previous canonical snapshot is available from `CanonicalSnapshotStore`.
15. `IndexPipeline` carries unchanged canonical objects forward from the previous canonical snapshot, rewrites carried snapshot ids to the new snapshot, and drops objects whose ownership includes changed or removed paths.
16. `IndexPipeline` builds an affected subset from newly extracted objects, then passes it to linkers and checkers alongside the merged full context.
17. `IndexPipeline` validates newly emitted canonical objects for required evidence and ownership metadata.
18. If validation fails, `ath index` writes the aggregated adapter validation report to the configured validation report path.
19. In `--validate-only` mode, the CLI writes a structured validation result artifact for successful runs, then stops without persisting a canonical snapshot, read model, or index state.
20. Otherwise, `IndexPipeline` stores the merged canonical objects for the current run through `KnowledgeStore`.
21. `JsonlReadModelWriter` exports JSONL read models to `.athanor/generated/current/jsonl`.
22. `IndexStateStore` persists file hash state to `.athanor/state/index-state.json` for the next run.

## Pipeline Assembly

`athanor-app` now exposes:

- `IndexPipeline`: orchestration for source discovery, extraction, linking, checking, and store writes.
- `AdapterRegistry`: ordered factories for source, extractor, linker, and checker adapters.
- `RuntimeBuilder`: app-layer runtime assembly for a project root, registry, and discovered adapter plugin manifests.
- `JsonlReadModelWriter`: reusable JSONL export for generated read models.
- `JsonlKnowledgeStore`: durable local canonical snapshot store used by the CLI.
- `context_project`: task-focused context-pack generation from the latest canonical snapshot.
- `explain_project`: exact stable-key entity explanation from the latest canonical snapshot.
- `check_project`: scoped API/documentation diagnostic reporting from the latest canonical snapshot.

## Context Pack Generation

`ath context <task>` reads the latest durable canonical snapshot without running indexing again. The initial context generator:

- tokenizes the task deterministically
- ranks canonical entities by matches in names, titles, stable keys, aliases, and source paths
- applies `summary`, `normal`, `deep`, or `full` presets for context size and relation depth
- accepts explicit token, file, entity, diagnostic, and relation-depth overrides
- expands direct matches by the configured number of relation hops
- includes diagnostics attached to selected entities
- returns stable file and entity scopes
- materializes selected entities, internal relations, and diagnostics in the JSON payload
- reports effective limits, approximate serialized token usage, omitted object counts, and whether relevance or limits caused omission in the JSON payload

The token budget is a deterministic estimate based on serialized canonical payload bytes divided by four; it is a size guard, not tokenizer-specific accounting. This remains an app-layer lexical slice rather than a `SearchIndex` implementation. Tantivy, vectors, and semantic ranking remain future adapters or services.

## Entity Explanation

`ath explain <stable-key>` reads one exact canonical entity from the latest durable snapshot without
re-indexing. The app-layer explanation includes:

- the full canonical entity and its source metadata
- facts where the entity is either subject or object
- outgoing and incoming relations, each resolved to the neighboring entity when available
- diagnostics attached to the entity
- the snapshot id and `athanor.entity_explanation.v1` response schema

The default CLI output is a compact directional summary. `--json` returns the complete explanation,
including canonical evidence, confidence, status, ownership, and payload fields. Stable-key lookup is
exact and currently explains one canonical entity at a time.

## Diagnostic Check Views

`ath check api` and `ath check docs` read open diagnostics from the latest durable canonical snapshot
without re-indexing. The app layer classifies diagnostic kinds into API and documentation scopes,
sorts results by severity and diagnostic id, and returns:

- snapshot id and requested scope
- total, critical, high, medium, and low counts
- complete canonical diagnostic objects

The default CLI output is a compact source-oriented list. `--json` emits the
`athanor.diagnostic_check.v1` report. These commands are currently read-only views and return success
after a valid query even when diagnostics exist; CI failure thresholds and strict-mode policy remain
deferred.

The default built-in registry currently assembles:

```text
store:
  JsonlKnowledgeStore

sources:
  LocalFileSystemSource

extractors:
  FileExtractor
  MarkdownExtractor
  OpenApiExtractor
  RustExtractor

linkers:
  MarkdownContainmentLinker
  ApiKnowledgeLinker

checkers:
  MarkdownStructureChecker
  ApiConsistencyChecker
```

`ath index` is responsible for CLI-facing concerns:

- canonicalizing the project root
- creating the default runtime builder
- choosing the generated JSONL output path
- loading and saving persisted index state
- reporting changed, unchanged, and removed file counts
- calling the read-model writer
- loading the previous canonical snapshot from the durable store
- writing adapter validation reports to `.athanor/generated/current/validation-report.json` or the `--validation-report` path when validation fails
- writing successful validation-only result JSON to `.athanor/generated/current/validation-result.json` or the `--validation-result` path
- supporting `--validate-only` for adapter contract validation without writing snapshots, state, or read models

`RuntimeBuilder` and `AdapterRegistry` are responsible for adapter assembly:

- keeping the built-in adapter list out of CLI code
- discovering adapter plugin manifests from `.athanor/adapters/*.json` and `.athanor/plugins/*/athanor-adapter.json`
- applying enabled manifest entries that map to known app-layer adapter factory ids
- loading external process sources, extractors, linkers, and checkers from manifest `command` entries
- preserving adapter order
- allowing tests, daemon code, and future plugins to share the same assembly point

`IndexPipeline` is responsible for orchestration:

- discovering sources
- classifying affected files from persisted state
- running extractors for changed files when a previous canonical snapshot is available
- falling back to full extraction when the previous canonical snapshot is missing
- merging unchanged canonical objects from the previous canonical snapshot
- pruning carried canonical objects by explicit ownership metadata, with source/evidence fallback for older snapshots
- deriving the affected subset from newly extracted objects for downstream adapters
- running linkers over the affected subset with full merged context available
- running checkers over the affected subset with full merged context available
- validating newly emitted entities/facts/relations/diagnostics before storage
- aggregating adapter validation failures by adapter, object type, object id, and missing metadata field
- stopping before durable writes when the CLI requested validation-only mode
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

.athanor/generated/current/
  validation-report.json
  validation-result.json

.athanor/adapters/
  <adapter-plugin>.json

.athanor/plugins/
  <plugin-name>/athanor-adapter.json

.athanor/state/
  index-state.json

.athanor/store/canonical/jsonl/
  latest.json
  snapshots/<snapshot-id>/
```

Generated JSONL files under `.athanor/generated/current/jsonl` are read models. They are not the source of truth and may be deleted and rebuilt. `validation-report.json` is written only for adapter contract validation failures and is removed after a successful index run. `validation-result.json` is written only for successful `--validate-only` runs and is removed after validation failures or normal index runs. Durable canonical snapshots live under `.athanor/store/canonical/jsonl`. The state file records the last indexed file paths, content hashes, language hints, and snapshot id so later runs can classify changed, unchanged, and removed files. Its schema is versioned so changes to built-in extraction, linking, or checking semantics can force a safe one-time full rebuild; the OpenAPI parser-adapter migration advances it to `athanor.index_state.v8`.

## Current Limitations

- Process source adapters perform a full discovery request per indexing run; streaming discovery and source-level change feeds are not implemented.
- Context generation uses deterministic lexical matching and approximate token accounting; model-specific tokenizers and semantic search are not implemented.
- Entity explanation requires an exact stable key and does not yet provide fuzzy lookup, history, or cross-snapshot comparison.
- Diagnostic check views expose open findings but do not yet implement CI exit thresholds, strict mode, suppression configuration, or historical comparison.
- Rust extraction does not expand macros, emit trait method declarations, or infer imports, calls, and framework routes.
- OpenAPI extraction supports 3.0.x and 3.1.x through replaceable parser backends but does not support Swagger 2.x/OpenAPI 3.2, resolve external references, merge specifications, or infer code handlers.
- API knowledge linking is lexical for code/docs and resolves only same-document component schemas; framework route metadata, call graphs, and Rust schema/type links are not implemented.
- API consistency diagnostics check unresolved local component schema references but do not compare schema fields with Rust types or check status codes, auth, or permissions yet.
- The current CLI still performs a full source discovery pass before classifying changed files.
- The JSONL canonical store is a local development store, not a concurrent multi-process database.
- Older canonical snapshots without ownership metadata are pruned by entity source paths and evidence source files.
- JSONL export is a reusable app-layer writer, not a full `Projector` port implementation yet.

## Next Good Step

Start the next roadmap vertical slice from `start.md`; all current indexing ports can now be supplied by external process adapters without Rust ABI coupling.
