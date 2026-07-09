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

Initial crate structure with 21 crates and the `ath` CLI binary.

### CLI

Status: verified.

Full command reference via `ath --help` and `athd --help`. Command groups: indexing, queries, checks, docs, API, generation, graph, Rustok, projects, repair, and daemon.

### Indexing Vertical Slice

Status: verified.

Core extraction, linking, checking, JSONL store, and read-model export pipeline. Verified with `cargo run -p ath --quiet -- index .`.

### IndexPipeline

Status: verified.

Orchestrates ordered source/extractor/linker/checker execution and writes canonical objects through `KnowledgeStore`. (`crates/athanor-app/src/pipeline.rs`)

### AdapterRegistry And RuntimeBuilder

Status: verified.

Assembles built-in adapters and keeps adapter ordering out of CLI code. Registry documented in `docs/architecture/adapters.md`. (`crates/athanor-app/src/runtime.rs`)

### JSONL Read Model Writer

Status: verified.

Writes JSONL read-model files and `manifest.json` generation outside CLI indexing. (`crates/athanor-app/src/read_model.rs`)

### Affected-Subset Linker And Checker Inputs

Status: verified.

Introduces `AffectedSubset` as a core input contract, passing affected entities and newly produced relations to linkers and checkers. (`crates/athanor-core/src/ports.rs`, `crates/athanor-linker-markdown/src/lib.rs`, `crates/athanor-checker-markdown/src/lib.rs`)

### Persisted File Change State

Status: verified.

Persists file paths, content hashes, language hints, and snapshot id in `.athanor/state/index-state.json`. Computes changed, unchanged, and removed file sets for incremental indexing.

### Incremental Extraction And Canonical Object Merge

Status: verified.

Loads previous canonical snapshot, extracts changed files only, reuses snapshot id when no changes, carries unchanged objects, and drops owned objects from changed or removed paths before rebuilding affected downstream outputs.

### JSONL Canonical Store Adapter

Status: verified.

Persists canonical entities, facts, relations, and diagnostics to `.athanor/store/canonical/jsonl`. Writes one JSONL snapshot directory per committed snapshot and `latest.json` for discovery. Documented in `docs/adapters/store-jsonl.md`.

### Canonical Object Ownership Metadata

Status: verified.

Adds `Ownership` metadata to entities, facts, relations, and diagnostics. Marks extractor output as owned by its source file and Markdown containment relations with union ownership. Used as the primary incremental merge pruning contract.

### Adapter Output Validation

Status: verified.

Validates newly emitted entities, facts, relations, and diagnostics for evidence and ownership. Fails indexing with adapter-specific error messages when required metadata is missing. Documented in `docs/architecture/adapters.md` and `docs/architecture/pipeline.md`.

### Aggregated Adapter Validation Reporting

Status: verified.

Introduces `AdapterValidationReport` and `AdapterValidationIssue` with adapter name, object type, object id, and missing metadata field. Aggregates multiple validation issues before returning an error.

### Adapter Validation Artifact

Status: verified.

Adds `ath index --validation-report <path>` for machine-readable adapter validation reports as JSON when indexing fails validation. Defaults to `.athanor/generated/current/validation-report.json`.

### Adapter Validation-Only Mode

Status: verified.

Adds `ath index --validate-only` to run extraction, linking, checking, and validation using a transient memory store without writing durable canonical snapshots or generated read models. Supports `--validation-result <path>`.

### Adapter Plugin Manifest Discovery

Status: verified.

Discovers adapter plugin manifests from `.athanor/adapters/*.json` and `.athanor/plugins/*/athanor-adapter.json`. Supports external process adapters via `command` entries. Full manifest schema documented in `docs/architecture/adapters.md`.

### RusTok FFA Adapter And Graph Extension

Status: verified.

Extracts FFA source markers and documentation status, emits canonical FFA surface and layer entities. Agent-facing read models via `ath rustok ffa audit`, `ath graph ffa surface`, `ath graph ffa violations`, `ath check rustok-ffa`. Documented in `docs/adapters/rustok-ffa.md`.

### RusTok FBA Adapter And Graph Extension

Status: verified.

Extracts FBA registry, port-code, and central-board markers, emits canonical FBA module, contract, port, operation, profile, and dependency entities. Agent-facing read models via `ath rustok fba audit`, `ath graph fba module|port|dependencies|violations`, `ath check rustok-fba`. Documented in `docs/adapters/rustok-fba.md`.

### RusTok Page Builder Adapter And Graph Extension

Status: verified.

Extracts Page Builder provider registry, adapter seam, wave evidence, consumer manifest, content-format, and FSD surface markers. Agent-facing read models via `ath rustok page-builder audit`, `ath graph page-builder provider|consumer|violations`, `ath check rustok-page-builder`. Documented in `docs/adapters/rustok-page-builder.md`.

### External Process Extractors, Linkers, And Checkers

Status: verified.

Lets manifest entries load source, extractor, linker, and checker adapters from external commands with stdin/stdout JSON. Full process adapter protocol documented in `docs/architecture/adapters.md`.

### Task-Focused Context Packs

Status: verified.

Adds `ath context <task>` and `ath context <task> --json`. Loads the latest durable canonical snapshot, selects direct lexical entity matches and expands by one relation hop, includes files and diagnostics, emits the canonical `ContextPack` model. (`crates/athanor-app/src/context.rs`, `apps/ath/src/main.rs`, `docs/architecture/pipeline.md`)

### Repository Overview Query

Status: verified.

Adds `ath overview [path]` and `ath overview [path] --json`. Reports canonical object totals, top entity and relation kinds, top source roots, coverage counters, graph hubs by relation degree, and diagnostic summaries. Emits the stable `athanor.overview.v1` JSON payload. (`crates/athanor-app/src/overview.rs`, `apps/ath/src/main.rs`)

