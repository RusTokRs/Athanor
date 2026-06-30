---
id: doc://docs/development/roadmap-status.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
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
- `athanor-extractor-openapi`
- `athanor-extractor-operations`
- `athanor-extractor-rust`
- `athanor-adapter-rustok-fba`
- `athanor-adapter-rustok-ffa`
- `athanor-adapter-rustok-page-builder`
- `athanor-linker-api`
- `athanor-linker-js-ts`
- `athanor-linker-markdown`
- `athanor-checker-markdown`
- `athanor-checker-api`
- `athanor-projector-support`
- `athanor-projector-wiki`
- `athanor-projector-html`
- `apps/ath`

### CLI

Status: verified.

Implemented commands:

```bash
ath
ath --version
ath init
ath index
ath index --json
ath index --validate-only
ath index --validate-only --validation-result <path>
ath bench --size small
ath bench --size medium --json
ath bench --size large --root <path> --keep-fixture --json
ath validate-changed --path <path>
ath validate-changed --path <path> --json
ath validate-changed --path <path> --file <file> --json
ath update --changed
ath update --changed --json
ath overview
ath overview --json
ath coverage
ath coverage --json
ath coverage --adapter <id>
ath coverage --file <path>
ath context <task>
ath context <task> --json
ath context --diff
ath context --diff --json
ath search <query>
ath search <query> --json
ath explain <stable-key>
ath explain <stable-key> --json
ath check affected
ath check affected --json
ath check api
ath check api --json
ath check docs
ath check docs --json
ath check env
ath check env --json
ath check scripts
ath check scripts --json
ath check deployment
ath check deployment --json
ath check runbooks
ath check runbooks --json
ath docs check
ath docs check --json
ath docs drift
ath docs drift --json
ath docs propose-fix
ath docs propose-fix --output <path>
ath docs apply-patch <patch-id-or-path>
ath docs operations check
ath docs operations check --json
ath api snapshot
ath api snapshot --json
ath api snapshot --cleanup
ath api snapshot --no-cleanup
ath api diff --from <snapshot> --to <snapshot>
ath api diff --json
ath api diff --cleanup
ath api diff --no-cleanup
ath api breaking-changes --from <snapshot> --to <snapshot>
ath api breaking-changes --json
ath api cleanup
ath api cleanup --dry-run
ath api cleanup --keep-snapshots <N> --keep-diffs <N>
ath api cleanup --json
ath check api --strict
ath check api --strict --json
ath wiki
ath wiki --output <directory>
ath report html
ath report html --output <directory>
ath generate
ath graph export --format json
ath graph export --format graphml
ath graph export --format json --max-entities <N> --max-relations <N>
ath graph related <stable-key>
ath graph related <stable-key> --depth <N> --json
ath graph path <from-stable-key> <to-stable-key>
ath graph path <from-stable-key> <to-stable-key> --max-depth <N> --json
ath graph hubs
ath graph hubs --kind <entity-kind> --limit <N> --json
ath graph pagerank
ath graph pagerank --kind <entity-kind> --limit <N> --json
ath graph cycles
ath graph cycles --max-depth <N> --limit <N> --json
ath rustok ffa audit
ath rustok ffa audit --json
ath graph ffa surface <module> <surface>
ath graph ffa surface <module> <surface> --json
ath graph ffa violations --module <module> --surface <surface>
ath graph ffa violations --module <module> --surface <surface> --json
ath check rustok-ffa
ath check rustok-ffa --json
ath rustok fba audit
ath rustok fba audit --json
ath rustok page-builder audit
ath rustok page-builder audit --json
ath graph fba module <module>
ath graph fba module <module> --json
ath graph fba port <module> <port>
ath graph fba port <module> <port> --json
ath graph fba dependencies --module <module>
ath graph fba dependencies --module <module> --json
ath graph fba violations --module <module>
ath graph fba violations --module <module> --json
ath graph page-builder provider
ath graph page-builder provider --json
ath graph page-builder consumer <module>
ath graph page-builder consumer <module> --json
ath graph page-builder violations --module <module>
ath graph page-builder violations --module <module> --json
ath check rustok-fba
ath check rustok-fba --json
ath check rustok-page-builder
ath check rustok-page-builder --json
ath projects list
ath projects list --json
ath projects add <project-id> <path>
ath projects remove <project-id>
ath projects resolve <project-id>
ath projects resolve <project-id> --json
athd serve <project-id>
athd serve <project-id> --max-concurrent-requests <N>
athd serve <project-id> --max-job-history <N>
athd serve <project-id> --max-request-bytes <N> --max-response-bytes <N>
athd serve <project-id> --transport local-socket
athd serve <project-id> --watch --debounce-ms <N>
athd serve <project-id> --watch --watch-poll --debounce-ms <N>
athd start <project-id>
athd start <project-id> --transport local-socket --watch
athd status <project-id>
athd status <project-id> --json
athd ping <project-id>
athd ping <project-id> --json
athd jobs <project-id>
athd jobs <project-id> --json
athd job <project-id> <job-id>
athd job <project-id> <job-id> --json
athd cancel <project-id> <job-id>
athd cancel <project-id> <job-id> --json
athd index <project-id>
athd index <project-id> --json
athd generate <project-id>
athd generate <project-id> --json
athd wiki <project-id>
athd wiki <project-id> --json
athd report-html <project-id>
athd report-html <project-id> --json
athd overview <project-id>
athd overview <project-id> --json
athd explain <project-id> <stable-key>
athd explain <project-id> <stable-key> --json
athd search <project-id> <query>
athd search <project-id> <query> --json
athd context <project-id> <task>
athd context <project-id> --diff
athd context <project-id> <task> --json
athd stop <project-id>
ath repair inspect
ath repair inspect --json
ath repair cleanup
ath repair cleanup --dry-run
ath repair cleanup --generated-only --dry-run
ath repair cleanup --dry-run --keep-canonical <N> --keep-generated <N>
ath repair cleanup --json
ath repair regenerate
ath repair regenerate --dry-run
ath repair regenerate --json
ath repair recover-canonical
ath repair recover-canonical --dry-run
ath repair recover-canonical --json
ath repair apply
ath repair apply --dry-run
ath repair apply --generated-only --dry-run
ath repair apply --dry-run --keep-canonical <N> --keep-generated <N>
ath repair apply --json
```

### Indexing Vertical Slice

Status: verified.

Implemented flow:

```text
local files
  -> file, Markdown, OpenAPI, and Rust extraction
  -> Markdown containment and cross-source API links
  -> Markdown structure diagnostics
  -> JSONL canonical store
  -> JSONL read-model export
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
  OpenApiExtractor
  OperationsExtractor
  JsTsExtractor
  RustExtractor

linkers:
  MarkdownContainmentLinker
  ApiKnowledgeLinker
  JsTsImportLinker
  RustLinker

checkers:
  MarkdownStructureChecker
  ApiConsistencyChecker
  EnvDocsChecker
  ScriptDocsChecker
  DeploymentDocsChecker
  RunbookConsistencyChecker
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
- reuses the previous canonical snapshot id when source discovery finds no changed or removed files
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

### Adapter Validation-Only Mode

Status: verified.

Implemented in:

- `apps/ath/src/main.rs`
- `crates/athanor-app/src/index.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath index --validate-only`
- runs source discovery, extraction, linking, checking, and adapter contract validation
- reuses previous canonical snapshot context when available
- uses a transient memory store for the validation run
- does not write durable canonical snapshots, generated read models, or index state
- still writes machine-readable validation reports when adapter validation fails

### Successful Validation-Only Result Artifact

Status: verified.

Implemented in:

- `apps/ath/src/main.rs`
- `crates/athanor-app/src/index.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath index --validate-only --validation-result <path>`
- writes machine-readable success JSON for successful validation-only runs
- defaults validation result output to `.athanor/generated/current/validation-result.json`
- serializes schema, status, snapshot, affected-file counts, and canonical object counts
- removes stale validation result artifacts after validation failures or normal index runs

### Adapter Plugin Manifest Discovery

Status: verified.

Implemented in:

- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-app/src/index.rs`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- introduces the `athanor.adapter_manifest` manifest schema
- discovers manifests from `.athanor/adapters/*.json` and `.athanor/plugins/*/athanor-adapter.json`
- applies enabled manifest entries through the app-layer `AdapterRegistry`
- supports known built-in adapter factory ids as the first registry-backed loading path
- fails fast for unknown adapter ids or invalid manifest schemas
- supports external process extractors, linkers, and checkers through manifest `command` entries
- keeps source process adapters explicitly deferred
- keeps production adapter installation in `athanor-runtime-defaults` while `athanor-app` package
  tests install a focused fixture-only composition; direct package tests no longer depend on
  workspace feature unification or CLI startup side effects

### RusTok FFA Adapter And Graph Extension

Status: verified.

Implemented in:

- `crates/athanor-adapter-rustok-ffa`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-app/src/graph.rs`
- `crates/athanor-app/src/check.rs`
- `apps/ath/src/main.rs`

Purpose:

- adds opt-in built-in factories `builtin.extractor.rustok_ffa`, `builtin.linker.rustok_ffa`, and `builtin.checker.rustok_ffa`
- extracts FFA source markers and documentation status without mixing with FBA
- emits canonical FFA surface and layer entities with evidence-backed relations
- diagnoses FFA-only issues under the `rustok_ffa_*` prefix
- exposes bounded agent-facing read models through `ath rustok ffa audit`, `ath graph ffa surface`, `ath graph ffa violations`, and `ath check rustok-ffa`
- makes FFA audit scope explicit through observed, actionable, scaffold, and host-wiring counts
- reports FFA core/transport/UI structural completion through explicit numerators, denominators,
  missing-layer counts, and integer percentages while leaving non-actionable rows unscored

### RusTok FBA Adapter And Graph Extension

Status: verified.

Implemented in:

- `crates/athanor-adapter-rustok-fba`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-app/src/graph.rs`
- `crates/athanor-app/src/check.rs`
- `apps/ath/src/main.rs`

Purpose:

- adds opt-in built-in factories `builtin.extractor.rustok_fba`, `builtin.linker.rustok_fba`, and `builtin.checker.rustok_fba`
- extracts FBA registry JSON and Rust `src/ports.rs` code markers without mixing with FFA
- emits canonical `fba_module`, `fba_contract`, `fba_port`, `fba_operation`, `fba_profile`, and `fba_dependency` entities with evidence-backed relations
- diagnoses FBA-only issues under the `rustok_fba_*` prefix
- exposes bounded agent-facing read models through `ath rustok fba audit`, `ath graph fba module`, `ath graph fba port`, `ath graph fba dependencies`, `ath graph fba violations`, and `ath check rustok-fba`
- makes FBA audit scope explicit through registry-backed, dependency-only, in-progress, and unknown-status module counts
- reports FBA contract completion through applicable evidence-derived requirements while keeping
  dependency-only rows unscored and migration status independent

### RusTok Page Builder Adapter And Graph Extension

Status: verified.

Implemented in:

- `crates/athanor-adapter-rustok-page-builder`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-app/src/graph.rs`
- `crates/athanor-app/src/check.rs`
- `apps/ath/src/main.rs`

Purpose:

- adds opt-in built-in factories `builtin.extractor.rustok_page_builder`, `builtin.linker.rustok_page_builder`, and `builtin.checker.rustok_page_builder`
- extracts Page Builder provider registry, adapter seam, wave evidence, consumer manifest, content-format, and FSD surface markers without mixing with FFA or FBA
- emits canonical Page Builder provider, consumer, contract, capability, fallback-profile, wave-evidence, adapter-seam, content-surface, and FSD-surface entities with evidence-backed relations
- diagnoses Page Builder-only issues under the `rustok_page_builder_*` prefix
- exposes bounded agent-facing read models through `ath rustok page-builder audit`, `ath graph page-builder provider`, `ath graph page-builder consumer`, `ath graph page-builder violations`, and `ath check rustok-page-builder`

### External Process Extractors, Linkers, And Checkers

Status: verified.

Implemented in:

- `crates/athanor-core/src/ports.rs`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-app/Cargo.toml`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- lets manifest entries load extractor, linker, and checker adapters from external commands
- sends `ExtractInput`, `LinkInput`, or `CheckInput` JSON to the process stdin
- reads `ExtractOutput`, relation arrays, or diagnostic arrays from process stdout
- scopes process extractors with optional `supports_extensions`
- resolves relative command paths from the manifest directory
- keeps canonical output validation in the existing indexing pipeline

### External Process Sources

Status: verified.

Implemented in:

- `crates/athanor-app/src/runtime.rs`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- lets manifest entries load source adapters from external commands
- starts each source command once per indexing run
- sends an absolute project-root discovery request as JSON on stdin
- reads a JSON array of `SourceFile` values from stdout
- reuses the existing process lifecycle, stderr failure reporting, and manifest-relative command resolution
- completes process adapter coverage for the current source, extractor, linker, and checker ports

### Task-Focused Context Packs

Status: verified.

Implemented in:

