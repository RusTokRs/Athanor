# Roadmap Status

This file tracks what has actually been implemented. It is intentionally separate from `start.md`, which is the full architectural plan.

Agent entrypoint: read `AGENTS.md`, `docs/README.md`, and `docs/development/agent-workflow.md` before implementation work.

## Implemented

### Workspace Skeleton

Status: verified.

Crates:

- `athanor-domain`
- `athanor-core`
- `athanor-app`
- `athanor-source-fs`
- `athanor-store-memory`
- `athanor-extractor-basic`
- `athanor-extractor-markdown`
- `athanor-linker-markdown`
- `athanor-checker-markdown`
- `apps/ath`

### CLI

Status: verified.

Implemented commands:

```bash
ath
ath --version
ath init
ath index
```

### Indexing Vertical Slice

Status: verified.

Implemented flow:

```text
local files
  -> file and Markdown extraction
  -> Markdown containment links
  -> Markdown structure diagnostics
  -> memory store
  -> JSONL export
```

Current runtime check:

```bash
cargo run -p ath --quiet -- index .
```

Recent observed output:

```text
files_indexed: 48
entities: 296
facts: 280
relations: 248
diagnostics: 0
```

### IndexPipeline

Status: verified.

Implemented in:

- `crates/athanor-app/src/pipeline.rs`

Purpose:

- owns ordered source/extractor/linker/checker execution
- writes canonical objects through `KnowledgeStore`
- lets `ath index` stay focused on CLI paths and JSONL export

The orchestration is reusable and no longer owns CLI-facing output concerns.

### AdapterRegistry And RuntimeBuilder

Status: verified.

Implemented in:

- `crates/athanor-app/src/runtime.rs`

Purpose:

- owns default built-in adapter assembly
- keeps adapter ordering out of CLI code
- lets CLI, daemon code, tests, and future plugins share the same app-layer assembly point

Current built-in registry:

```text
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

### JSONL Read Model Writer

Status: verified.

Implemented in:

- `crates/athanor-app/src/read_model.rs`

Purpose:

- owns JSONL read-model file writing
- owns `manifest.json` generation
- keeps generated output behavior reusable outside CLI indexing
- lets `ath index` stay focused on root normalization, runtime construction, and reporting

## In Progress

None.

## Next

Recommended next task:

```text
Introduce affected-subset execution for linkers and checkers.
```

Why:

- `IndexPipeline` owns orchestration.
- `AdapterRegistry` and `RuntimeBuilder` own adapter assembly.
- `JsonlReadModelWriter` owns generated read-model export.
- Linkers and checkers still run over the full extracted set.
- Incremental indexing will need a way to pass only affected entities, facts, and relations through downstream adapters.

## Verification Commands

Run before marking implementation work as verified:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
```