### Explicit Context Limits And Levels

Status: verified.

Adds `summary`, `normal`, `deep`, and `full` context presets with `--budget`, `--max-files`, `--max-entities`, `--max-diagnostics`, and `--max-depth` overrides. Records effective limits, approximate token usage, and omitted counts. (`crates/athanor-app/src/context.rs`, `apps/ath/src/main.rs`)

### Canonical Entity Explanation

Status: verified.

Adds `ath explain <stable-key>` and `ath explain <stable-key> --json`. Resolves an entity by exact stable key, includes facts, separates incoming and outgoing relations, resolves neighboring entities, includes diagnostics, and exposes full evidence, ownership, confidence, and status. (`crates/athanor-app/src/explain.rs`, `apps/ath/src/main.rs`)

### Scoped Diagnostic Check Views

Status: verified.

Adds `ath check api` and `ath check docs` with optional `--json` output. Reads the latest durable canonical snapshot, classifies API and documentation diagnostic kinds, returns open diagnostics sorted by severity, reports total and per-severity counts. (`crates/athanor-app/src/check.rs`, `apps/ath/src/main.rs`)

### Editable Documentation Completeness Policy And Gate

Status: verified.

Adds `ath docs check` and `ath docs check --json`. Reads project policy from `[docs.completeness]` in `athanor.toml`, gates editable Markdown pages under `docs.editable_path`, verifies explicitly declared required frontmatter fields and allowed statuses, optionally requires `last_verified_snapshot` match, includes open diagnostics at or above a configurable severity threshold. Documented in `docs/development/docs-completeness-policy.md`.

### Editable Documentation Drift Report

Status: verified.

Adds `ath docs drift` and `ath docs drift --json`. Distinguishes current pages from pages with missing or stale `last_verified_snapshot` metadata. Emits the stable `athanor.docs_drift.v1` JSON report. (`crates/athanor-app/src/docs.rs`, `apps/ath/src/main.rs`)

### Automated CI/CD Baseline

Status: implemented; local CI contract verified, first hosted matrix run pending.

Runs on pushes to `main`, pull requests, and manual dispatches. Tests Rust 1.95 on Linux, Windows, and macOS. Enforces formatting, workspace tests, Clippy warnings, indexing smoke tests, and documentation completeness gate. Documented in `docs/development/ci.md`.

### Workspace Metadata And Status Hygiene

Status: verified.

Confirms canonical repository URL, keeps inherited workspace metadata, adds shared author, description, homepage, and documentation metadata to all 18 packages, marks internal workspace packages as non-publishable.

### Markdown Wiki Projector

Status: verified.

Adds `ath wiki [path]` with an optional `--output` directory. Loads the latest durable canonical snapshot, writes a neutral Markdown index plus entity and open-diagnostic pages with YAML frontmatter, source locations, facts, relations, evidence, and diagnostics. Emits a versioned manifest and replaces the previous wiki atomically. Documented in `docs/adapters/projector-wiki.md`.

### HTML Report Projector

Status: verified.

Adds `ath report html [path]` with an optional `--output` directory. Writes a self-contained static report with snapshot metrics, open diagnostic details, and a deterministic canonical entity table. Extracts shared mechanics into `athanor-projector-support`. Documented in `docs/adapters/projector-html.md`.

### Coordinated Immutable Generated Generations

Status: verified.

Adds `ath generate [path]` to project JSONL, wiki, and HTML into one immutable generation under `.athanor/generated/generations/<generation>`. Updates `current.json` atomically. Idempotent when current pointer targets latest snapshot. (`crates/athanor-projector-support/src/lib.rs`, `crates/athanor-app/src/generation.rs`)

### Library Adoption Plan

Status: verified.

Records retained, approved, conditional, and deferred third-party dependencies. Selects `pulldown-cmark`, `oas3`, `jsonschema`, `notify`, and Tantivy for their relevant phases. Defines adapter boundaries and contract-test criteria for every adoption. Documented in `docs/development/library-adoption-plan.md`.

### Pulldown-Cmark Markdown Extraction

Status: verified.

Replaces the line-based Markdown heading scanner with `pulldown-cmark` 0.13.4. Parses ATX and setext headings from CommonMark events, ignores heading syntax inside fenced code blocks, normalizes inline formatting, maps parser byte offsets to deterministic source evidence lines. Documented in `docs/adapters/extractor-markdown.md`.

### Markdown Documentation Frontmatter

Status: verified.

Parses optional leading YAML frontmatter through `serde_yaml_ng`. Supports explicit `doc://` page identity stable across source path moves, applies explicit language, classifies documentation as `editable` or `generated`, records kind, source language, concepts, entity references, verification snapshot, and status. Documented in `docs/adapters/extractor-markdown.md`.

### Markdown Frontmatter Reference Linking And Diagnostics

Status: verified.

Adds canonical `documentation_reference_unresolved` and `duplicate_documentation_id` diagnostic kinds. Resolves exact stable keys declared in Markdown `entities` and `concepts` frontmatter lists, emits verified generic `documents` relations, diagnoses unresolved references and ambiguous duplicates. Includes both diagnostic kinds in `ath check docs`.

### Replaceable OpenAPI Parser Backends

Status: verified.

Adds a private `OpenApiDocumentParser` boundary and `NormalizedOpenApiDocument`. Dispatches OpenAPI 3.1.x documents to `oas3` 0.22.0, keeps OpenAPI 3.0.x support through a replaceable legacy parser, replaces unmaintained `serde_yaml` with `serde_yaml_ng`. Records selected parser backend in canonical payloads. Documented in `docs/adapters/extractor-openapi.md`.