- `crates/athanor-app/src/context.rs`
- `apps/ath/src/main.rs`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath context <task>` and `ath context <task> --json`
- loads the latest durable canonical snapshot without re-indexing
- selects direct lexical entity matches and expands them by one relation hop
- includes files and diagnostics associated with the selected entities
- emits the canonical `ContextPack` model with a self-contained JSON payload
- keeps search-backend and CLI presentation details out of domain/core

### Repository Overview Query

Status: verified.

Implemented in:

- `crates/athanor-app/src/overview.rs`
- `apps/ath/src/main.rs`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath overview [path]` and `ath overview [path] --json`
- loads the latest durable canonical snapshot without re-indexing
- reports canonical object totals, top entity kinds, top relation kinds, and top source roots
- summarizes API, documentation, and operations coverage counters
- ranks graph hubs by relation degree with stable keys and source anchors
- includes compact open diagnostic summaries for quick repository orientation
- emits the stable `athanor.overview.v1` JSON payload
- keeps the command as an app-layer read-only query over canonical snapshots, not a new store or graph source of truth

### Explicit Context Limits And Levels

Status: verified.

Implemented in:

- `crates/athanor-app/src/context.rs`
- `apps/ath/src/main.rs`
- `docs/architecture/pipeline.md`

Purpose:

- adds `summary`, `normal`, `deep`, and `full` context presets
- adds `--budget`, `--max-files`, `--max-entities`, `--max-diagnostics`, and `--max-depth` overrides
- bounds graph expansion and canonical payload material deterministically
- records effective limits, approximate token usage, and omitted object counts in the context payload

### Canonical Entity Explanation

Status: verified.

Implemented in:

- `crates/athanor-app/src/explain.rs`
- `apps/ath/src/main.rs`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath explain <stable-key>` and `ath explain <stable-key> --json`
- loads the latest durable canonical snapshot without re-indexing
- resolves an entity by exact stable key
- includes facts where the entity is subject or object
- separates incoming and outgoing relations and resolves their neighboring entities
- includes diagnostics attached to the entity
- exposes full evidence, ownership, confidence, status, and payload data in JSON mode
- keeps query orchestration in the app layer without changing domain/core contracts

### Scoped Diagnostic Check Views

Status: verified.

Implemented in:

- `crates/athanor-app/src/check.rs`
- `apps/ath/src/main.rs`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath check api` and `ath check docs` with optional `--json` output
- reads the latest durable canonical snapshot without re-indexing
- classifies API and documentation diagnostic kinds in the app layer
- returns open diagnostics only, sorted by severity and diagnostic id
- reports total and per-severity counts
- preserves complete diagnostic evidence, ownership, entity ids, status, and payload in JSON mode
- keeps the initial commands read-only with no CI failure threshold or strict-mode policy

### Editable Documentation Completeness Policy And Gate

Status: verified.

Implemented in:

- `crates/athanor-app/src/docs.rs`
- `apps/ath/src/main.rs`
- `crates/athanor-extractor-markdown/src/frontmatter.rs`
- `crates/athanor-extractor-markdown/src/lib.rs`
- `docs/development/docs-completeness-policy.md`

Purpose:

- adds `ath docs check` and `ath docs check --json`
- reads project policy from `[docs.completeness]` in `athanor.toml`
- gates only editable Markdown pages under `docs.editable_path`
- verifies explicitly declared required frontmatter fields and allowed statuses
- optionally requires `last_verified_snapshot` to match the current canonical snapshot
- includes open documentation diagnostics at or above a configurable severity threshold
- returns a non-zero process status when the gate fails
- excludes generated documentation and never rewrites editable source files
- advances persisted index state to v11 so existing projects capture explicit frontmatter field metadata once

### Editable Documentation Drift Report

Status: verified.

Implemented in:

- `crates/athanor-app/src/docs.rs`
- `apps/ath/src/main.rs`
- `docs/development/docs-completeness-policy.md`

Purpose:

- adds `ath docs drift` and `ath docs drift --json`
- reads the latest durable canonical snapshot without re-indexing
- selects editable documentation under `docs.editable_path`
- distinguishes current pages from pages with missing or stale `last_verified_snapshot` metadata
- emits the stable `athanor.docs_drift.v1` JSON report
- remains informational and never modifies documentation or fails solely because drift exists

### Automated CI/CD Baseline

Status: implemented; local CI contract verified, first hosted matrix run pending.

Implemented in:

- `.github/workflows/ci.yml`
- `docs/development/ci.md`

Purpose:

- runs on pushes to `main`, pull requests, and manual dispatches
- tests Rust 1.95 on Linux, Windows, and macOS
- enforces formatting, workspace tests, Clippy warnings, and indexing smoke tests
- runs the editable-documentation completeness gate against the newly indexed snapshot
- uses the locked dependency graph and Rust build caching
- grants read-only repository permissions and does not persist checkout credentials
- cancels superseded runs for the same workflow ref while allowing every matrix OS to finish
- migrates current English documentation to the required editable frontmatter contract

### Workspace Metadata And Status Hygiene

Status: verified.

Implemented in:

- `Cargo.toml`
- all workspace package manifests
- `README.md`

Purpose:

- confirms the canonical `RusTokRs/Athanor` repository URL against GitHub and the local `origin`
- keeps repository metadata inherited from one workspace-level source
- adds shared author, description, homepage, and documentation metadata to all 18 packages
- marks internal workspace packages as non-publishable until an explicit release plan exists
- updates the root status description to cover incremental snapshots, queries, process adapters, projectors, documentation policy, and CI
- states the current local/offline boundary and the next engine features without implying they already exist

### Markdown Wiki Projector

Status: verified.

Implemented in:

- `crates/athanor-projector-wiki`
- `crates/athanor-app/src/wiki.rs`
- `apps/ath/src/main.rs`
- `docs/adapters/projector-wiki.md`

Purpose:

- adds `ath wiki [path]` with an optional `--output` directory
- loads the latest durable canonical snapshot without re-indexing
- implements the core `Projector` port in an adapter crate
- writes a neutral Markdown index plus entity and open-diagnostic pages
- includes YAML frontmatter, source locations, facts, relations, evidence, and attached diagnostics
- emits a versioned manifest with snapshot and canonical object counts
- builds a complete staging directory and replaces the previous wiki without exposing partial pages
- keeps generated wiki content disposable and fully regenerable from canonical JSONL storage

### HTML Report Projector

Status: verified.

Implemented in:

- `crates/athanor-projector-html`
- `crates/athanor-projector-support`
- `crates/athanor-app/src/report.rs`
- `apps/ath/src/main.rs`
- `docs/adapters/projector-html.md`

Purpose:

- adds `ath report html [path]` with an optional `--output` directory
- loads the latest durable canonical snapshot without re-indexing
- implements a second core `Projector` adapter with a versioned input contract
- writes a self-contained static report and versioned manifest
- shows snapshot metrics, open diagnostic details, and a deterministic canonical entity table
- HTML-escapes all dynamic canonical values and avoids external scripts, styles, and network resources
- extracts shared canonical projection and staged publication mechanics into `athanor-projector-support`
- keeps generated HTML disposable and fully regenerable from canonical JSONL storage

### Coordinated Immutable Generated Generations

Status: verified.

Implemented in:

