---
id: doc://docs/development/roadmap-status.md
kind: developer_guide
language: en
source_language: en
last_verified_snapshot: snap_jsonl_00000030
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
- `athanor-extractor-rust`
- `athanor-linker-api`
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
ath index --validate-only
ath index --validate-only --validation-result <path>
ath context <task>
ath context <task> --json
ath explain <stable-key>
ath explain <stable-key> --json
ath check api
ath check api --json
ath check docs
ath check docs --json
ath docs check
ath docs check --json
ath wiki
ath wiki --output <directory>
ath report html
ath report html --output <directory>
ath generate
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
  RustExtractor

linkers:
  MarkdownContainmentLinker
  ApiKnowledgeLinker

checkers:
  MarkdownStructureChecker
  ApiConsistencyChecker
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
- advances persisted index state to v6 so existing projects build schema relations and diagnostics once

## In Progress

None.

## Next

This backlog contains prioritized initiatives based on recent project research and technical debt analysis.

### P0: Quality & Hygiene Baseline (Next Up)

1. **Metadata & Configuration Hygiene**
   - **Goal**: Fix package repository URLs (upstream drift) and keep `README.md` status descriptions synchronized.
   - **Why**: Clean up workspace metadata and align expectations for team members and AI agents.

### P1: Engine Extensions & Core Features

1. **Rust Relation Graph Slice**
   - **Goal**: Add imports, static call graphs, module containment, and `tested_by` relations on top of the syn-based extractor.
   - **Why**: Critical for high-precision impact analysis, diagnostics, and AI context packs.

2. **Lexical Search Read-Model**
   - **Goal**: Implement `SearchIndex` via Tantivy as a disposable read-model and add the `ath search` CLI command.
   - **Why**: Seed AI context packs with lexical lookup before expanding relations, bypassing store pollution.

3. **Code Impact Analysis**
   - **Goal**: Implement `ath impact <stable-key | path>` query command.
   - **Why**: Traverse ownership, dependency graphs, and diagnostics to calculate direct and transitive blast radius.

4. **Large-Repository Scale & Performance**
   - **Goal**: Introduce secondary indexes (path -> object IDs, stable-key -> entity ID) and chunked JSONL loading.
   - **Why**: Eliminate the memory and IO bottleneck of loading full snapshots on huge repos.

### P2: Transports & Operations

1. **Agent Transport Layer**
   - **Goal**: Stabilize machine-readable JSON output contracts and implement socket/HTTP (Axum) or MCP server adapters.
   - **Why**: Allow external orchestration agents to consume Athanor queries easily.

2. **Security & Supply-Chain Automation**
   - **Goal**: Integrate `cargo-deny`, `cargo-audit` checks, and nightly vulnerability scan workflows.
   - **Why**: Mitigate supply-chain risks and ensure attestation readiness.

## Verification Commands

Run before marking implementation work as verified:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
```