### Rust Code Extraction Slice

Status: verified.

Adds the built-in `builtin.extractor.rust` adapter. Parses Rust source with `syn`, emits canonical module, function, and symbol entities, `symbol_defined` facts, stable `symbol://rust:` keys, parser-derived line evidence, and single-file ownership. Documented in `docs/adapters/extractor-rust.md`.

### OpenAPI Extraction Slice

Status: verified.

Adds the built-in `builtin.extractor.openapi` adapter. Parses OpenAPI 3.x YAML and JSON, emits canonical API endpoint and component-schema entities, route and schema declaration facts, operation metadata including methods, paths, tags, parameters, responses, and security. Documented in `docs/adapters/extractor-openapi.md`.

### API Knowledge Linker Slice

Status: verified.

Adds the built-in `builtin.linker.api_knowledge` adapter. Links OpenAPI operation ids to matching Rust function and method names, links Markdown pages and sections to API operations. Emits inferred `implemented_by`, `documents_operation`, and `documents_api` relations with combined evidence and ownership. Documented in `docs/adapters/linker-api.md`.

### API Consistency Checker Slice

Status: verified.

Adds the built-in `builtin.checker.api_consistency` adapter. Reports OpenAPI endpoints without linked Rust implementations and implemented endpoints without linked Markdown documentation. Reevaluates endpoints when relevant functions, documents, or API relations change. Documented in `docs/adapters/checker-api.md`.

### OpenAPI Request/Response Schema Slice

Status: verified.

Records request and response schema `$ref` uses with media type and response status metadata. Emits verified `schema_for_request` and `schema_for_response` relations for same-document component schemas. Reports `api_request_schema_mismatch` and `api_response_schema_mismatch` diagnostics.

### OpenAPI Example Extraction And Validation

Status: verified.

Materializes media-type `example` and `examples.*.value` entries as canonical `ApiExample` entities. Validates examples with `jsonschema` 0.46.5 (Draft 4 for OpenAPI 3.0, Draft 2020-12 for OpenAPI 3.1). Reports `api_example_invalid` diagnostics with evidence and ownership.

### Immutable API Contract Snapshots And Diff

Status: verified.

Adds `ath api snapshot` and `ath api diff` with JSON output modes. Publishes stable-key-sorted endpoint, schema, and example contracts under `.athanor/api/snapshots`. Classifies removed endpoints, auth/security changes, and schema-reference changes as breaking. Adds `ath api breaking-changes` as a non-zero-exit CI gate.

### API Contract Artifact Cleanup

Status: verified.

Adds `ath api cleanup`, `ath api cleanup --dry-run`, and `ath api cleanup --json`. Applies explicit retention to `.athanor/api/snapshots` and `.athanor/api/diffs`. Defaults to retaining the latest two API contract snapshots and two diff artifacts. Always retains the latest snapshot selected by `.athanor/api/latest.json`.

### API Retention Automation

Status: verified.

Adds `[api.retention]` configuration with `auto_cleanup`, `keep_snapshots`, and `keep_diffs`. Runs API cleanup automatically after successful `ath api snapshot` and `ath api diff` when enabled. Adds per-command `--cleanup`, `--no-cleanup`, `--keep-snapshots`, and `--keep-diffs` overrides. Strict API checks force retention cleanup off.

### Evidence-Backed API Breaking Diagnostics And Strict Gate

Status: verified.

Advances API contract snapshots and diffs to v2 with entity identity, source, and ownership. Emits `api_breaking_change_detected` domain diagnostics for every breaking diff entry. Adds `ath check api --strict` to combine current open API diagnostics with contract breaking changes.

### Rust Relation Graph Slice

Status: verified.

Adds the built-in `builtin.linker.rust` adapter. Walks function block expressions to find path and method calls, extracts `use` tree paths, detects `#[test]` attributes, connects parent modules to child entities via `Contains` relations, resolves relative imports dynamically. Documented in `docs/adapters/linker-rust.md`.

### Lexical Search Read-Model

Status: verified.

Implements `SearchIndex` port via Tantivy 0.26.1. Adds `ath search <query>` subcommand. Integrates into context-pack selection. Emits bounded `athanor.search.v1` reports. Documented in `docs/adapters/search-tantivy.md`.

### Code Impact Analysis

Status: verified.

Adds `ath impact <target>` to analyze the direct and transitive blast radius of changes. Supports `--diff` mode for unindexed working-tree changes. Traverses dependency, call, containment, and test relations via BFS. Gathers open diagnostics in the blast radius. Documented in `docs/adapters/impact.md`.

### Agent Transport Layer

Status: verified.

Implements the standard stdio JSON-RPC Model Context Protocol (MCP) server. Exposes Athanor query tools (`search`, `explain`, `context`, `impact`, `check`, `index`) as MCP tools. Redirects all logging and debug outputs to stderr. Documented in `docs/adapters/transport-mcp.md`.

### Large-Repository Scale And Performance

Status: verified.

Generates and writes two secondary index files on snapshot commit: `path_index.json` and `stable_key_index.json`. Optimizes memory usage by parsing JSONL line-by-line using a reusable line buffer. Documented in `docs/adapters/store-jsonl.md`.

### Extraction Parallelization, Shared Downstream Inputs, And Tracing

Status: verified.

Runs extractor and source-file tasks with up to 16 concurrent in-flight tasks. Shares full-context entity, fact, and relation lists through `Arc<Vec<T>>` values. Initializes tracing output and emits structured debug diagnostics for indexing phases.

### Security And Supply-Chain Automation

Status: verified.