- `crates/athanor-projector-support/src/lib.rs`
- `crates/athanor-app/src/generation.rs`
- `crates/athanor-app/src/read_model.rs`
- `apps/ath/src/main.rs`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath generate [path]`
- loads the latest canonical snapshot once for all read models
- projects JSONL, Markdown wiki, and HTML into one numbered generation directory
- writes a complete generation manifest before publication
- publishes immutable `.athanor/generated/generations/<generation>` directories
- updates portable `.athanor/generated/current.json` only after every output succeeds
- preserves the previous pointer when projection fails
- keeps direct `.athanor/generated/current/*` outputs as uncoordinated compatibility paths
- extends `JsonlReadModelWriter` to project a loaded canonical snapshot without duplicating JSONL writing logic
- adds `athanor.generation.v1` reports with per-phase `athanor.generation_metrics.v1` timings;
  normal generation prints the timings and `repair regenerate --json` exposes the bounded report
- indexes entity attachments once per wiki/HTML projection instead of scanning every canonical
  fact, relation, diagnostic, and entity lookup for every generated entity page
- builds coordinated wiki and HTML outputs in parallel after JSONL projection while still switching
  the current generation pointer only after both output trees finish successfully
- treats `ath generate` as idempotent when the current generated pointer already targets the latest
  canonical snapshot, returning an `up_to_date` report without rewriting generated JSONL, wiki, or
  HTML read models
- pre-creates high-volume wiki/HTML page directories before writing entity and diagnostic pages,
  avoiding repeated parent-directory creation checks inside per-page loops
- buffers canonical-store and generated-read-model JSONL serialization with explicit flushes,
  avoiding per-token filesystem writes while preserving the same line-delimited contract

### Library Adoption Plan

Status: verified.

Implemented in:

- `docs/development/library-adoption-plan.md`
- `docs/README.md`
- `start.md`

Purpose:

- records retained, approved, conditional, and deferred third-party dependencies
- selects `pulldown-cmark`, `oas3`, `jsonschema`, `notify`, and Tantivy for their relevant phases
- requires replacement of the unmaintained `serde_yaml` backend through a compatibility spike
- defines adapter boundaries and contract-test criteria for every adoption
- keeps canonical models, stable identity, evidence, ownership, and incremental merge Athanor-owned

### Pulldown-Cmark Markdown Extraction

Status: verified.

Implemented in:

- `crates/athanor-extractor-markdown`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-markdown.md`
- `docs/development/library-adoption-plan.md`

Purpose:

- replaces the line-based Markdown heading scanner with `pulldown-cmark` 0.13.4
- parses ATX and setext headings from CommonMark events
- ignores heading syntax inside fenced code blocks
- normalizes inline formatting into canonical heading titles
- maps parser byte offsets to deterministic source evidence lines
- preserves Athanor stable slug, ownership, fact, and entity contracts
- enables GFM extensions explicitly
- advances persisted index state to v7 so existing projects rebuild Markdown structure once

### Markdown Documentation Frontmatter

Status: verified.

Implemented in:

- `crates/athanor-extractor-markdown/src/frontmatter.rs`
- `crates/athanor-extractor-markdown/src/lib.rs`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-markdown.md`

Purpose:

- parses optional leading YAML frontmatter through adapter-private `serde_yaml_ng`
- supports explicit `doc://` page identity that remains stable across source path moves
- applies explicit language to documentation pages and sections
- classifies documentation as `editable` or `generated`
- records documentation kind, source language, concepts, entity references, verification snapshot, and status
- excludes frontmatter bytes from CommonMark heading extraction while preserving full-file evidence lines
- keeps path-based identity and `markdown` language compatibility when frontmatter is absent
- rejects malformed/unclosed frontmatter and invalid explicit identity/language values
- advances persisted index state to v9 so existing projects rebuild documentation metadata once

### Markdown Frontmatter Reference Linking And Diagnostics

Status: verified.

Implemented in:

- `crates/athanor-domain/src/model.rs`
- `crates/athanor-linker-markdown/src/lib.rs`
- `crates/athanor-checker-markdown/src/lib.rs`
- `crates/athanor-app/src/check.rs`
- `crates/athanor-app/src/index_state.rs`

Purpose:

- adds canonical `documentation_reference_unresolved` and `duplicate_documentation_id` diagnostic kinds
- resolves exact stable keys declared in Markdown `entities` and `concepts` frontmatter lists
- emits verified generic `documents` relations from documentation pages to resolved targets
- attaches declaration evidence and union ownership from page and target sources
- rebuilds explicit relations when either side is affected
- diagnoses unresolved references and ambiguous duplicate document identities
- uses candidate-aware diagnostic ownership so target additions, removals, and renames invalidate findings
- includes both new diagnostic kinds in `ath check docs`
- lets API consistency checks accept verified generic `documents` relations from frontmatter
- verifies incremental resolved → unresolved → resolved transitions when only the target file changes
- advances persisted index state to v10 so existing projects build explicit relations and diagnostics once

### Replaceable OpenAPI Parser Backends

Status: verified.

Implemented in:

- `crates/athanor-extractor-openapi/src/parser.rs`
- `crates/athanor-extractor-openapi/src/lib.rs`
- `crates/athanor-extractor-openapi/tests/fixtures`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-openapi.md`
- `docs/development/library-adoption-plan.md`

Purpose:

- adds a private `OpenApiDocumentParser` boundary and `NormalizedOpenApiDocument`
- dispatches OpenAPI 3.1.x documents to `oas3` 0.22.0
- keeps OpenAPI 3.0.x support through a replaceable legacy normalized-value parser
- replaces the unmaintained `serde_yaml` dependency with `serde_yaml_ng` for preflight and fallback parsing
- records the selected parser backend in canonical endpoint/schema payloads
- verifies canonical endpoint keys and schema-use metadata across 3.0.3, 3.1.0, and 3.1.1 YAML/JSON fixtures
- keeps all third-party parser types inside the OpenAPI adapter crate
- advances persisted index state to v8 so existing projects rebuild parser metadata once

### Rust Code Extraction Slice

Status: verified.

Implemented in:

- `crates/athanor-extractor-rust`
- `crates/athanor-app/src/runtime.rs`
- `docs/adapters/extractor-rust.md`

Purpose:

- adds the built-in `builtin.extractor.rust` adapter
- parses Rust source with `syn` without changing domain/core contracts
- emits canonical module, function, and symbol entities
- emits `symbol_defined` facts connected to canonical file entities
- derives stable `symbol://rust:` keys from source module paths
- includes parser-derived line evidence and single-file ownership metadata
- advances persisted index state to v2 so existing projects rebuild unchanged Rust files once

### OpenAPI Extraction Slice

Status: verified.

Implemented in:

- `crates/athanor-extractor-openapi`
- `crates/athanor-app/src/runtime.rs`
- `docs/adapters/extractor-openapi.md`

Purpose:

- adds the built-in `builtin.extractor.openapi` adapter
- parses OpenAPI 3.x YAML and JSON without changing domain/core contracts
- emits canonical API endpoint and component-schema entities
- emits route and schema declaration facts connected to canonical file entities
- records operation metadata including methods, paths, tags, parameters, responses, and security
- advances persisted index state to v3 so existing projects rebuild supported specification files once

### API Knowledge Linker Slice

Status: verified.

Implemented in:

- `crates/athanor-linker-api`
- `crates/athanor-app/src/runtime.rs`
- `docs/adapters/linker-api.md`

Purpose:

- adds the built-in `builtin.linker.api_knowledge` adapter
- links OpenAPI operation ids to matching Rust function and method names
- links Markdown pages and sections to API operations using operation ids, path segments, and tags
- emits inferred `implemented_by`, `documents_operation`, and `documents_api` relations
- combines evidence and ownership from both sides of each relation
- scopes incremental output to pairs with at least one affected entity
- advances persisted index state to v4 so existing projects build the new cross-source relations once

### API Consistency Checker Slice

Status: verified.

Implemented in:

- `crates/athanor-checker-api`
- `crates/athanor-app/src/pipeline.rs`
- `crates/athanor-app/src/runtime.rs`
- `docs/adapters/checker-api.md`

Purpose:

- adds the built-in `builtin.checker.api_consistency` adapter
- reports OpenAPI endpoints without linked Rust implementations
- reports implemented endpoints without linked Markdown documentation
- reevaluates endpoints when relevant functions, documents, or API relations change
- includes endpoint evidence and candidate-aware ownership for incremental invalidation
- forces a safe full rebuild when files are added or removed
- advances persisted index state to v5 so existing projects build the new diagnostics once

### OpenAPI Request/Response Schema Slice

Status: verified.

Implemented in:

- `crates/athanor-extractor-openapi`
- `crates/athanor-linker-api`
- `crates/athanor-checker-api`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-openapi.md`
- `docs/adapters/linker-api.md`
- `docs/adapters/checker-api.md`

Purpose:

- records request and response schema `$ref` uses with media type and response status metadata
- collects nested references from arrays and composition schemas
- emits verified `schema_for_request` and `schema_for_response` relations for same-document component schemas
- reports `api_request_schema_mismatch` and `api_response_schema_mismatch` when local component references do not resolve
- preserves evidence and ownership on all new relations and diagnostics
- keeps external references, inline-schema materialization, and Rust type comparison deferred

### OpenAPI Example Extraction And Validation

Status: verified.

Implemented in:

- `crates/athanor-extractor-openapi`
- `crates/athanor-linker-api`
- `crates/athanor-checker-api`
- `crates/athanor-app/src/index_state.rs`

Purpose:

- materializes media-type `example` and `examples.*.value` entries as canonical `ApiExample` entities
- preserves endpoint, direction, media type, response status, example name, value, and schema metadata
- emits verified `example_for` relations from examples to their declaring endpoint
- validates examples with adapter-private `jsonschema` 0.46.5
- selects Draft 4 for OpenAPI 3.0 and Draft 2020-12 for OpenAPI 3.1
- disables file and network resolvers and caches validators by normalized schema per checker run
- reports `api_example_invalid` diagnostics with evidence and ownership
- advances persisted index state to v12 so existing projects rebuild OpenAPI knowledge once
- keeps external/schema-level examples and external schema references deferred

### Immutable API Contract Snapshots And Diff

Status: verified.

Implemented in:

- `crates/athanor-app/src/api.rs`
- `apps/ath/src/main.rs`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath api snapshot` and `ath api diff` with JSON output modes
- publishes stable-key-sorted endpoint, schema, and example contracts under `.athanor/api/snapshots`
- keeps snapshot files immutable and updates `.athanor/api/latest.json` atomically
- supports explicit `--from`/`--to` ids and automatic comparison of the latest two snapshots
- reports deterministic added, removed, and changed contract items
- classifies removed endpoints/status codes, auth/security changes, and schema-reference changes as breaking
- applies field-level schema rules for type changes, required-set changes, removed properties, and property type changes
- keeps descriptions, optional property additions, additions, and example-only changes informational
- adds `ath api breaking-changes` as a non-zero-exit CI gate over the same deterministic diff
- keeps persisted diff diagnostics separate from immutable canonical indexing snapshots

### API Contract Artifact Cleanup

Status: verified.

Implemented in:

- `crates/athanor-app/src/api.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath api cleanup`, `ath api cleanup --dry-run`, and `ath api cleanup --json`
- applies explicit retention to `.athanor/api/snapshots` and `.athanor/api/diffs`
- defaults to retaining the latest two API contract snapshots and two diff artifacts
- always retains the latest API contract snapshot selected by `.athanor/api/latest.json`
- removes diff artifacts whose endpoint snapshots are no longer retained
- keeps API contract cleanup separate from `ath index` so frequent indexing does not silently delete comparison history

### API Retention Automation

Status: verified.

Implemented in:

- `crates/athanor-app/src/api.rs`
- `crates/athanor-app/src/config.rs`
- `crates/athanor-app/src/init.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`
- `docs/adapters/checker-api.md`

Purpose:

- adds `[api.retention]` configuration with `auto_cleanup`, `keep_snapshots`, and `keep_diffs`
- keeps automatic cleanup disabled by default while preserving the existing manual `ath api cleanup` path
- runs API cleanup automatically after successful `ath api snapshot` and `ath api diff` when enabled
- adds per-command `--cleanup`, `--no-cleanup`, `--keep-snapshots`, and `--keep-diffs` overrides for snapshot and diff commands
- keeps strict API checks and `ath api breaking-changes` read-only by forcing API retention cleanup off for those gate paths
- reports automatic cleanup results in JSON and text output when cleanup runs
- preserves the latest API contract snapshot selected by `.athanor/api/latest.json` through the existing cleanup safety rules

### Evidence-Backed API Breaking Diagnostics And Strict Gate

Status: verified.

Implemented in:

- `crates/athanor-app/src/api.rs`
- `apps/ath/src/main.rs`
- `docs/architecture/pipeline.md`

Purpose:

- advances API contract snapshots and diffs to v2 with entity identity, source, and ownership
- emits `api_breaking_change_detected` domain diagnostics for every breaking diff entry
- guarantees non-empty evidence and ownership, including an artifact fallback for v1 snapshots
- persists deterministic diff reports under `.athanor/api/diffs/<from>--<to>.json`
- adds `ath check api --strict` to combine current open API diagnostics with contract breaking changes
- returns a non-zero exit status from strict mode when either diagnostic source has findings
- preserves the read-only success behavior of `ath check api` without `--strict`

### Rust Relation Graph Slice

Status: verified.

Implemented in:

- `crates/athanor-extractor-rust`
- `crates/athanor-linker-rust`
- `crates/athanor-app/src/runtime.rs`
- `docs/adapters/linker-rust.md`

Purpose:

- adds the built-in `builtin.linker.rust` adapter
- walks function block expressions to find path/method calls
- extracts `use` tree paths (imports)
- detects `#[test]` / `#[tokio::test]` attributes to map them to `EntityKind::TestCase`
- connects parent modules/symbols to child entities via `Contains` relations
- resolves relative imports and local calls dynamically looking up declared imports of parent modules
- scopes incremental output to pairs with at least one affected entity

### Lexical Search Read-Model

Status: verified.

Implemented in:

- `crates/athanor-search-tantivy`
- `crates/athanor-app/src/search.rs`
- `crates/athanor-app/src/context.rs`
- `apps/ath/src/main.rs`
- `docs/adapters/search-tantivy.md`

Purpose:

- implements the `SearchIndex` port via Tantivy 0.26.1
- adds the `ath search <query>` subcommand to query the index
- integrates lexical search into context-pack selection as a seed mechanism
- dynamically manages index rebuilds on snapshot updates using `index_meta.json`
- emits bounded `athanor.search.v1` reports with the query, limit, returned count, truncation status, omitted lower bound, canonical entity ids, stable keys, source anchors, and ownership metadata
- fetches one result past the requested limit so agent-facing output can report when results were truncated without requiring agents to inspect the search index or generated JSONL directly
- rebuilds full canonical snapshots with one Tantivy batch commit before opening the reader, avoiding per-document segment reloads and Windows memory-mapped file lock failures
- disables incremental background segment merging so open Windows readers cannot block obsolete mapped segment deletion; later full rebuilds compact the disposable index

### Code Impact Analysis

Status: verified.

Implemented in:

- `crates/athanor-app/src/impact.rs`
- `crates/athanor-app/src/lib.rs`
- `apps/ath/src/main.rs`
- `docs/adapters/impact.md`

Purpose:

- implements the `ath impact <target>` subcommand to analyze the direct and transitive blast radius of changes
- supports `--diff` mode to calculate impact based on unindexed changes in the working tree
- traverses dependency, call, containment, and test relations via a BFS graph traversal
- gathers open diagnostics attached to any entity in the blast radius

### Agent Transport Layer

Status: verified.

Implemented in:

- `crates/athanor-transport-mcp`
- `apps/ath/src/main.rs`
- `docs/adapters/transport-mcp.md`

Purpose:

- implements the standard stdio JSON-RPC Model Context Protocol (MCP) server
- exposes Athanor's query tools (`search`, `explain`, `context`, `impact`, `check`, `index`) as MCP tools
- formats Athanor app queries output as structured text contents for the agent to use
- redirects all logging and debug outputs to `stderr` to protect the JSON-RPC stdio channel

### Large-Repository Scale & Performance

Status: verified.

Implemented in:

- `crates/athanor-store-jsonl/src/lib.rs`
- `docs/adapters/store-jsonl.md`

Purpose:

- generates and writes two secondary index files on snapshot commit: `path_index.json` and `stable_key_index.json`
- optimizes memory usage by parsing JSONL line-by-line / chunk-by-chunk using a reusable line buffer

### Extraction Parallelization, Shared Downstream Inputs, And Tracing

Status: verified.

Implemented in:

- `crates/athanor-core/src/ports.rs`
- `crates/athanor-app/src/pipeline.rs`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-linker-rust/src/lib.rs`
- `crates/athanor-linker-api/src/lib.rs`
- `apps/ath/src/main.rs`
- `docs/architecture/pipeline.md`
- `docs/architecture/adapters.md`

Purpose:

- runs extractor/source-file tasks through a future stream with up to 16 concurrent in-flight tasks
- changes in-process `LinkInput` and `CheckInput` full-context lists to shared `Arc<Vec<T>>` values while preserving JSON serialization for external process adapters
- avoids cloning full entity, fact, and relation lists for every linker and checker invocation
- optimizes Rust linker qualified-name and entity-id resolution with hash maps
- optimizes API linker example, schema, and operation-id matching with lookup maps
- initializes standard CLI tracing output and emits indexing/runtime logs through tracing
- emits structured debug diagnostics for source discovery counts, affected-file classification, full-rebuild reasons, adapter input/output counts, canonical object storage counts, and snapshot commits
- keeps logs on `stderr`, preserving normal CLI text and JSON responses on `stdout`

### Security & Supply-Chain Automation

Status: verified.

Implemented in:

- `deny.toml`
- `.github/workflows/ci.yml`
- `.github/workflows/security.yml`
- `docs/development/ci.md`

Purpose:

- integrates dependency security and license compliance checks using `cargo-deny` in the main CI workflow
- configures nightly vulnerability scans using `cargo-audit` in a scheduled GitHub Actions workflow
- enforces dependency license compliance through `deny.toml`, allowing audited permissive and weak-copyleft transitive licenses while keeping BUSL scoped to explicit SurrealDB crate exceptions and banning unapproved copyleft licenses
- monitors crate advisories and checks for duplicate dependency versions
- documents all supply-chain security checks in the developer guidelines

### API Registry and Source-of-Truth Policy

Status: verified.

Implemented in:

- `crates/athanor-app/src/api_registry.rs`
- `crates/athanor-app/src/config.rs`
- `crates/athanor-app/src/check.rs`
- `apps/ath/src/main.rs`

Purpose:

- Adds `ath api registry` and `ath api registry --json` to list API endpoints, their linked handlers, and their documentation pages.
- Parses `source_of_truth` policy configurations (`hybrid`, `openapi_first`, `code_first`) from `athanor.toml`.
- Dynamically filters diagnostic findings on API checks (`ath check api`) according to the selected policy.

### Documentation Frontmatter Patch Workflow

Status: verified.

Implemented in:

- `crates/athanor-app/src/docs.rs`
- `apps/ath/src/main.rs`
- `docs/development/docs-completeness-policy.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath docs propose-fix`
- adds `ath docs apply-patch <patch-id-or-path>`
- writes reviewable `athanor.docs_patch.v1` JSON proposals under `.athanor/patches/docs/` by default
- proposes deterministic frontmatter fixes for documentation completeness and drift findings
- proposes Markdown API documentation pages for implemented endpoints that lack linked documentation
- proposes skeletal Markdown operations pages for undocumented environment variables
- applies proposals only through an explicit command
- refuses to overwrite existing files for create operations
- rejects stale proposals whose snapshot does not match the latest canonical snapshot
- keeps existing API documentation update drafts deferred

### API Documentation Draft Enrichment

Status: verified.

Implemented in:

- `crates/athanor-app/src/docs.rs`
- `docs/development/docs-completeness-policy.md`
- `docs/architecture/pipeline.md`

Purpose:

- enriches `ath docs propose-fix` API documentation create drafts from the canonical API graph
- includes endpoint method, path, operation id, tags, declared response codes, and security payloads
- includes linked Rust handler source when an `implemented_by` relation is available
- includes linked request/response schemas from `schema_for_request` and `schema_for_response` relations
- includes linked examples from `example_for` relations
- preserves diagnostic evidence and review-before-apply semantics
- keeps narrative rewrites and multi-page API documentation edits deferred

### Existing API Documentation Patch Updates

Status: verified.

Implemented in:

- `crates/athanor-app/src/docs.rs`
- `docs/development/docs-completeness-policy.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends `ath docs propose-fix` beyond create-only API documentation drafts
- finds existing editable API pages that declare an endpoint in frontmatter or are linked through canonical documentation relations
- limits managed contract updates to pages marked as API documentation
- proposes endpoint-specific Athanor-managed API contract blocks delimited by `athanor:api-doc` comments
- refreshes existing managed blocks when endpoint contract facts or graph relations change
- supports multiple endpoint blocks in one API documentation page
- adds missing endpoint stable keys to API page frontmatter `entities` when canonical documentation relations already link the page to the endpoint
- proposes generated coordination blocks when one endpoint is documented by multiple editable API pages
- proposes generated narrative review blocks when human-authored API text mentions routes that do not match the page's current linked endpoints
- preserves human-authored Markdown outside managed blocks
- applies the update only through explicit `ath docs apply-patch`
- leaves direct human-authored narrative rewriting out of scope

### Stale API Narrative Rewrite Drafts

Status: verified.

Implemented in:

- `crates/athanor-app/src/docs.rs`
- `docs/development/docs-completeness-policy.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends `ath docs propose-fix` stale API narrative handling beyond route review lists
- includes reviewable original-line and draft-line suggestions in generated narrative review blocks
- proposes deterministic route replacements only when an editable API page has exactly one linked current endpoint
- skips direct rewrite drafts when multiple linked endpoint routes make the replacement ambiguous
- preserves human-authored Markdown outside generated blocks
- applies proposed content only through explicit `ath docs apply-patch`

### Environment Documentation Check View

Status: verified.

Implemented in:

- `crates/athanor-extractor-rust/src/lib.rs`
- `crates/athanor-extractor-operations`
- `crates/athanor-checker-api/src/lib.rs`
- `crates/athanor-app/src/check.rs`
- `crates/athanor-app/src/docs.rs`
- `crates/athanor-app/src/index_state.rs`
- `apps/ath/src/main.rs`
- `crates/athanor-transport-mcp/src/lib.rs`
- `docs/adapters/extractor-rust.md`
- `docs/adapters/extractor-operations.md`
- `docs/adapters/checker-api.md`
- `docs/architecture/pipeline.md`
- `docs/development/docs-completeness-policy.md`

Purpose:

- extracts Rust environment-variable usage as canonical `EnvVar` entities and `env_var_used` facts
- uses runtime configuration `Feature` entities from the operations extractor as environment-scope documentation targets
- checks whether environment variables are linked from editable Markdown documentation through `documents` relations
- checks whether runtime configuration keys are linked from editable Markdown documentation through `documents` relations
- exposes the findings through `ath check env` and `ath check env --json`
- keeps environment diagnostics separate from generic documentation structure diagnostics
- exposes the same `env` scope through the MCP `check` tool
- integrates `missing_env_var` diagnostics with `ath docs propose-fix` so agents can review and apply operations documentation drafts
- integrates scoped runtime configuration `missing_documentation` diagnostics with `ath docs propose-fix`
- advances persisted index state to v28 so existing projects rebuild environment-scope documentation diagnostics once

### Operations File Extraction

Status: verified.

Implemented in:

- `crates/athanor-extractor-operations`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-operations.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds the built-in `builtin.extractor.operations` adapter
- parses dotenv-style files such as `.env.example`, `.env`, and `*.env`
- emits canonical `EnvVar` entities from `KEY=value` and `export KEY=value` declarations
- emits redacted `env_var_used` facts with declaration evidence and ownership
- parses Makefile targets from `Makefile`, `makefile`, and `*.mk` as `ScriptCommand` entities
- parses Dockerfile stages as `DockerService` entities
- parses Dockerfile `RUN`, `CMD`, and `ENTRYPOINT` instructions as `ScriptCommand` entities
- parses Dockerfile `ENV` declarations as redacted environment-variable knowledge
- emits evidence-backed `symbol_defined` facts for operational targets, stages, and commands
- avoids storing raw dotenv or Dockerfile environment values in canonical snapshots
- advances persisted index state to v14 so existing projects rebuild operational knowledge once
- keeps CI, deployment, and runbook extraction deferred

### Shell Script Operational Extraction

Status: verified.

Implemented in:

- `crates/athanor-extractor-operations`
- `crates/athanor-extractor-operations/README.md`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-operations.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends the built-in `builtin.extractor.operations` adapter to `*.sh`, `*.bash`, and `*.zsh`
- parses `export KEY=value` and `readonly KEY=value` as redacted `EnvVar` knowledge
- parses `name() {`, `function name {`, and `function name() {` as `ScriptCommand` function entities
- emits evidence-backed `env_var_used` facts for shell environment declarations
- emits evidence-backed `symbol_defined` facts for shell function definitions
- avoids storing raw shell environment values in canonical snapshots
- advances persisted index state to v15 so existing projects rebuild operational knowledge once
- keeps command invocation, sourced file, control-flow, trap, here-document, CI, deployment, and runbook extraction deferred

### Docker Compose Operational Extraction

Status: verified.

Implemented in:

- `crates/athanor-extractor-operations`
- `crates/athanor-extractor-operations/Cargo.toml`
- `crates/athanor-extractor-operations/README.md`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-operations.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends the built-in `builtin.extractor.operations` adapter to common docker-compose file names
- parses top-level compose services as `DockerService` entities
- records service image and build context metadata when present
- parses service `command` and `entrypoint` declarations as `ScriptCommand` entities
- parses compose service `environment` mapping and array forms as redacted `EnvVar` knowledge
- emits evidence-backed `symbol_defined` facts for compose services and service commands
- emits evidence-backed `env_var_used` facts for compose environment declarations
- avoids storing raw compose environment values in canonical snapshots
- advances persisted index state to v16 so existing projects rebuild operational knowledge once
- keeps `env_file`, profiles, includes, extends, anchors, volume semantics, healthchecks, dependencies, networks, deployment, and runbook extraction deferred

### GitHub Actions Operational Extraction

Status: verified.

Implemented in:

- `crates/athanor-extractor-operations`
- `crates/athanor-extractor-operations/README.md`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-operations.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends the built-in `builtin.extractor.operations` adapter to `.github/workflows/*.yml` and `.github/workflows/*.yaml`
- parses workflow declarations as `ScriptCommand` entities
- parses workflow jobs and `runs-on` metadata as `ScriptCommand` entities
- parses step `run` commands and `uses` action references as `ScriptCommand` entities
- parses top-level, job-level, and step-level `env` mappings as redacted `EnvVar` knowledge
- emits evidence-backed `symbol_defined` facts for workflow, job, and step declarations
- emits evidence-backed `env_var_used` facts for workflow environment declarations
- avoids storing raw GitHub Actions environment values in canonical snapshots
- advances persisted index state to v17 so existing projects rebuild operational knowledge once
- keeps expression evaluation, permissions, matrices, reusable workflows, service containers, caches, artifacts, secrets, deployment, and runbook extraction deferred

### Cargo Manifest Operational Extraction

Status: verified.

Implemented in:

- `crates/athanor-extractor-operations`
- `crates/athanor-extractor-operations/README.md`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-operations.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends the built-in `builtin.extractor.operations` adapter to `Cargo.toml`
- parses Cargo package declarations as `Package` entities
- parses Cargo workspace declarations and workspace members as `Package` entities
- parses dependencies, dev-dependencies, build-dependencies, workspace dependencies, and target-specific dependencies as `Dependency` entities
- records dependency version, path, git, registry, package alias, optional, and feature metadata when present
- emits evidence-backed `symbol_defined` facts for package, workspace, and dependency declarations
- advances persisted index state to v18 so existing projects rebuild operational knowledge once
- keeps inherited workspace field resolution, target expression evaluation, patches, replacements, profiles, build scripts, other deployment configs, database migrations, runtime configuration, and runbook extraction deferred

### Kubernetes Deployment Manifest Extraction

Status: verified.

Implemented in:

- `crates/athanor-extractor-operations`
- `crates/athanor-extractor-operations/README.md`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-operations.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends the built-in `builtin.extractor.operations` adapter to common Kubernetes YAML manifest paths and filenames
- parses YAML documents with `kind` and `metadata.name` as deployment/service knowledge
- records Kubernetes workloads, services, ConfigMaps, Secrets, and related manifest resources as `DockerService` entities
- records container images and container names in resource payloads
- parses container `command` and `args` declarations as `ScriptCommand` entities
- parses container `env` declarations and ConfigMap/Secret `data` keys as redacted `EnvVar` knowledge
- emits evidence-backed `symbol_defined` facts for Kubernetes resources and container commands
- emits evidence-backed `env_var_used` facts for Kubernetes environment declarations
- avoids storing raw Kubernetes Secret, ConfigMap, or container environment values in canonical snapshots
- advances persisted index state to v19 so existing projects rebuild operational knowledge once
- keeps Helm/Kustomize evaluation, `envFrom`, projected volumes, probes, selectors, RBAC semantics, rollout strategy, advanced database migration semantics, runtime configuration, and runbook extraction deferred

### SQL Database Migration Extraction

Status: verified.

Implemented in:

- `crates/athanor-extractor-operations`
- `crates/athanor-extractor-operations/README.md`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-operations.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends the built-in `builtin.extractor.operations` adapter to SQL migration files in common migration paths and filenames
- parses migration files as `DbMigration` entities
- parses simple `CREATE TABLE [IF NOT EXISTS] [schema.]table` statements as `DbTable` entities
- emits evidence-backed `symbol_defined` facts for SQL migration files
- emits evidence-backed `migration_creates_table` facts from migrations to created tables
- advances persisted index state to v20 so existing projects rebuild operational knowledge once
- keeps quoted dotted identifiers, column details, constraints, `ALTER TABLE`, views, indexes, triggers, functions, down migrations, ORM-specific migration metadata, advanced runtime configuration semantics, and runbook extraction deferred

### Runtime Configuration Extraction

Status: verified.

Implemented in:

- `crates/athanor-extractor-operations`
- `crates/athanor-extractor-operations/README.md`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-operations.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends the built-in `builtin.extractor.operations` adapter to JSON, TOML, and YAML runtime configuration files in common config/settings paths and filenames
- flattens scalar configuration keys into redacted `Feature` entities
- records scalar value kinds without storing raw configuration values
- records env-like uppercase config keys as redacted `EnvVar` knowledge
- emits evidence-backed `symbol_defined` facts for runtime configuration keys
- emits evidence-backed `env_var_used` facts for env-like runtime configuration keys
- advances persisted index state to v21 so existing projects rebuild operational knowledge once
- keeps framework-specific config schemas, environment interpolation, includes/imports, profiles, encrypted values, arrays of objects, and runbook extraction deferred

### Script Documentation Check View

Status: verified.

Implemented in:

- `crates/athanor-checker-api`
- `crates/athanor-checker-api/README.md`
- `crates/athanor-app/src/check.rs`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-transport-mcp/src/lib.rs`
- `apps/ath/src/main.rs`
- `docs/adapters/checker-api.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds the built-in `builtin.checker.script_docs` adapter
- checks canonical `ScriptCommand` entities for explicit Markdown `documents` relations
- emits evidence-backed `missing_documentation` diagnostics with payload scope `scripts`
- exposes the findings through `ath check scripts` and `ath check scripts --json`
- exposes the same `scripts` scope through the MCP `check` tool
- keeps the command read-only and non-failing, matching the initial API/docs/env check views
- advances persisted index state to v22 so existing projects rebuild script documentation diagnostics once
- keeps rollout and deeper runbook consistency checks deferred

### Deployment Documentation Check View

Status: verified.

Implemented in:

- `crates/athanor-checker-api`
- `crates/athanor-checker-api/README.md`
- `crates/athanor-app/src/check.rs`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-transport-mcp/src/lib.rs`
- `apps/ath/src/main.rs`
- `docs/adapters/checker-api.md`
- `docs/adapters/transport-mcp.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds the built-in `builtin.checker.deployment_docs` adapter
- checks canonical `DockerService` deployment/service entities for explicit Markdown `documents` relations
- emits evidence-backed `missing_documentation` diagnostics with payload scope `deployment`
- exposes the findings through `ath check deployment` and `ath check deployment --json`
- exposes the same `deployment` scope through the MCP `check` tool
- keeps the command read-only and non-failing, matching the initial API/docs/env/scripts check views
- advances persisted index state to v23 so existing projects rebuild deployment documentation diagnostics once
- keeps rollout and deeper runbook consistency checks deferred

### Runbook Consistency Check View

Status: verified.

Implemented in:

- `crates/athanor-extractor-markdown`
- `crates/athanor-extractor-markdown/README.md`
- `crates/athanor-linker-markdown`
- `crates/athanor-checker-api`
- `crates/athanor-checker-api/README.md`
- `crates/athanor-app/src/check.rs`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-transport-mcp/src/lib.rs`
- `apps/ath/src/main.rs`
- `docs/adapters/extractor-markdown.md`
- `docs/adapters/linker-markdown.md`
- `docs/adapters/checker-api.md`
- `docs/adapters/transport-mcp.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- emits canonical `Runbook` entities from Markdown frontmatter `kind: runbook` or `kind: operations_runbook`
- emits canonical `OperationStep` entities from ordered-list items in runbook Markdown bodies
- derives `runbook://...` stable keys from the source documentation page identity
- derives `runbook://...#step-N` stable keys from runbook identity and step sequence
- records runbook operation targets from frontmatter `entities`
- links documentation pages to emitted runbook entities through verified `contains` relations
- links emitted runbook entities to operation-step entities through verified `contains` relations
- adds the built-in `builtin.checker.runbook_consistency` adapter
- checks canonical `Runbook` entities for at least one known operational target
- checks canonical `Runbook` entities for at least one extracted operation step
- checks extracted operation steps for an explicit reference to at least one declared operational target stable key, name, title, or alias
- checks that every declared operational target is covered by at least one extracted operation step
- emits evidence-backed `stale_documentation` diagnostics with payload scope `runbooks`
- exposes the findings through `ath check runbooks` and `ath check runbooks --json`
- exposes the same `runbooks` scope through the MCP `check` tool
- keeps the command read-only and non-failing, matching the other initial operational check views
- advances persisted index state to v29 so existing projects rebuild runbook knowledge and diagnostics once
- keeps step dependencies and richer runbook semantics deferred

### Operations Documentation Draft Expansion

Status: verified.

Implemented in:

- `crates/athanor-app/src/docs.rs`
- `docs/development/docs-completeness-policy.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends `ath docs propose-fix` operations draft generation beyond `missing_env_var`
- creates reviewable Markdown operations pages for scoped script `missing_documentation` diagnostics
- creates reviewable Markdown operations pages for scoped deployment `missing_documentation` diagnostics
- creates reviewable Markdown operations pages for scoped runbook `stale_documentation` diagnostics
- writes drafts under `<editable_path>/operations/` with frontmatter `entities` pointing at the missing or stale operational stable key
- includes source evidence, canonical entity kind, and review notes without modifying editable documentation until `docs apply-patch`

### Operations Documentation Check Workflow

Status: verified.

Implemented in:

- `crates/athanor-app/src/check.rs`
- `apps/ath/src/main.rs`
- `docs/development/docs-completeness-policy.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath docs operations check` and `ath docs operations check --json`
- aggregates environment, script, deployment, and runbook documentation diagnostics from one latest canonical snapshot load
- reuses the same scoped reports as `ath check env`, `ath check scripts`, `ath check deployment`, and `ath check runbooks`
- emits stable `athanor.operations_docs_check.v1` JSON with total counts and per-scope reports
- returns a non-zero process status when any operational documentation diagnostic is open
- keeps source files read-only and leaves remediation to `ath docs propose-fix` and `ath docs apply-patch`

### Canonical Merge Deduplication

Status: verified.

Implemented in:

- `crates/athanor-app/src/pipeline.rs`
- `docs/architecture/pipeline.md`

Purpose:

- canonicalizes merged entities, facts, relations, and diagnostics by canonical id before storage
- removes duplicate canonical diagnostics that can be carried forward from older snapshots
- ensures current-run objects replace carried objects on id conflicts during incremental indexing
- keeps generated JSONL read models and scoped check views backed by the deduplicated canonical snapshot
- avoids moving conflict policy into the JSONL store adapter, preserving the app-layer ownership of incremental merge behavior

### OpenAPI Test Fixture Exclusion

Status: verified.

Implemented in:

- `crates/athanor-extractor-openapi`
- `crates/athanor-extractor-openapi/README.md`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/extractor-openapi.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`

Purpose:

- prevents OpenAPI files under `tests/fixtures` from being extracted as product API knowledge during repository self-indexing
- keeps intentionally incomplete parser and contract fixtures available to adapter tests without creating false-positive API diagnostics
- advances persisted index state to v26 so existing projects rebuild canonical API knowledge once
- keeps source discovery broad enough to retain fixture file entities while scoping product API extraction in the OpenAPI adapter

### Canonical Graph JSON And GraphML Export

Status: verified.

Implemented in:

- `crates/athanor-app/src/graph.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath graph export --format json`
- adds `ath graph export --format graphml`
- reads the latest durable canonical snapshot without re-indexing
- emits the stable `athanor.graph_export.v1` JSON graph payload
- emits a GraphML-compatible representation of the same bounded graph for external graph tooling
- ranks nodes deterministically by relation degree and stable key
- includes canonical node ids, stable keys, kinds, names, source anchors, degrees, relation ids, relation kinds, endpoints, status, confidence, and evidence anchors
- bounds output with `--max-entities` and `--max-relations`
- reports omitted node and edge counts when limits truncate the disposable read model
- keeps graph export as an app-layer query over canonical entities and relations, not a new store or source of truth

### Repository Overview Structure Summaries

Status: verified.

Implemented in:

- `crates/athanor-app/src/overview.rs`
- `apps/ath/src/main.rs`
- `docs/architecture/pipeline.md`

Purpose:

- extends `ath overview` with bounded canonical module structure summaries
- ranks modules by direct `defines` and `contains` members, then stable key
- aggregates directional integration boundaries from canonical relations crossing source roots
- includes relation-kind counts and bounded canonical relation ids for traceability
- applies the existing `--top` limit to modules, boundaries, relation kinds, and sampled relation ids
- keeps overview generation deterministic, read-only, and derived from the latest canonical snapshot

### Related Entity Graph Navigation

Status: verified.

Implemented in:

- `crates/athanor-app/src/graph.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath graph related <stable-key>` and JSON output
- traverses incoming and outgoing canonical relations breadth-first from one exact stable key
- bounds traversal depth, entity count, and relation count
- sorts neighbors, nodes, and relations deterministically
- reports per-node distance plus canonical entity and relation ids
- retains relation status, confidence, and evidence anchors for traceability
- marks results when entity or relation limits truncate the exploration
- keeps graph navigation read-only and derived from the latest canonical snapshot

### Shortest Graph Path Navigation

Status: verified.

Implemented in:

- `crates/athanor-app/src/graph.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath graph path <from-stable-key> <to-stable-key>` and JSON output
- searches incoming and outgoing canonical relations with deterministic breadth-first traversal
- returns one ordered shortest path while retaining each relation's canonical direction
- includes canonical entity ids, relation ids, status, confidence, and evidence anchors
- bounds search by maximum depth and visited entity count
- distinguishes complete no-path results from searches truncated by configured limits
- keeps path navigation read-only and derived from the latest canonical snapshot

### Graph Hub Degree Centrality

Status: verified.

Implemented in:

- `crates/athanor-app/src/graph.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath graph hubs` and JSON output
- ranks connected canonical entities by total degree
- reports incoming and outgoing degree separately
- retains bounded sorted incoming and outgoing canonical relation ids
- supports filtering by serialized canonical entity kind
- bounds ranked entities and relation ids per direction
- sorts ties deterministically by incoming degree, outgoing degree, and stable key
- keeps centrality reporting read-only and derived from the latest canonical snapshot

### Directed Graph PageRank Centrality

Status: verified.

Implemented in:

- `crates/athanor-app/src/graph.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath graph pagerank` and JSON output
- calculates directed PageRank over the complete latest canonical entity/relation graph
- redistributes dangling-node score across all canonical entities
- bounds computation by maximum iterations and convergence tolerance
- bounds output entities and incoming canonical relation trace ids
- applies optional entity-kind filtering after full-graph scoring so filters do not alter centrality
- reports graph counts, effective iterations, convergence state, scores, stable keys, source anchors, omitted result counts, and bounded canonical incoming relation traces with evidence anchors
- sorts equal scores deterministically by stable key
- keeps PageRank read-only and derived from canonical relations rather than creating a second graph source of truth

### Directed Graph Cycle Detection

Status: verified.

Implemented in:

- `crates/athanor-app/src/graph.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath graph cycles` and JSON output
- finds simple cycles that follow canonical relation direction
- orders search roots and outgoing relations deterministically
- deduplicates the same directed cycle found from different starting entities
- returns ordered canonical entity and relation ids with relation evidence anchors
- bounds cycle length, search roots, and unique result count
- reports omitted starts and whether configured limits truncated the search
- keeps cycle detection read-only and derived from the latest canonical snapshot

### HTML Report Graph And Entity Detail Pages

Status: verified.

Implemented in:

- `crates/athanor-projector-html/src/lib.rs`
- `crates/athanor-projector-html/README.md`
- `docs/adapters/projector-html.md`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- extends `ath report html` output with one `entities/<entity-id>.html` detail page per canonical entity
- adds entity detail sections for identity, ownership, attached relations, attached facts, diagnostics, and evidence locations
- adds a compact graph summary to `index.html` with relation-kind counts and high-degree connected entities
- adds a bounded interactive SVG graph with up to 80 high-degree entities and 240 canonical relations
- supports node search, relation-kind filtering, zoom, deterministic layout reset, node dragging, canonical detail links, and evidence-backed direct relation inspection
- reports omitted node and relation counts so the interactive read model is explicit about truncation
- adds embedded client-side filters for entity search, source path, entity kind, and diagnostic severity
- keeps the report self-contained with embedded CSS/script and no network dependencies
- keeps HTML report files disposable read models generated from the latest canonical snapshot

### Agent Bounded Retrieval Contract

Status: verified.

Implemented in:

- `AGENTS.md`
- `docs/development/agent-workflow.md`
- `docs/development/definition-of-done.md`
- `docs/architecture/pipeline.md`
- `docs/development/roadmap-status.md`

Purpose:

- establishes that generated JSONL, wiki, HTML, graph, API, search, and vector artifacts are backing read models or inspection outputs
- forbids normal agent workflows from depending on full generated artifact reads
- requires agent-facing access to use bounded query/context commands with explicit limits and stable schemas
- requires canonical ids, stable keys, source anchors, and evidence links in bounded agent-facing outputs
- requires omitted or truncated counts when limits hide available canonical data
- requires any future large generated artifact to ship with a bounded retrieval path or explicitly document the existing bounded query that covers it before the feature is complete

### Explicit Multi-Repository Project Registry

Status: verified.

Implemented in:

- `crates/athanor-app/src/project_registry.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`
- `README.md`

Purpose:

- adds `ath projects list`, `add`, `resolve`, and `remove`
- stores explicit project-id to canonical-root mappings in `~/.athanor/projects.json` by default
- supports `ATHANOR_PROJECT_REGISTRY` and per-command `--registry` overrides
- emits stable `athanor.project_registry.v1` and `athanor.project_resolution.v1` JSON reports
- rejects invalid ids, duplicate ids, duplicate canonical roots, unknown schemas, and implicit default-project selection
- publishes registry updates through staged replacement
- keeps canonical snapshots, state, generated outputs, API artifacts, and configuration isolated under each repository root
- establishes the routing contract that future daemon and MCP multi-repository requests must name and resolve one exact project id before querying knowledge

### Daemon Lifecycle And Read-Only Protocol Slice

Status: verified.

Implemented in:

- `apps/athd/src/main.rs`
- `crates/athanor-app/src/daemon.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds the `athd` daemon entrypoint with background `start`, foreground `serve`, `ping`, `status`, `jobs`, `job`, `cancel`, `index`, `generate`, `wiki`, `report-html`, `overview`, `explain`, `search`, `context`, and `stop`
- makes background start readiness-bounded and idempotent for an already healthy project daemon, with structured logs redirected to the protected per-user runtime directory
- resolves every command through the explicit project registry before connecting to or serving a repository daemon
- writes per-project endpoint, token, lock, and log files under the protected per-user runtime root outside the repository
- prevents two daemon instances from owning the same project runtime directory through an OS advisory file lock that recovers automatically after crashes
- uses authenticated `athanor.daemon_endpoint.v2`, `athanor.daemon_request.v2`, and `athanor.daemon_response.v2` schemas with a fresh per-process 256-bit token, loopback-only TCP, optional Unix domain socket or Windows named pipe transport, and explicit loopback-only v1 migration mode
- records configurable daemon request and response byte limits in endpoint metadata and applies them on both server and client sides
- optionally watches the project root with `notify-debouncer-mini`, supports platform-recommended or polling watcher backends, ignores `.athanor` artifact events, and schedules debounced background indexing jobs after source changes
- bounds daemon request and response messages to 1 MiB, returning structured daemon errors for oversized computed responses
- handles requests concurrently up to `--max-concurrent-requests` and returns structured busy errors after the limit is reached
- adds an in-memory daemon job registry and bounded `athanor.daemon_jobs.v1` job listing report
- records lifecycle jobs and read-only `overview`/`explain`/`search`/`context` request jobs with running, succeeded, or failed status
- bounds retained in-memory job history with `--max-job-history`, pruning oldest finished records first
- supports exact daemon job lookup by stable job id
- starts one background indexing job through `athd index`, reusing the existing `index_project` implementation and rejecting concurrent index jobs
- records structured index job results with snapshot id, file counts, and JSONL output directory
- caches the latest canonical snapshot, one snapshot-keyed Tantivy search handle shared by search and context, and bounded least-recently-used overview/non-diff-context results; successful daemon-owned index jobs invalidate the complete cache epoch, while diff context remains on current source discovery
- starts one background coordinated generation job through `athd generate`, reusing `generate_project`, rejecting concurrent generation jobs, and recording generation id, snapshot id, pointer path, and canonical object counts
- starts background direct projection jobs through `athd wiki` and `athd report-html`, reusing the existing projector services, rejecting concurrent jobs of the same kind, and recording snapshot id, output directory, and canonical object counts
- registers background index and projection jobs with shared cancellation tokens before worker start, cancels queued jobs immediately, and moves running jobs through `cancelling` to safe index-stage and staged projector-loop checkpoints before canonical snapshot or generated-output publication
- preserves atomic publication semantics when cancellation arrives too late: an already-started commit or publication completes and the job reports success instead of exposing partially published state
- rejects requests whose project id does not match the daemon endpoint
- serves read-only status, bounded overview, exact entity explanation, bounded lexical search, and bounded task context responses from the latest canonical snapshot
- exposes daemon context level and limit overrides, including diff-based changed-file context
- keeps logs separate from structured protocol output
- completes Phase 7 with native local TCP/local-socket access independent of the optional MCP transport adapter

### Production V1 Hardening

Status: verified.

Implemented in:

- `crates/athanor-app/src/daemon.rs`
- `crates/athanor-app/src/daemon_runtime.rs`
- `apps/athd/src/main.rs`
- `.github/workflows/release.yml`
- `.github/workflows/production.yml`
- `docs/development/production.md`

Purpose:

- adds authenticated daemon protocol v2 and protected per-user runtime paths for Windows and Linux
- rejects non-loopback TCP, unauthenticated requests, and protocol v1 unless explicitly enabled for loopback migration
- adds crash-safe OS locking, lifecycle status, cooperative stop cancellation, bounded job drain, health diagnostics, and stale staging cleanup under known artifact roots
- adds idempotent per-user Task Scheduler and `systemd --user` service install, status, and uninstall commands
- rotates structured JSONL daemon logs at 10 MiB with five retained files
- disables external process adapters by default and requires explicit project opt-in
- adds optimized release builds, signed and attested Windows/Linux archives, authenticated daemon E2E, Windows service E2E, and nightly watcher/query soak coverage

### External Process Adapter Runtime Limits

Status: verified.

Implemented in:

- `crates/athanor-app/src/runtime.rs`
- `docs/architecture/adapters.md`
- `docs/development/production.md`

Purpose:

- rejects unknown adapter manifest, adapter entry, and process command fields during manifest parsing
- rejects empty command programs, parent-directory components, and bare command names that would otherwise resolve through `PATH`
- canonicalizes absolute command paths before execution
- canonicalizes manifest-relative command paths and requires them to stay inside the manifest directory
- requires every external command program to match `[adapters].external_process_allowlist` after canonicalization
- bounds serialized stdin, stdout, stderr, and wall-clock execution for external process adapters
- terminates timed-out adapter processes and reports bounded adapter-scoped errors
- reports oversized stdout, oversized stderr, invalid JSON, and non-zero exit status without unbounded process output capture
- keeps external process adapter output on the existing canonical evidence and ownership validation path

### External Process Adapter User Trust

Status: verified.

Implemented in:

- `crates/athanor-app/src/runtime.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/operations/env-athanor-adapter-trust.md`
- `docs/architecture/adapters.md`
- `docs/development/production.md`

Purpose:

- adds a user-level adapter trust store at `~/.athanor/adapter-trust.json` or `ATHANOR_ADAPTER_TRUST`
- documents `ATHANOR_ADAPTER_TRUST` as the explicit adapter trust store override
- records trusted adapter manifests by canonical manifest path and SHA-256 manifest content hash
- requires `[adapters].allow_external_process = true`, a matching executable allowlist entry, and a matching user-level trust record before loading external process adapters from discovered manifests
- invalidates trust automatically when a manifest changes
- adds `ath plugins list`, `ath plugins trust <manifest>`, and `ath plugins untrust <manifest>` with optional `--trust-store` overrides and JSON output
- keeps trust state outside the repository so a cloned repo cannot trust its own executable adapters

### Near-Term Hardening And Scale Audit

Status: verified.

Audited in:

- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-app/src/pipeline.rs`
- `crates/athanor-app/src/daemon.rs`
- `crates/athanor-app/src/daemon_runtime.rs`
- `crates/athanor-app/src/index.rs`
- `apps/ath/src/main.rs`
- `apps/athd/src/main.rs`
- `docs/architecture/pipeline.md`
- `docs/architecture/adapters.md`
- `docs/development/production.md`
- `.github/workflows/production.yml`
- `.github/workflows/release.yml`

Classification:

- Implemented: authenticated daemon protocol v2 with fresh per-process token, protected runtime paths, loopback-only TCP, optional local socket transport, request and response byte limits, structured busy responses, bounded job history, cooperative cancellation tokens for writable jobs, daemon cache invalidation after index jobs, watcher debounce with `.athanor` artifact filtering, signed and attested release archives, production daemon E2E workflow, nightly watcher/query soak workflow, external-process adapter opt-in, executable allowlisting, bounded process execution, strict manifest parsing, and user-level manifest trust.
- Implemented but under-tested: daemon lifecycle edge cases beyond the current unit and workflow coverage, including client disconnect during request handling, crash/restart with stale endpoint/token/lock metadata, corrupted endpoint metadata, cancellation during real index/generation jobs, parallel read-only query traffic during index jobs, watcher event storms, Unix socket permission checks, and Windows named-pipe lifecycle checks.
- Implemented scale follow-up: in-process linker and checker inputs share full-context `Arc<Vec<T>>` values across phases without rebuilding complete entity and fact lists at linker/checker phase entry; JSON serialization for external process adapter `LinkInput` and `CheckInput` remains array-compatible.
- Not implemented: benchmark fixtures for small/medium/large repositories, capability/coverage reporting for incomplete analysis, and one consolidated release-readiness checklist.

Resulting P1 follow-up plan:

- Pipeline benchmarks and equivalence: add broader golden equivalence tests over canonical entities, facts, relations, diagnostics, stable ids, evidence, and ownership; add small, medium, and large fixture benchmarks; update `docs/architecture/pipeline.md` and this roadmap entry.
- Daemon fault-injection coverage: extend tests around `crates/athanor-app/src/daemon.rs`, `crates/athanor-app/src/daemon_runtime.rs`, and `apps/athd/src/main.rs` for disconnects, stale and corrupted runtime metadata, cancellation under real writable jobs, concurrent read-only traffic during index jobs, watcher event storms, Unix socket permissions, and Windows named-pipe lifecycle; document any platform edge case that cannot be represented in automated tests in `docs/development/production.md` and `docs/architecture/pipeline.md`.

### Full/Incremental Canonical Equivalence Test

Status: verified.

Implemented in:

- `crates/athanor-app/src/index.rs`
- `docs/development/roadmap-status.md`

Purpose:

- adds an app-layer regression test that indexes one fixture incrementally and another fixture from scratch in the same final source state
- loads durable canonical snapshots directly from `JsonlKnowledgeStore` instead of reading generated JSONL read models
- compares canonical entities, facts, relations, and diagnostics after normalizing snapshot ids, including snapshot values nested in payloads
- verifies that incremental changed-file indexing preserves stable ids, evidence, ownership, relations, and diagnostics equivalently to a fresh full index for the covered mixed Markdown/Rust/OpenAPI fixture

### Bounded Index Metrics Report

Status: verified.

Implemented in:

- `crates/athanor-app/src/pipeline.rs`
- `crates/athanor-app/src/index.rs`
- `crates/athanor-app/src/daemon.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`
- `docs/development/roadmap-status.md`

Purpose:

- adds bounded `athanor.index_metrics.v1` pipeline metrics to `IndexPipelineOutput`
- records wall-clock phase timings for source discovery, affected-file classification, snapshot begin, extraction, merge, linking, checking, canonicalization, and canonical storage
- records discovered, changed, unchanged, removed, extracted, and final canonical object counts
- aggregates adapter metrics by phase and adapter name so extraction metrics remain bounded by configured adapters rather than source-file count
- records adapter runs, timings, input counts, output counts, validation issue counts, timeout counts, and optional process byte counters where supported
- adds `athanor.index_report_metrics.v1` to `IndexReport`, including pipeline metrics plus JSONL read-model, validation-result, and index-state write timings
- adds `ath index --json` so agents and automation can read the bounded metrics report without reading generated JSONL artifacts
- includes metrics in daemon index job results and MCP index tool responses through the shared serialized `IndexReport`

### Pipeline Benchmark Fixtures

Status: verified.

Implemented in:

- `crates/athanor-app/src/bench.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`
- `docs/development/roadmap-status.md`

Purpose:

- adds `ath bench --size <small|medium|large>` for synthetic Markdown, Rust, and OpenAPI indexing fixtures
- runs the normal `index_project` path so source discovery, extraction, linking, checking, canonical storage, JSONL read-model writing, and index-state writing are measured together
- emits `athanor.index_benchmark.v1` in JSON mode, including the nested bounded `IndexReport` and pipeline metrics
- supports temporary fixtures by default and optional `--root <path> --keep-fixture` for repeatable local inspection
- keeps performance regression inspection bounded through report metrics instead of requiring agents to read generated JSONL artifacts directly

### Daemon Fault-Injection Coverage Slice

Status: verified.

Implemented in:

- `crates/athanor-app/src/daemon.rs`
- `crates/athanor-app/src/daemon_runtime.rs`
- `docs/development/roadmap-status.md`

Purpose:

- treats client disconnects during response writes as non-fatal connection outcomes instead of failed daemon request handling
- adds coverage that malformed daemon request shapes are rejected without creating daemon jobs, and stopping daemons allow lifecycle reads while rejecting new work
- adds coverage that invalid read/query command parameters are rejected without creating daemon jobs, and cancelling a running read-only job returns a non-cancellable error without mutating the job
- adds coverage that duplicate writable daemon jobs are rejected without creating a second job, and protocol cancellation of a queued writable job finishes it and removes its cancellation token
- adds coverage that invalid request JSON, project mismatches, empty daemon responses, and invalid daemon response JSON fail with bounded errors without creating work
- adds coverage that unsupported endpoint protocol metadata is rejected before connection and busy responses mask invalid authentication as a generic authentication failure
- adds coverage for clients disconnecting before sending a request without creating daemon jobs
- adds coverage for oversized daemon requests returning structured errors without creating daemon jobs
- adds coverage for client-side daemon byte limits: oversized outbound requests are not written and oversized wire responses are rejected before parsing
- adds coverage for stale and corrupted runtime metadata before connection attempts, including invalid endpoint JSON, endpoint token-path mismatch, unsupported endpoint schema, and corrupted token files
- adds coverage that reacquiring a daemon runtime lock after a simulated crash replaces stale lock metadata with the current project id, process id, and Athanor version
- adds coverage that daemon runtime endpoint and token files are removed when the runtime file guard is dropped
- adds coverage that unsafe daemon serve options are rejected before binding, including polling without watch, invalid debounce, non-loopback TCP, and oversized protocol limits
- adds coverage that index and generation jobs finished with an `operation cancelled` error are recorded as cancelled and drop their cancellation tokens
- adds coverage that daemon shutdown cancellation requests active jobs, times out while they remain active, and drains successfully after cancellation is recorded
- adds coverage that daemon `status`, `explain`, `search`, `overview`, and `context` requests still complete while an index job is already running
- keeps the read-only daemon contention coverage deterministic on Windows by releasing cached search resources before temporary project cleanup
- adds platform coverage for local socket setup metadata: Unix stale socket cleanup or Windows named-pipe label sanitization where available
- deduplicates daemon watcher source paths after debounce delivery, filters `.athanor` artifact noise, covers event storms being skipped while an index job is already active, and adds a live polling-watcher debounce smoke test for source changes versus generated artifacts
- keeps the existing single-instance lock, busy response, authentication, protocol-v1 compatibility, cancellation state, staging cleanup, and oversized response tests intact

### Pipeline Shared Input Copy Reduction

Status: verified.

Implemented in:

- `crates/athanor-core/src/ports.rs`
- `crates/athanor-app/src/pipeline.rs`
- `docs/architecture/pipeline.md`
- `docs/development/roadmap-status.md`

Purpose:

- changes `LinkInput` and `CheckInput` full-context fields from `Arc<[T]>` to `Arc<Vec<T>>`
- moves canonicalized entity and fact vectors into shared `Arc<Vec<T>>` values once, before linker execution
- reuses the same entity and fact allocations for checker execution instead of rebuilding full-context slices
- moves the checker relation context into one shared `Arc<Vec<Relation>>` for all checker adapters
- preserves external process adapter JSON compatibility because `Arc<Vec<T>>` serializes to the same JSON arrays as the previous shared slice fields
- returns owned vectors for canonical storage through `Arc::try_unwrap` when no adapter retained a reference, falling back to a clone only when necessary
- adds a regression test proving linker and checker phases observe the same entity and fact shared-context allocations

### Analysis Coverage Report Slice

Status: verified.

Implemented in:

- `crates/athanor-app/src/coverage.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`
- `docs/development/roadmap-status.md`

Purpose:

- adds `ath coverage`, `ath coverage --json`, `ath coverage --adapter <id>`, and `ath coverage --file <path>`
- reads the latest canonical snapshot and `.athanor/state/index-state.json` without running indexing or reading generated JSONL artifacts
- emits stable `athanor.coverage.v1` with tracked-file counts, canonical object counts, adapter evidence/fact coverage, diagnostic-kind counts, file-level rows, applied filters, explicit row limits, and omitted counts
- keeps coverage reporting bounded and deterministic for CLI use and future daemon/MCP routing
- leaves canonical capability declarations, parser recovery diagnostics, unsupported syntax diagnostics, and `ath capabilities` for a later analysis-completeness slice

### JavaScript/TypeScript Adapter Initial Slice

Status: verified.

Implemented in:

- `crates/athanor-extractor-js-ts`
- `crates/athanor-core/src/ports.rs`
- `crates/athanor-app/src/pipeline.rs`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-source-fs/src/lib.rs`
- `docs/README.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`
- `docs/adapters/extractor-js-ts.md`
- `docs/development/roadmap-status.md`

Purpose:

- adds one built-in `athanor-extractor-js-ts` language adapter for mixed JavaScript and TypeScript projects
- supports `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, and `.cts` source files with per-file language hints preserved in emitted metadata
- parses JavaScript/JSX through `tree-sitter-javascript` and TypeScript/TSX through `tree-sitter-typescript`
- keeps parser-specific AST structures inside the adapter and emits backend-independent canonical module, function, class, symbol, package, dependency, fact, and diagnostic objects
- extracts module import/export payloads, functions, methods, classes, TypeScript interface/type declarations, and `package.json` package/dependency declarations
- extends extractor output so adapters can emit evidence-backed diagnostics directly when parser recovery or unsupported syntax is detected
- coalesces nested tree-sitter parse errors at the outer parser error node, strips leading UTF-8 BOMs, accepts Node shebang scripts, and avoids reporting ordinary top-level runtime statements or ambient module declarations as unsupported declaration diagnostics, reducing JS/TS noise on real Rustok frontend files
- registers the adapter through the app-layer runtime by default after focused adapter tests and documentation updates
- advances persisted index state to v32 so existing projects rebuild once and pick up JavaScript/TypeScript and Markdown diagnostic noise-reduction changes for unchanged files
- keeps React, Next.js, NestJS, Express, Vue, route inference, component semantics, and project conventions out of the base language adapter

Current limitations:

- exact relative import findings are materialized by the JS/TS import linker; package, alias,
  dynamic, CommonJS, export, and re-export relations remain deferred
- parser errors and unsupported declaration shapes are diagnostic-backed, but deeper capability reporting remains part of the Analysis Completeness Reporting backlog

### JavaScript/TypeScript Dual-Parser Verification Mode

Status: verified.

Implemented in:

- `crates/athanor-extractor-js-ts`
- `crates/athanor-runtime-defaults`
- `crates/athanor-app/src/index_state.rs`
- `apps/ath`
- `apps/athd`
- `docs/README.md`
- `docs/adapters/extractor-js-ts.md`
- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md`
- `docs/development/roadmap-status.md`

Purpose:

- adds opt-in `js-ts-precision` build features for `ath` and `athd`, leaving normal indexing on the
  existing single tree-sitter backend
- runs maintained Rust-native Oxc as a second parser for affected JS/TS source files and compares
  adapter-local normalized declarations, static imports, source-backed re-exports, ranges, and
  parser recovery state rather than raw ASTs
- retains tree-sitter as the canonical-output backend so agreed findings keep normal stable keys
  and ids; contradictory findings are never silently merged
- emits evidence-backed diagnostics for backend-only findings, source-range mismatches, and
  recovery differences
- records bounded `athanor.js_ts_precision.v1` module metrics with a per-file limit of 64 reported
  disagreements plus explicit omitted counts
- uses a precision-specific index-state schema so switching the build mode safely rebuilds
  unchanged sources once
- keeps `ath coverage` schema-stable while exposing precision diagnostic kinds and file counts
  through its existing bounded diagnostic summaries

Current limitations:

- precision comparison excludes methods and variable-bound functions until both parser backends
  expose an equally stable adapter-local representation
- precision mode parses every affected JS/TS file twice and is intended for high-value repositories
  or verification runs rather than the default large-repository path

### JavaScript/TypeScript Relative Import Linker

Status: verified.

Implemented in:

- `crates/athanor-linker-js-ts`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-app/src/index_state.rs`
- `docs/adapters/linker-js-ts.md`
- `docs/architecture/adapters.md`
- `docs/development/roadmap-status.md`

Purpose:

- adds the built-in `js-ts-imports` linker over normalized JS/TS module import payloads
- resolves only exact relative imports against existing canonical modules, including extensionless
  paths and directory `index` modules
- emits verified `imports` relations with import-line evidence and ownership from both source files
- re-emits a relation when either module is affected so new or changed targets can resolve imports
  from unchanged source modules during incremental indexing
- advances persisted index state to v33 so existing repositories rebuild once and receive JS/TS
  import relations
- keeps package, Node built-in, workspace alias, TypeScript path alias, dynamic import, CommonJS,
  export, re-export, symbol binding, and framework semantics outside this precise initial slice

## Next

This backlog tracks the remaining global plan from `start.md`. The entries below are prioritized by dependency order and current product value; each item should be moved into `Implemented` only after code, documentation, and required verification are complete.

### Daemon Fault-Injection Coverage

Status: planned.

Priority: P1.

Scope:

- extend daemon tests beyond existing token, v1 compatibility, busy response, and oversized response unit coverage
- cover daemon crash/restart behavior beyond the implemented stale endpoint metadata, stale token, stale lock recovery, and corrupted endpoint metadata client-side checks
- cover cancellation during real long-running index and generation job execution beyond the implemented daemon job-state cancellation result mapping
- cover deeper parallel read-only request contention while an index job is running, beyond the implemented status/search/explain/overview/context coverage
- cover live watcher debounce timing beyond the implemented source-path deduplication, artifact filtering, and active-index skip behavior
- cover Unix socket permissions and Windows named pipe lifecycle where platform support is available

Acceptance:

- daemon restart and repair behavior is deterministic after interrupted jobs or corrupted runtime metadata
- atomic publication guarantees remain intact under cancellation and process termination
- all dangerous protocol and lifecycle edge cases are represented by automated tests or explicitly documented as out of scope

### OpenAPI And GraphQL Contract Consistency

Status: planned.

Priority: P1.

Scope:

- add a community-facing GraphQL contract adapter alongside the existing OpenAPI adapter, covering SDL, introspection JSON, `.graphql`/`.gql` operations, embedded frontend operations where practical, types, fields, arguments, directives, deprecations, and validation diagnostics
- normalize OpenAPI and GraphQL findings into a shared API contract model for operations, types, fields, inputs, enums, examples, errors, authentication requirements, deprecations, and ownership without making Athanor core/domain depend on either protocol
- extend API linking and checking so REST operations, GraphQL queries/mutations/subscriptions, schema types, examples, documentation pages, frontend callsites, backend route handlers, resolver implementations, and generated clients can be compared through bounded commands
- report cross-protocol drift with evidence, including missing implementations, undocumented operations, invalid examples, incompatible type fields, enum differences, nullability or required-field mismatches, stale frontend operations, and deprecated contract usage
- add Rustok-specific API checks as an opt-in adapter layer that maps REST and GraphQL contracts onto FBA module ownership, Loco routes, resolver code, permissions, Page Builder consumers, dashboards, and published platform compatibility rules
- expose agent-facing commands such as `ath api consistency`, `ath api consistency --json`, `ath graph api operation <id>`, `ath graph api type <name>`, `ath check rustok-api`, and `ath rustok api audit` with explicit limits, stable schemas, canonical ids, and evidence links
- keep GraphQL support adapter-first and community-usable, while the Rustok adapter only adds platform interpretation such as module ownership, permission gates, compatibility windows, and product dashboards

Acceptance:

- OpenAPI and GraphQL receive equal first-class contract treatment without duplicating canonical API registry, evidence building, path normalization, or stable id logic
- community users can validate GraphQL contracts, operations, examples, documentation links, and implementation links without enabling Rustok-specific adapters
- Rustok users can detect REST/GraphQL drift for the same platform capability, including permission mismatches, FBA ownership violations, Page Builder dependencies on unstable operations, and breaking changes to published APIs
- every reported inconsistency includes source evidence and ownership, and every large comparison is available through bounded CLI, daemon, and future MCP outputs instead of requiring generated artifact inspection
- tests cover GraphQL SDL extraction, introspection extraction, operation validation, OpenAPI/GraphQL type drift, frontend stale-operation detection, missing resolver or route implementation, and Rustok permission or ownership mismatches

### Whole-Code Relationship Graph And Change Map

Status: planned.

Priority: P1.

Scope:

- provide a community-facing code relationship graph that connects contracts, source modules, symbols, call edges, imports, routes, resolvers, database models, migrations, generated clients, frontend callsites, tests, documentation, operations files, and diagnostics through canonical entities, facts, relations, ownership, and evidence
- add a bounded change-map workflow that answers "where must I inspect or edit for this change?" from a task prompt, explicit target stable key, or working-tree diff without requiring agents to read generated graph, JSONL, wiki, or HTML artifacts
- expose commands such as `ath change-map <task>`, `ath change-map --target <stable-key>`, `ath change-map --diff`, `ath change-map --json`, and daemon/MCP equivalents with explicit limits, relation-chain explanations, omitted counts, canonical ids, stable keys, file paths, and evidence anchors
- rank impacted items by relation type, direction, confidence, ownership, open diagnostics, test coverage, and proximity to changed files while keeping the result deterministic and explainable
- extend language and framework adapters over time so the graph includes deeper Rust, JavaScript/TypeScript, GraphQL, OpenAPI, SeaORM/database, frontend framework, test, CI, and operations relations without moving adapter-specific semantics into `athanor-domain` or `athanor-core`
- support community projects first through generic relation kinds and adapter contracts, then add Rustok-specific interpretation as an opt-in layer for FBA modules, FFA surfaces, Page Builder providers/consumers, Loco routes, permissions, published platform APIs, dashboards, and compatibility rules
- keep graph visualizations and exports disposable read models; normal agent workflows must use bounded graph, impact, context, and change-map commands instead of inspecting complete generated artifacts

Acceptance:

- community users can ask for a change map and receive a bounded, evidence-backed list of likely code, contract, docs, test, and operations locations to inspect or update
- Rustok users receive the same generic map plus platform-specific reasons such as FBA ownership, permission gates, Page Builder dependencies, route/resolver ownership, and compatibility risk
- every returned item includes why it was selected, the relation chain from the task/diff/target, source evidence, ownership, and stable canonical identifiers
- results degrade honestly when adapters only provide partial knowledge, including confidence, omitted counts, unsupported syntax or skipped-file diagnostics, and links to capability/completeness reports
- tests cover task-targeted change maps, diff-based change maps, relation-chain explanations, deterministic ranking, omitted-limit reporting, missing-test surfacing, and Rustok-specific platform annotations

### Analysis Completeness Reporting

Status: planned.

Priority: P2.

Scope:

- add canonical coverage/capability facts or diagnostics with evidence for unsupported syntax, skipped files, partial parsing, parser recovery, and extractor confidence
- expose bounded commands such as `ath capabilities`, `ath capabilities --json`, and deeper `ath coverage` variants for fully processed, partially processed, skipped, and unsupported constructs
- report per-adapter discovered files, fully processed files, partially processed files, skipped files, unsupported constructs, and omitted counts when limits apply
- keep coverage output agent-facing through stable schemas and explicit limits instead of requiring generated artifact reads

Acceptance:

- users can see where the knowledge graph is incomplete before relying on query, graph, or daemon answers
- every incompleteness claim has source evidence or a documented adapter-level capability declaration
- coverage reports remain bounded, deterministic, and suitable for CLI, daemon, and future MCP use

### Release Readiness Gate

Status: planned.

Priority: P2.

Scope:

- define one release-gate checklist that combines formatting, tests, clippy, indexing smoke, docs checks, repair inspection, daemon doctor, binary smoke tests, archive checksum verification, security audit, signing, and provenance checks
- document rollback expectations for broken binaries, broken generated-output pointers, and daemon runtime metadata
- ensure the release gate references existing CI, production, security, and release workflows instead of duplicating their implementation details

Acceptance:

- release readiness can be evaluated from one documented checklist
- every release artifact has checksums and provenance, and every local generated/canonical artifact can be inspected or repaired through bounded commands

### Affected Diagnostic Check

Status: verified.

Implemented in:

- `crates/athanor-app/src/check.rs`
- `apps/ath/src/main.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath check affected` and `ath check affected --json`
- compares current source discovery with `.athanor/state/index-state.json`
- reports open diagnostics from the latest canonical snapshot that touch changed or removed files through attached entities, ownership, or evidence
- reports editable documentation drift only for affected documents whose `last_verified_snapshot` differs from the latest canonical snapshot
- reports stale or potentially stale local artifacts for coordinated generated generations, direct wiki output, direct HTML report output, API contract latest pointers, and API diff directories
- emits stable `athanor.affected_check.v1` JSON with affected file counts, affected documentation drift, stale artifact statuses, and diagnostics
- keeps the command read-only and does not commit a new index snapshot, patch documentation, regenerate outputs, run repair apply, or delete artifacts

### Diff-Based Context Packs

Status: verified.

Implemented in:

- `crates/athanor-app/src/context.rs`
- `apps/ath/src/main.rs`
- `crates/athanor-transport-mcp/src/lib.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath context --diff` and `ath context --diff --json`
- compares current source discovery with `.athanor/state/index-state.json`
- selects canonical entities from changed or removed files as direct context roots
- expands related entities using the existing context-pack limits and relation depth
- emits the normal `athanor.context_pack.v1` payload plus diff file counts
- keeps the command read-only and does not commit a new index snapshot

### Fast Changed-File Extractor Preflight

Status: verified.

Implemented in:

- `apps/ath/src/main.rs`
- `crates/athanor-app/src/lib.rs`
- `crates/athanor-app/src/pipeline.rs`
- `crates/athanor-app/src/runtime.rs`
- `crates/athanor-app/src/validate_changed.rs`
- `crates/athanor-extractor-markdown/src/lib.rs`
- `crates/athanor-source-fs/src/lib.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`
- `docs/development/agent-workflow.md`
- `docs/development/roadmap-status.md`

Purpose:

- adds `ath validate-changed --path <path>`, `ath validate-changed --path <path> --json`, and explicit `--file <path>` narrowing
- selects changed and untracked paths from Git status, with an index-state fallback for non-Git roots
- reads only selected changed files instead of performing full repository source discovery
- runs matching extractors and canonical metadata validation without linkers, checkers, storage, index-state updates, or JSONL read-model writes
- returns `athanor.changed_validation.v1` with changed/removed counts, extractor diagnostics, and extraction metrics
- verified on `D:\RusTok`: three changed frontend files completed with no diagnostics and `source_discovery_ms = 0`, `extraction_ms = 128`, compared with roughly 166 seconds for a writable one-file incremental index
- optimized markdown line lookup after real preflight measurements on Athanor docs: `docs/development/roadmap-status.md` markdown extraction dropped from about 17.8 seconds to 36 ms, and `docs/architecture/pipeline.md` dropped from about 4.1 seconds to 10 ms

Current limitations:

- this is a parser/extractor preflight, not full graph validation
- deleted files are counted but cannot produce extractor diagnostics
- full canonical consistency still requires `ath index`, `ath update --changed`, and scoped `ath check ...` commands

### Changed-File Update Command

Status: verified.

Implemented in:

- `apps/ath/src/main.rs`
- `crates/athanor-app/src/index.rs`

Purpose:

- adds `ath update --changed` and `ath update --changed --json`
- exposes the existing incremental indexing path as an explicit update command
- writes a new durable canonical snapshot and JSONL read model
- updates `.athanor/state/index-state.json` after successful output writes
- reports changed, unchanged, and removed file counts through the existing index report

### Repair Inspection

Status: verified.

Implemented in:

- `apps/ath/src/main.rs`
- `crates/athanor-app/src/repair.rs`

Purpose:

- adds `ath repair inspect` and `ath repair inspect --json`
- validates local canonical latest pointers and snapshot manifests without modifying files
- validates generated current pointers and generation manifests without modifying files
- reports orphan canonical snapshots and generated generations for future cleanup
- reports stale generated outputs when `.athanor/generated/current.json` points to a snapshot older than the latest canonical snapshot

### Repair Cleanup

Status: verified.

Implemented in:

- `apps/ath/src/main.rs`
- `crates/athanor-app/src/check.rs`
- `crates/athanor-app/src/repair.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath repair cleanup`, `ath repair cleanup --dry-run`, and `ath repair cleanup --json`
- adds `ath repair cleanup --generated-only` for stale generated generation cleanup without removing orphan canonical snapshots
- removes orphan canonical snapshot directories not selected by `.athanor/store/canonical/jsonl/latest.json`
- removes orphan generated generation directories not selected by `.athanor/generated/current.json`
- supports `--keep-canonical <N>` and `--keep-generated <N>` to retain the newest N orphan artifacts of each kind
- reports the initial repair inspection, removed or planned removals, and remaining issues as `athanor.repair_cleanup.v1`
- refuses to remove paths outside the known canonical snapshot and generated generation roots
- keeps pointer rewriting, stale generation republishing, and current artifact removal deferred

### Repair Regenerate

Status: verified.

Implemented in:

- `apps/ath/src/main.rs`
- `crates/athanor-app/src/repair.rs`
- `crates/athanor-app/src/generation.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath repair regenerate`, `ath repair regenerate --dry-run`, and `ath repair regenerate --json`
- detects stale, missing, or invalid generated-current pointers from the repair inspection report
- detects corrupted current generated generation state, including invalid pointer paths, missing current manifests, manifest schema mismatches, manifest generation id mismatches, and manifest snapshot mismatches
- reuses the coordinated `generate_project` path to publish JSONL, wiki, and HTML from the latest canonical snapshot
- updates `.athanor/generated/current.json` only after the replacement generation is fully published
- reports the initial inspection, whether regeneration was needed, the new generation when created, and remaining issues as `athanor.repair_regenerate.v1`
- leaves old generated generation directories for explicit `ath repair cleanup`

### Repair Canonical Pointer Recovery

Status: verified.

Implemented in:

- `apps/ath/src/main.rs`
- `crates/athanor-app/src/repair.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath repair recover-canonical`, `ath repair recover-canonical --dry-run`, and `ath repair recover-canonical --json`
- detects missing, invalid, or dangling `.athanor/store/canonical/jsonl/latest.json` pointers from the repair inspection report
- selects the newest local canonical snapshot with a supported manifest schema and matching snapshot id
- atomically rewrites only `latest.json` when recovery is needed and not a dry run
- reports the initial inspection, selected snapshot, recovered snapshot, and remaining issues as `athanor.repair_recover_canonical.v1`
- leaves canonical snapshot contents and cleanup policy unchanged

### Repair Apply

Status: verified.

Implemented in:

- `apps/ath/src/main.rs`
- `crates/athanor-app/src/repair.rs`
- `docs/README.md`
- `docs/architecture/pipeline.md`

Purpose:

- adds `ath repair apply`, `ath repair apply --dry-run`, and `ath repair apply --json`
- runs canonical latest-pointer recovery, generated-current regeneration, and orphan artifact cleanup in deterministic order
- reports all stage outputs and final remaining issues as `athanor.repair_apply.v1`
- keeps `--dry-run` read-only across every stage
- passes `--generated-only` to the cleanup stage when canonical snapshot cleanup should be skipped
- passes `--keep-canonical <N>` and `--keep-generated <N>` retention settings to the cleanup stage
- delegates artifact deletion to the same root-checked cleanup rules as `ath repair cleanup`

### Phase 6 Remainder - Affected Workflow And Repair

Status: verified.

Scope:

- completed affected workflow artifact and documentation drift reporting
- completed generated-current repair handling for stale pointers, missing pointers, invalid pointers, invalid pointer paths, missing current generation directories, missing current manifests, and current manifest mismatches

Acceptance:

- changed-file workflows avoid full recomputation where safe
- diff-based context and impact commands work before a new durable index is committed
- repair cleanup, generated-output regeneration, canonical latest-pointer recovery, and full repair apply are deterministic and documented

### Phase 6.5 - Agent Graph Navigation And Overview

Status: verified.

Scope:

- extend the initial repository overview query beyond the implemented module structure and integration-boundary summaries as new canonical language relations become available
- extend graph export beyond the implemented `ath graph export --format json` with GraphML-compatible output, generated from canonical snapshots rather than replacing canonical storage (initial GraphML output implemented)
- extend the HTML report beyond the implemented bounded interactive SVG graph, compact summary, per-entity detail pages, and filters only when additional graph controls provide clear inspection value
- extend graph navigation beyond implemented related-entity exploration, shortest path, degree-centrality hubs, directed PageRank, and directed cycle detection with optional further centrality algorithms over canonical relations
- improve `ath impact` with explanatory relation paths and an optional future precision mode for deeper call/data-flow analysis once language adapters can support it (initial path-step explanations implemented)
- use the implemented explicit multi-repository registry as the routing foundation for future daemon and MCP use, so one server cannot accidentally answer from the wrong repository
- treat ideas from GitNexus, Graphify, code-review-graph, and similar code-graph tools as product patterns to adapt, not storage or source-of-truth replacements

Acceptance:

- every graph query result can be traced back to canonical entity, relation, diagnostic, and evidence ids
- exported graph files and interactive reports are disposable read models that can be regenerated from the latest canonical snapshot
- overview and graph-navigation outputs are bounded, deterministic, and suitable for agent context
- no normal agent workflow depends on reading complete generated JSONL, wiki, HTML, graph, API, search, or vector artifacts
- multi-repository support keeps repository identity explicit in CLI, daemon, MCP, and generated artifacts
- documentation explains the boundary between canonical knowledge, graph algorithms, and generated graph views

### Phase 8 - I18n And Concepts

Status: planned.

Scope:

- implement concept mapping, glossary/alias handling, language detection, and cross-language context selection
- add `translation_of` and translation drift relations and diagnostics
- expose commands such as `ath concept map`, `ath docs i18n check`, and `ath docs propose-translation`

Acceptance:

- multilingual documentation can share canonical knowledge without duplicating source of truth
- translation drift is reported as diagnostics with evidence
- generated translation proposals are patch-based drafts

### Phase 9 Remainder - Semantic Search And Vectors

Status: planned.

Scope:

- add embedding provider and vector index adapters after benchmark and offline requirements are defined
- add local semantic retrieval and hybrid lexical/vector search over canonical read models
- keep vectors disposable and rebuildable from canonical snapshots

Acceptance:

- vector search results include evidence and source canonical object ids
- semantic retrieval is optional and does not change canonical truth
- resource limits and secret-handling rules cover embedding workflows

### Phase 10 - Rustok Adapter

Status: planned.

Scope:

- add Postgres/SeaORM storage and read-model adapters for Rustok mode
- add Rustok/Loco routes, permission integration, compatible errors, and dashboards
- support API registry, diagnostics, docs drift, breaking changes, invalid examples, and translation drift views in Rustok

Acceptance:

- standalone builds do not require Postgres/SeaORM
- Rustok-mode builds do not require SurrealDB
- Postgres/JSONB read models remain projections of canonical Athanor knowledge

### Phase 11 Remainder - Community Modules Foundation

Status: planned.

Scope:

- complete module manifest, module registry, CLI module management, permission model, compatibility matrix, extension SDK docs, and adapter contract tests
- expose commands such as `ath module list` and `ath module enable <module>`
- keep community modules independent from a Rustok marketplace

Acceptance:

- modules can be discovered, validated, enabled, and disabled without changing core/domain contracts
- adapter contract tests are reusable by external module authors
- compatibility and security constraints are documented

### Phase 12 - Advanced Language And Framework Support

Status: planned.

Scope:

- deepen Python, Go, PHP, Java, C#, and C/C++ support through adapters
- extend JavaScript/TypeScript beyond the initial promoted adapter slices with deeper semantic, framework, and optional external-index support
- add framework adapters and optional LSP/SCIP/LSIF import/export paths
- preserve adapter-first boundaries for every language and framework integration

Acceptance:

- each language/framework slice ships as an isolated adapter with focused tests and docs
- imported external indexes remain read-model inputs, not replacements for canonical contracts
- new adapters satisfy evidence, ownership, and validation requirements

## Verification Commands

Run before marking implementation work as verified:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
```
