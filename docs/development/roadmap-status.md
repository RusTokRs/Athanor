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
files_indexed: 47
entities: 294
facts: 278
relations: 247
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

## In Progress

None.

## Next

Recommended next task:

```text
Move JSONL export behind a projector or shared utility.
```

Why:

- `IndexPipeline` owns orchestration.
- `AdapterRegistry` and `RuntimeBuilder` own adapter assembly.
- JSONL export is still hand-written in the CLI-facing indexing service.
- Future generated read models should share a projector or utility instead of duplicating file-writing logic.

## Verification Commands

Run before marking implementation work as verified:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
```