Integrates dependency security and license compliance checks using `cargo-deny` in the main CI workflow. Configures nightly vulnerability scans using `cargo-audit`. Enforces dependency license compliance through `deny.toml`. Documents all supply-chain security checks. Documented in `docs/development/ci.md`.

### API Registry And Source-Of-Truth Policy

Status: verified.

Adds `ath api registry` and `ath api registry --json` to list API endpoints, their linked handlers, and their documentation pages. Parses `source_of_truth` policy configurations from `athanor.toml`. Dynamically filters diagnostic findings on API checks. (`crates/athanor-app/src/api_registry.rs`, `apps/ath/src/main.rs`)

### Documentation Frontmatter Patch Workflow

Status: verified.

Adds `ath docs propose-fix` and `ath docs apply-patch <patch-id-or-path>`. Writes reviewable `athanor.docs_patch.v1` JSON proposals under `.athanor/patches/docs/`. Proposes deterministic frontmatter fixes for completeness and drift findings, Markdown API documentation pages for implemented endpoints lacking documentation, and skeletal operations pages for undocumented environment variables. Refuses to overwrite existing files. Documented in `docs/development/docs-completeness-policy.md`.

### API Documentation Draft Enrichment

Status: verified.

Enriches `ath docs propose-fix` API documentation create drafts from the canonical API graph. Includes endpoint method, path, operation id, tags, declared response codes, security payloads, linked Rust handler source, linked request/response schemas, and linked examples. Preserves diagnostic evidence and review-before-apply semantics. Documented in `docs/development/docs-completeness-policy.md`.

### Existing API Documentation Patch Updates

Status: verified.

Extends `ath docs propose-fix` to update existing API documentation pages. Proposes managed API contract blocks delimited by `athanor:api-doc` comments. Refreshes managed blocks when endpoint facts change, supports multiple endpoints per page. Documented in `docs/development/docs-completeness-policy.md`.

### Stale API Narrative Rewrite Drafts

Status: verified.

Extends `ath docs propose-fix` stale API narrative handling beyond route review lists. Includes reviewable original-line and draft-line suggestions in generated narrative review blocks. Proposes deterministic route replacements only when an editable API page has exactly one linked current endpoint. Documented in `docs/development/docs-completeness-policy.md`.

### Environment Documentation Check View

Status: verified.

Extracts Rust environment-variable usage as canonical `EnvVar` entities and `env_var_used` facts. Uses runtime configuration `Feature` entities as environment-scope documentation targets. Checks whether environment variables and runtime configuration keys are linked from editable Markdown documentation. Exposes through `ath check env` and `ath check env --json`. Integrates with `ath docs propose-fix` for operations documentation drafts. Documented in `docs/adapters/extractor-rust.md`, `docs/adapters/extractor-operations.md`, and `docs/adapters/checker-api.md`.

### Operations File Extraction

Status: verified.

Adds the built-in `builtin.extractor.operations` adapter. Parses dotenv-style files such as `.env.example`, `.env`, and `*.env`. Emits canonical `EnvVar` entities from `KEY=value` and `export KEY=value` declarations. Parses Makefile targets as `ScriptCommand` entities, Dockerfile stages as `DockerService` entities, and Dockerfile instructions as `ScriptCommand` entities. Documented in `docs/adapters/extractor-operations.md`.

### Shell Script Operational Extraction

Status: verified.

Extends the operations adapter to `*.sh`, `*.bash`, and `*.zsh`. Parses `export KEY=value` and `readonly KEY=value` as redacted `EnvVar` knowledge. Parses shell function definitions as `ScriptCommand` entities. Emits evidence-backed `env_var_used` and `symbol_defined` facts. Documented in `docs/adapters/extractor-operations.md`.

### Docker Compose Operational Extraction

Status: verified.

Extends the operations adapter to common docker-compose file names. Parses top-level compose services as `DockerService` entities. Records service image and build context metadata. Parses service `command` and `entrypoint` declarations as `ScriptCommand` entities and compose service `environment` mappings as redacted `EnvVar` knowledge. Documented in `docs/adapters/extractor-operations.md`.

### GitHub Actions Operational Extraction

Status: verified.

Extends the operations adapter to `.github/workflows/*.yml` and `.github/workflows/*.yaml`. Parses workflow declarations, jobs, `runs-on` metadata, step `run` commands, and `uses` action references as `ScriptCommand` entities. Parses top-level, job-level, and step-level `env` mappings as redacted `EnvVar` knowledge. Documented in `docs/adapters/extractor-operations.md`.

### Cargo Manifest Operational Extraction

Status: verified.

Extends the operations adapter to `Cargo.toml`. Parses Cargo package and workspace declarations as `Package` entities. Parses dependencies, dev-dependencies, build-dependencies, workspace dependencies, and target-specific dependencies as `Dependency` entities with version, path, git, registry, package alias, optional, and feature metadata. Documented in `docs/adapters/extractor-operations.md`.

### Kubernetes Deployment Manifest Extraction

Status: verified.

Extends the operations adapter to common Kubernetes YAML manifest paths and filenames. Parses YAML documents with `kind` and `metadata.name` as deployment/service knowledge. Records workloads, services, ConfigMaps, Secrets, and related resources as `DockerService` entities. Parses container `command` and `args` as `ScriptCommand` entities and container `env` declarations as redacted `EnvVar` knowledge. Documented in `docs/adapters/extractor-operations.md`.

### SQL Database Migration Extraction

Status: verified.

Extends the operations adapter to SQL migration files in common migration paths and filenames. Parses migration files as `DbMigration` entities and simple `CREATE TABLE` statements as `DbTable` entities. Emits evidence-backed `migration_creates_table` facts from migrations to created tables. Documented in `docs/adapters/extractor-operations.md`.

