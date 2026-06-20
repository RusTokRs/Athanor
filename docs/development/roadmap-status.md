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
- `athanor-store-jsonl`
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

Recent observed output shape:

```text
indexed <N> files into snapshot snap_memory_00000001
affected files: <changed> changed, <unchanged> unchanged, <removed> removed
wrote JSONL to <project>/.athanor/generated/current/jsonl
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

Current CLI store:

```text
JsonlKnowledgeStore
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

### Affected-Subset Linker And Checker Inputs

Status: verified.

Implemented in:

- `crates/athanor-core/src/ports.rs`
- `crates/athanor-app/src/pipeline.rs`
- `crates/athanor-linker-markdown/src/lib.rs`
- `crates/athanor-checker-markdown/src/lib.rs`

Purpose:

- introduces `AffectedSubset` as a core input contract for downstream adapters
- passes affected entities, facts, and newly produced relations to linkers and checkers
- keeps full extracted context available for adapters that need neighboring objects
- updates Markdown linker/checker adapters to scope emitted relations and diagnostics to affected documentation paths/pages

Current CLI behavior passes only newly extracted changed-file objects as affected when previous canonical JSONL is available.

### Persisted File Change State

Status: verified.

Implemented in:

- `crates/athanor-app/src/index_state.rs`
- `crates/athanor-app/src/index.rs`
- `crates/athanor-app/src/read_model.rs`
- `apps/ath/src/main.rs`

Purpose:

- persists last-run file paths, content hashes, language hints, and snapshot id in `.athanor/state/index-state.json`
- computes changed, unchanged, and removed file sets by comparing current discovery output to the previous state
- includes affected file counts in the JSONL manifest and CLI output

### Incremental Extraction And Canonical Object Merge

Status: verified.

Implemented in:

- `crates/athanor-app/src/pipeline.rs`
- `crates/athanor-app/src/read_model.rs`
- `crates/athanor-app/src/index.rs`

Purpose:

- loads the previous canonical snapshot from `CanonicalSnapshotStore`
- extracts changed files only when a previous canonical snapshot is available
- carries unchanged entities, facts, relations, and diagnostics into the new snapshot
- rewrites carried fact, relation, diagnostic, and snapshot-bearing entity payloads to the new snapshot id
- drops old canonical objects owned by changed or removed paths before rebuilding affected downstream outputs
- falls back to full extraction when the previous canonical snapshot is missing

Current behavior: carried objects are selected by explicit ownership metadata, with source/evidence fallback for older canonical snapshots.

### JSONL Canonical Store Adapter

Status: verified.

Implemented in:

- `crates/athanor-core/src/ports.rs`
- `crates/athanor-store-jsonl/src/lib.rs`
- `crates/athanor-store-jsonl/README.md`
- `docs/adapters/store-jsonl.md`

Purpose:

- introduces `CanonicalSnapshot` and `CanonicalSnapshotStore` as core storage contracts
- persists canonical entities, facts, relations, and diagnostics to `.athanor/store/canonical/jsonl`
- writes one JSONL snapshot directory per committed snapshot
- writes `latest.json` for latest-snapshot discovery
- lets `ath index` load previous canonical objects from a durable store instead of generated read models

### Canonical Object Ownership Metadata

Status: verified.

Implemented in:

- `crates/athanor-domain/src/model.rs`
- `crates/athanor-extractor-basic/src/lib.rs`
- `crates/athanor-extractor-markdown/src/lib.rs`
- `crates/athanor-linker-markdown/src/lib.rs`
- `crates/athanor-checker-markdown/src/lib.rs`
- `crates/athanor-app/src/pipeline.rs`

Purpose:

- adds `Ownership` metadata to entities, facts, relations, and diagnostics
- marks extractor output as owned by its source file
- marks Markdown containment relations with the union of related entity owners
- marks Markdown diagnostics with the ownership of the diagnosed entity
- uses ownership metadata as the primary incremental merge pruning contract
- keeps source/evidence fallback for older canonical snapshots without ownership metadata

### Adapter Output Validation

Status: verified.

Implemented in:

- `crates/athanor-app/src/pipeline.rs`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- documents canonical output requirements for ownership-aware adapters
- validates newly emitted entities before storage
- validates newly emitted facts, relations, and diagnostics for evidence and ownership
- fails indexing with adapter-specific error messages when required metadata is missing
- leaves carried older snapshots compatible through merge fallback behavior

### Aggregated Adapter Validation Reporting

Status: verified.

Implemented in:

- `crates/athanor-app/src/pipeline.rs`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- introduces `AdapterValidationReport`
- introduces `AdapterValidationIssue`
- records adapter name, object type, object id, and missing metadata field
- aggregates multiple validation issues from one adapter output before returning an error
- keeps the external failure path simple while making the validation result structured inside the app layer

### Adapter Validation Artifact

Status: verified.

Implemented in:

- `apps/ath/src/main.rs`
- `crates/athanor-app/src/index.rs`
- `crates/athanor-app/src/pipeline.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath index --validation-report <path>`
- writes machine-readable adapter validation reports as JSON when indexing fails validation
- defaults validation report output to `.athanor/generated/current/validation-report.json`
- removes stale validation reports after a successful index run
- serializes adapter name, object type, object id, and missing metadata field for every validation issue

## In Progress

None.

## Next

Recommended next task:

```text
Add an explicit adapter validation mode that checks contracts without committing a snapshot.
```

Why:

- Adapter validation reports are now machine-readable when indexing fails.
- The current validation path still runs as part of indexing.
- Adapter/plugin authors would benefit from a command mode dedicated to contract checking without persisting new canonical snapshots.

## Verification Commands

Run before marking implementation work as verified:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
```