### Runtime Configuration Extraction

Status: verified.

Extends the operations adapter to JSON, TOML, and YAML runtime configuration files in common config and settings paths. Flattens scalar configuration keys into redacted `Feature` entities, records scalar value kinds without storing raw configuration values, records env-like uppercase config keys as redacted `EnvVar` knowledge. Documented in `docs/adapters/extractor-operations.md`.

### Script Documentation Check View

Status: verified.

Adds the built-in `builtin.checker.script_docs` adapter. Checks canonical `ScriptCommand` entities for explicit Markdown `documents` relations. Emits evidence-backed `missing_documentation` diagnostics with payload scope `scripts`. Exposes findings through `ath check scripts` and `ath check scripts --json`. Documented in `docs/adapters/checker-api.md`.

### Deployment Documentation Check View

Status: verified.

Adds the built-in `builtin.checker.deployment_docs` adapter. Checks canonical `DockerService` deployment and service entities for explicit Markdown `documents` relations. Emits evidence-backed `missing_documentation` diagnostics with payload scope `deployment`. Exposes findings through `ath check deployment` and `ath check deployment --json`. Documented in `docs/adapters/checker-api.md`.

### Runbook Consistency Check View

Status: verified.

Emits canonical `Runbook` entities from Markdown frontmatter `kind: runbook` or `kind: operations_runbook`. Emits canonical `OperationStep` entities from ordered-list items. Adds the built-in `builtin.checker.runbook_consistency` adapter checking runbooks for known operational targets, extracted operation steps, and target coverage. Exposes findings through `ath check runbooks` and `ath check runbooks --json`. Documented in `docs/adapters/checker-api.md`.

### Operations Documentation Draft Expansion

Status: verified.

Extends `ath docs propose-fix` operations draft generation beyond `missing_env_var`. Creates reviewable Markdown operations pages for scoped script, deployment, and runbook documentation diagnostics. Writes drafts under `<editable_path>/operations/` with frontmatter `entities` pointing at the missing or stale operational stable key. Documented in `docs/development/docs-completeness-policy.md`.

### Operations Documentation Check Workflow

Status: verified.

Adds `ath docs operations check` and `ath docs operations check --json`. Aggregates environment, script, deployment, and runbook documentation diagnostics from one latest canonical snapshot load. Returns a non-zero process status when any operational documentation diagnostic is open. Documented in `docs/development/docs-completeness-policy.md`.

### Canonical Merge Deduplication

Status: verified.

Canonicalizes merged entities, facts, relations, and diagnostics by canonical id before storage. Removes duplicate canonical diagnostics carried forward from older snapshots. Ensures current-run objects replace carried objects on id conflicts during incremental indexing.

### OpenAPI Test Fixture Exclusion

Status: verified.

Prevents OpenAPI files under `tests/fixtures` from being extracted as product API knowledge during repository self-indexing. Keeps intentionally incomplete parser and contract fixtures available to adapter tests without creating false-positive API diagnostics.

### Canonical Graph JSON And GraphML Export

Status: verified.

Adds `ath graph export --format json` and `ath graph export --format graphml`. Reads the latest durable canonical snapshot without re-indexing. Emits bounded graph payload with node ids, stable keys, kinds, degrees, relation endpoints, status, confidence, and evidence anchors. Supports `--max-entities` and `--max-relations` limits. (`crates/athanor-app/src/graph.rs`, `apps/ath/src/main.rs`)

### Repository Overview Structure Summaries

Status: verified.

Extends `ath overview` with bounded canonical module structure summaries. Ranks modules by direct `defines` and `contains` members. Aggregates directional integration boundaries from canonical relations crossing source roots. Includes relation-kind counts and bounded canonical relation ids. (`crates/athanor-app/src/overview.rs`, `apps/ath/src/main.rs`)

### Related Entity Graph Navigation

Status: verified.

Adds `ath graph related <stable-key>` with JSON output. Traverses incoming and outgoing canonical relations breadth-first from one exact stable key. Bounds traversal depth, entity count, and relation count. Reports per-node distance plus canonical entity and relation ids. (`crates/athanor-app/src/graph.rs`, `apps/ath/src/main.rs`)

### Shortest Graph Path Navigation

Status: verified.

Adds `ath graph path <from-stable-key> <to-stable-key>` with JSON output. Searches incoming and outgoing canonical relations with deterministic breadth-first traversal. Returns one ordered shortest path while retaining each relation's canonical direction. Bounds search by maximum depth and visited entity count. (`crates/athanor-app/src/graph.rs`, `apps/ath/src/main.rs`)

### Graph Hub Degree Centrality

Status: verified.

Adds `ath graph hubs` with JSON output. Ranks connected canonical entities by total degree. Reports incoming and outgoing degree separately. Supports filtering by serialized canonical entity kind. Bounds ranked entities and relation ids per direction. (`crates/athanor-app/src/graph.rs`, `apps/ath/src/main.rs`)

### Directed Graph PageRank Centrality

Status: verified.

Adds `ath graph pagerank` with JSON output. Calculates directed PageRank over the complete latest canonical entity and relation graph. Redistributes dangling-node score. Bounds computation by maximum iterations and convergence tolerance. Applies optional entity-kind filtering after full-graph scoring. (`crates/athanor-app/src/graph.rs`, `apps/ath/src/main.rs`)

### Directed Graph Cycle Detection

Status: verified.

Adds `ath graph cycles` with JSON output. Finds simple cycles that follow canonical relation direction. Orders search roots and outgoing relations deterministically. Deduplicates the same directed cycle found from different starting entities. Bounds cycle length, search roots, and unique result count. (`crates/athanor-app/src/graph.rs`, `apps/ath/src/main.rs`)

### HTML Report Graph And Entity Detail Pages

Status: verified.

Extends `ath report html` output with entity detail pages for identity, ownership, relations, facts, diagnostics, and evidence locations. Adds a compact graph summary to `index.html` with relation-kind counts and a bounded interactive SVG graph supporting node search, relation-kind filtering, zoom, and deterministic layout. Adds embedded client-side filters for entity search, source path, entity kind, and diagnostic severity. Documented in `docs/adapters/projector-html.md`.

### Agent Bounded Retrieval Contract

Status: verified.

Establishes that generated JSONL, wiki, HTML, graph, API, search, and vector artifacts are backing read models or inspection outputs. Requires agent-facing access to use bounded query/context commands with explicit limits, stable schemas, canonical ids, and evidence links. Requires omitted or truncated counts when limits hide available canonical data.

### Explicit Multi-Repository Project Registry

Status: verified.

Adds `ath projects list`, `add`, `resolve`, and `remove`. Stores explicit project-id to canonical-root mappings in `~/.athanor/projects.json` by default. Emits stable `athanor.project_registry.v1` and `athanor.project_resolution.v1` JSON reports. Establishes the routing contract for future daemon and MCP multi-repository requests. (`crates/athanor-app/src/project_registry.rs`, `apps/ath/src/main.rs`)

### Daemon Lifecycle And Read-Only Protocol Slice

Status: verified.

Adds the `athd` daemon entrypoint with background start, foreground serve, ping, status, jobs, job, cancel, index, generate, wiki, report-html, overview, explain, search, context, and stop. Uses authenticated protocol v2 with fresh per-process token, loopback-only TCP, optional local socket transport, request and response byte limits, concurrent request handling, in-memory job registry, and bounded job history. Caches latest canonical snapshot, search handle, and bounded overview and context results. Registers background jobs with cancellation tokens. (`apps/athd/src/main.rs`, `crates/athanor-app/src/daemon.rs`, `docs/architecture/pipeline.md`)

### Production V1 Hardening

Status: verified.

Adds authenticated daemon protocol v2 and protected per-user runtime paths for Windows and Linux. Rejects non-loopback TCP, unauthenticated requests, and protocol v1 unless explicitly enabled. Adds crash-safe OS locking, lifecycle status, cooperative stop, bounded job drain, health diagnostics, and stale staging cleanup. Adds idempotent per-user Task Scheduler and `systemd --user` service install, status, and uninstall. Rotates structured JSONL daemon logs at 10 MiB with five retained files. Disables external process adapters by default. Adds optimized release builds, signed and attested Windows and Linux archives, authenticated daemon E2E, Windows service E2E, and nightly watcher and query soak coverage. Documented in `docs/development/production.md`.

### External Process Adapter Runtime Limits

Status: verified.

Rejects unknown adapter manifest, adapter entry, and process command fields. Rejects empty command programs, parent-directory components, and bare command names. Canonicalizes absolute command paths and requires manifest-relative paths to stay inside the manifest directory. Requires every external command program to match `[adapters].external_process_allowlist`. Bounds serialized stdin, stdout, stderr, and wall-clock execution. Terminates timed-out adapter processes. Documented in `docs/architecture/adapters.md` and `docs/development/production.md`.

### External Process Adapter User Trust

Status: verified.

Adds a user-level adapter trust store at `~/.athanor/adapter-trust.json` or `ATHANOR_ADAPTER_TRUST`. Records trusted adapter manifests by canonical manifest path and SHA-256 manifest content hash. Requires `[adapters].allow_external_process = true`, a matching executable allowlist entry, and a matching user-level trust record before loading external process adapters. Invalidates trust automatically when a manifest changes. Adds `ath plugins list`, `ath plugins trust <manifest>`, and `ath plugins untrust <manifest>`. Documented in `docs/operations/env-athanor-adapter-trust.md` and `docs/architecture/adapters.md`.

### Near-Term Hardening And Scale Audit

Status: verified.

Audited daemon lifecycle edge cases, scale follow-up items, and resulting P1 follow-up plan for pipeline benchmarks and daemon fault-injection coverage. Classification: implemented, implemented but under-tested, and implemented scale follow-up. Documented in `docs/development/production.md`.

### Full/Incremental Canonical Equivalence Test

Status: verified.

Adds an app-layer regression test that indexes one fixture incrementally and another from scratch in the same final source state. Loads durable canonical snapshots directly from `JsonlKnowledgeStore`. Compares canonical entities, facts, relations, and diagnostics after normalizing snapshot ids. Verifies that incremental changed-file indexing preserves stable ids, evidence, ownership, relations, and diagnostics equivalently to a fresh full index. (`crates/athanor-app/src/index.rs`)

### Bounded Index Metrics Report

Status: verified.

Adds bounded `athanor.index_metrics.v1` pipeline metrics to `IndexPipelineOutput`. Records wall-clock phase timings for source discovery, affected-file classification, snapshot begin, extraction, merge, linking, checking, canonicalization, and canonical storage. Records discovered, changed, unchanged, removed, extracted, and final canonical object counts. Aggregates adapter metrics by phase and adapter name. Adds `ath index --json`. Includes metrics in daemon index job results and MCP index tool responses. (`crates/athanor-app/src/pipeline.rs`, `apps/ath/src/main.rs`)

### Pipeline Benchmark Fixtures

Status: verified.

Adds `ath bench --size <small|medium|large>` for synthetic Markdown, Rust, and OpenAPI indexing fixtures. Runs the normal `index_project` path. Emits `athanor.index_benchmark.v1` in JSON mode including nested bounded `IndexReport` and pipeline metrics. Supports temporary fixtures by default and optional `--root <path> --keep-fixture`. (`crates/athanor-app/src/bench.rs`, `apps/ath/src/main.rs`)

### Daemon Fault-Injection Coverage Slice

Status: verified.

Comprehensive daemon fault-injection coverage including client disconnects, malformed requests, duplicate writable jobs, protocol cancellation, invalid JSON, busy responses, oversized requests, stale and corrupted runtime metadata, lock reacquisition after crash, unsafe serve options, cancellation of index and generation jobs, shutdown draining, concurrent read-only contention during index jobs, Unix and Windows socket lifecycle, watcher debounce, and polling-watcher smoke tests. Full test inventory documented in `docs/development/production.md`.

### Pipeline Shared Input Copy Reduction

Status: verified.

Changes `LinkInput` and `CheckInput` full-context fields from `Arc<[T]>` to `Arc<Vec<T>>`. Moves canonicalized entity and fact vectors into shared `Arc<Vec<T>>` values once before linker execution, reuses the same allocations for checker execution. Preserves external process adapter JSON compatibility. Returns owned vectors for canonical storage through `Arc::try_unwrap` when no adapter retained a reference, falling back to a clone only when necessary. (`crates/athanor-core/src/ports.rs`, `crates/athanor-app/src/pipeline.rs`)

### Analysis Coverage Report Slice

Status: verified.

Adds `ath coverage`, `ath coverage --json`, `ath coverage --adapter <id>`, and `ath coverage --file <path>`. Reads the latest canonical snapshot and `.athanor/state/index-state.json` without running indexing. Emits stable `athanor.coverage.v1` with tracked-file counts, canonical object counts, adapter evidence and fact coverage, diagnostic-kind counts, file-level rows, applied filters, explicit row limits, and omitted counts. (`crates/athanor-app/src/coverage.rs`, `apps/ath/src/main.rs`)

### JavaScript/TypeScript Adapter Initial Slice

Status: verified.

Adds one built-in `athanor-extractor-js-ts` language adapter for mixed JavaScript and TypeScript projects. Supports `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, and `.cts` source files with per-file language hints. Parses JavaScript and JSX through `tree-sitter-javascript` and TypeScript and TSX through `tree-sitter-typescript`. Extracts module import/export payloads, functions, methods, classes, TypeScript interface and type declarations, and `package.json` package and dependency declarations. Extends extractor output so adapters can emit evidence-backed diagnostics directly when parser recovery or unsupported syntax is detected. Documented in `docs/adapters/extractor-js-ts.md`.

### JavaScript/TypeScript Dual-Parser Verification Mode

Status: verified.

Adds opt-in `js-ts-precision` build features for `ath` and `athd`. Runs maintained Rust-native Oxc as a second parser for affected JS/TS source files and compares adapter-local normalized declarations, static imports, source-backed re-exports, ranges, and parser recovery state. Retains tree-sitter as the canonical-output backend. Emits evidence-backed diagnostics for backend-only findings, source-range mismatches, and recovery differences. Records bounded `athanor.js_ts_precision.v1` module metrics. Documented in `docs/adapters/extractor-js-ts.md`.

### JavaScript/TypeScript Relative Import Linker

Status: verified.

Adds the built-in `js-ts-imports` linker over normalized JS/TS module import payloads. Resolves only exact relative imports against existing canonical modules, including extensionless paths and directory `index` modules. Emits verified `imports` relations with import-line evidence and ownership from both source files. Advances persisted index state to v33. Documented in `docs/adapters/linker-js-ts.md`.

### Affected Diagnostic Check

Status: verified.

Adds `ath check affected` and `ath check affected --json`. Reports open diagnostics from the latest canonical snapshot that touch changed or removed files. Reports stale or potentially stale local artifacts for coordinated generated generations. Emits stable `athanor.affected_check.v1` JSON with affected file counts, drift, and diagnostics. (`crates/athanor-app/src/check.rs`, `apps/ath/src/main.rs`)

### Diff-Based Context Packs

Status: verified.

Adds `ath context --diff` and `ath context --diff --json`. Selects canonical entities from changed or removed files as direct context roots. Emits the normal `athanor.context_pack.v1` payload plus diff file counts. (`crates/athanor-app/src/context.rs`, `apps/ath/src/main.rs`, `crates/athanor-transport-mcp/src/lib.rs`)

### Fast Changed-File Extractor Preflight

Status: verified.

Adds `ath validate-changed --path <path>` and `--file <path>` for fast extractor-only preflight. Selects changed and untracked paths from Git status, runs extractors without linkers, checkers, or storage. Returns `athanor.changed_validation.v1` with changed and removed counts, diagnostics, and extraction metrics. (`crates/athanor-app/src/validate_changed.rs`)

### Changed-File Update Command

Status: verified.

Adds `ath update --changed` and `ath update --changed --json`. Exposes the existing incremental indexing path as an explicit update command. Writes a new durable canonical snapshot and JSONL read model. (`apps/ath/src/main.rs`, `crates/athanor-app/src/index.rs`)

### Repair Commands

Status: verified.

`ath repair inspect` validates local canonical and generated pointers without modifying files. `ath repair cleanup` removes orphan canonical snapshot and generated generation directories. `ath repair regenerate` detects stale, missing, or invalid generated-current pointers and republishes from latest snapshot. `ath repair recover-canonical` detects missing or dangling `latest.json` and rewrites it atomically. `ath repair apply` runs canonical recovery, regeneration, and cleanup in deterministic order. All repair commands support `--dry-run` and `--json` output. (`apps/ath/src/main.rs`, `crates/athanor-app/src/repair.rs`)

### Phase 6 Remainder - Affected Workflow And Repair

Status: verified.

Completed affected workflow artifact and documentation drift reporting, and generated-current repair handling for stale, missing, or invalid pointers and manifests.

### Phase 6.5 - Agent Graph Navigation And Overview

Status: verified.

Completed repository overview queries, GraphML export, interactive HTML graph, related-entity exploration, shortest path, degree-centrality hubs, PageRank, cycle detection, and path-step impact explanations. Multi-repository registry routing foundation implemented.

## Next

This backlog tracks the remaining global plan from `start.md`. The entries below are prioritized by dependency order and current product value; each item should be moved into `Implemented` only after code, documentation, and required verification are complete.

### OpenAPI And GraphQL Contract Consistency

Status: in progress.

Priority: P1.

Implemented first slice in:

- `crates/athanor-extractor-graphql`
- `crates/athanor-checker-api`
- `crates/athanor-linker-api`
- `docs/adapters/extractor-graphql.md`
- `docs/adapters/checker-api.md`
- `docs/adapters/linker-api.md`

Current implementation covers: SDL/introspection extraction, operation/fragment/directive/schema entities, fragment-spread resolution, variable validation, directive argument validation, operation-to-schema-type linking, and OpenAPI/GraphQL drift detection. Full implementation details in `docs/adapters/extractor-graphql.md` and `docs/adapters/linker-api.md`.

Remaining:

- replace or supplement the dependency-free recognizer with a formal GraphQL parser contract and fixture corpus

Scope:

- add a community-facing GraphQL contract adapter alongside the existing OpenAPI adapter
- normalize OpenAPI and GraphQL findings into a shared API contract model
- extend API linking and checking for cross-protocol drift with evidence
- add Rustok-specific API checks as an opt-in adapter layer

Acceptance:

- OpenAPI and GraphQL receive equal first-class contract treatment
- community users can validate GraphQL contracts without enabling Rustok-specific adapters
- every reported inconsistency includes source evidence and ownership

### Whole-Code Relationship Graph And Change Map

Status: in progress.

Priority: P1.

Implemented first slice in:

- `crates/athanor-app/src/change_map.rs`
- `apps/ath/src/main.rs`
- `apps/athd/src/main.rs`
- `crates/athanor-transport-mcp/src/lib.rs`
- `docs/adapters/change-map.md`

Current implementation: task-, target-, and diff-rooted `ath change-map` queries with bounded `athanor.change_map.v1` output, relation-chain explanations, file diversity, diagnostic attachment, and adapter annotations. Full algorithm, scoring, and test coverage in `docs/adapters/change-map.md`.

Remaining:

- add planned GraphQL, framework, database, and frontend callsite relations
- tune ranking against real Rustok repository
- add end-to-end daemon and MCP fixtures

Scope:

- provide a community-facing code relationship graph connecting contracts, source, tests, docs, and diagnostics
- rank impacted items by relation type, confidence, ownership, diagnostics, and test coverage
- extend language/framework adapters over time without moving adapter semantics into domain/core

Acceptance:

- community users receive a bounded, evidence-backed change map
- every returned item includes selection reason, relation chain, source evidence, and stable identifiers
- results degrade honestly when adapters provide partial knowledge

Acceptance:

- community users can ask for a change map and receive a bounded, evidence-backed list of likely code, contract, docs, test, and operations locations to inspect or update
- Rustok users receive the same generic map plus platform-specific reasons such as FBA ownership, permission gates, Page Builder dependencies, route/resolver ownership, and compatibility risk
- every returned item includes why it was selected, the relation chain from the task/diff/target, source evidence, ownership, and stable canonical identifiers
- results degrade honestly when adapters only provide partial knowledge, including confidence, omitted counts, unsupported syntax or skipped-file diagnostics, and links to capability/completeness reports
- tests cover task-targeted change maps, diff-based change maps, relation-chain explanations, deterministic ranking, omitted-limit reporting, missing-test surfacing, and Rustok-specific platform annotations

### Analysis Completeness Reporting

Status: in progress (first slice delivered).

Priority: P2.

Scope:

- add canonical coverage/capability facts or diagnostics with evidence for unsupported syntax, skipped files, partial parsing, parser recovery, and extractor confidence
- expose bounded commands such as `ath capabilities`, `ath capabilities --json`, and deeper `ath coverage` variants for fully processed, partially processed, skipped, and unsupported constructs
- report per-adapter discovered files, fully processed files, partially processed files, skipped files, unsupported constructs, and omitted counts when limits apply
- keep coverage output agent-facing through stable schemas and explicit limits instead of requiring generated artifact reads

Delivered (first slice):

- adds `ath capabilities`, `ath capabilities --json`, `ath capabilities --limit <n>`, and `ath capabilities --min-confidence <f>`
- reads the latest canonical snapshot and `.athanor/state/index-state.json` without running indexing or reading generated JSONL artifacts
- emits stable `athanor.capabilities.v1` with tracked-file counts, content-processed counts and ratio (files that received extraction beyond the baseline `file` inventory adapter), per-language completeness, per-adapter files/facts/low-confidence/min-confidence rows, below-threshold facts with evidence paths, content-unprocessed file rows, explicit row limits, and omitted counts
- unit tests cover content-unprocessed detection, low-confidence fact surfacing, and bounded limits with omitted counts

Remaining:

- canonical capability/coverage facts or diagnostics with evidence for unsupported syntax, skipped files, partial parsing, and parser recovery
- deeper `ath coverage` variants for fully processed, partially processed, skipped, and unsupported constructs
- documented adapter-level capability declarations

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