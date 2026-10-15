# Changelog

All notable user-visible changes to Athanor are recorded in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and release versions use
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). The top `[Unreleased]` section collects
changes after the latest frozen candidate. A release version must have exactly one section with an ISO
`YYYY-MM-DD` date and at least one substantive note before its matching `v<version>` tag may be
published. HTML comments, thematic breaks, empty list markers, and empty fenced code blocks are not
substantive release notes.

## [Unreleased]

### Added

- Add Rust trait-supertrait and JS/TS class/interface `Extends`, preserving Rust/JS/TS `Implements`; bump index state v46→v47.
- Add the bounded `explore` MCP tool: one query returns ranked entries, citations, and one-hop relations.
- Add `builtin.extractor.treesitter` over Python and Go: one data-driven engine records declarations,
  receiver-qualified methods and other declarations, imports, exports, and statically named call sites.
- Add `builtin.linker.treesitter`: Python and Go imports, cross-file calls through package units,
  receiver variables, and import bindings, test bridges, and a per-language coverage measurement.
- Add a monitoring operations category: the operations profile derives declared health, status, and
  metrics endpoints and telemetry variable names from canonical evidence; probes, alerts, dashboards,
  and SLOs stay absent until they have an evidence source.
- Add the JavaScript/TypeScript call graph. The JS/TS extractor records each import statement's local
  bindings (`default_name`, `namespace_name`, `named`) and a bounded `calls` array per function,
  method, and variable-bound function declaration with the statically named callee, receiver, kind,
  argument count, and line/column. `builtin.linker.js_ts_calls` resolves those call sites into
  evidence-backed `Calls` relations (same-module, `this.` member, imported binding, and unique-name
  fallbacks) and emits `TestedBy` when a test-location declaration calls production code.
- Preserve JS/TS named import/export aliases, link relative re-export dependencies, resolve bounded call chains, and bump index state v47→v48.
- Add strict versioned request, outline, context, citation, draft, manifest, and validation-report
  contracts for evidence-backed documentation generation: exact snapshot binding, portable paths,
  stable-key/evidence traceability, cited diagram edges, metrics, and checksums.
- Add a minimal fixture repository and Rustok evaluation corpus with expected sections, citation paths,
  diagram edges, known gaps, repeatability, unsupported-relation disclosure, and review criteria.
- Add a deterministic architecture profile that converts one explicit canonical snapshot into cited
  Markdown, relation-backed Mermaid, omission disclosure, footnotes, metrics, and checksums.
- Add an application-layer immutable documentation publisher with staged generation directories,
  checksum-bound artifacts, an atomic `documentation/current.json` pointer, and exact `UpToDate` reuse.
- Add `ath docs generate-architecture` for one exact committed snapshot with hard-limit flags, `--force`,
  text/JSON reports, and Ctrl-C cancellation; the existing coordinated `ath generate` remains unchanged.
- Add validated `ath docs architecture current`, `manifest`, and `validation` inspection commands that
  reject pointer escape, identity drift, unsupported artifact layouts, and checksum mismatch.
- Add `overview` and `capabilities` MCP tools exposing the repository overview and the bounded
  analysis-completeness report to MCP clients.
- Add a bounded first-party projection for agent-tool MCP configuration: `.codex.json` and
  `antigravity.json` project one `ScriptCommand` per server, with evidence and ownership.
- Add local semantic retrieval with zero new dependencies: hashing-trick embeddings over character
  trigrams and word n-grams (FNV-1a buckets) power `ath search --mode semantic|hybrid` and MCP `search`.

### Fixed

- Stop generated module/API/operations/onboarding documents from claiming shipped publication, Store
  loading, CLI, and transport integration; headers name the deferred scope and its limits.
- Reject Windows drive paths, reserved device names, trailing-dot/space components, and
  case-insensitive output-path collisions before documentation artifacts can be published.
- Reject raw-file/secret context access, out-of-context citations and diagram endpoints, uncited factual
  claims, malformed evidence ranges, provider metrics without opt-in, and inconsistent valid reports;
  reject tampered manifests/artifacts as up to date and rebuild instead of reusing them.
- Reject missing or uncommitted documentation snapshot IDs instead of falling back to the latest visible
  snapshot, and preserve the previous current pointer when CLI generation is cancelled.
- Restore the bounded snapshot-search module and rebuild the workspace after the semantic-search slice:
  mode dispatch and hybrid ranking move to `search/execution.rs`, `execute_search` is re-exported, and
  `athanor-search-tantivy` compiles again with the declared `serde` dependency and score-ordered
  `TopDocs`; semantic hits are floored at 0.2 cosine similarity.

## [0.2.1] - 2026-07-23

### Fixed

- Package the actual `*.cdx.json` files emitted by `cargo-cyclonedx 0.5.9` instead of searching only for
  `bom.json`.
- Validate that the workspace SBOM inventory is non-empty and that every packaged document is a
  non-empty CycloneDX JSON object before signing or attesting the archive.

### Release

- Preserve `v0.2.0` as an immutable failed publication attempt: its release contract and Linux artifacts
  passed, but the SBOM inventory gate stopped publication before a GitHub Release was created.

## [0.2.0] - 2026-07-22

### Added

- Local-first `ath` CLI and `athd` daemon for indexing and querying repository knowledge.
- Explicit runtime composition for stores, search, projectors, extractors, and transports.
- Transactional index publication with cancellation-safe rollback and durable post-commit success.
- Memory, JSONL, and SurrealDB storage backends with shared conformance coverage.
- Rust, JavaScript/TypeScript, Markdown, OpenAPI, GraphQL, and Rustok-oriented extraction and linking.
- MCP transport with bounded request admission, responsive cancellation, and typed index reports.
- Cross-protocol OpenAPI/GraphQL consistency checks for requests, responses, status policy,
  authentication families, and permission scopes.
- Evidence-backed documentation lifecycle, JSON contract inventory, and repository indexing smoke tests.
- Cross-platform CI on Linux, macOS, and Windows with security, feature, coverage, installer, AppSec,
  Store Conformance, and Production Gate workflows.
- Release archives for Linux and Windows with checksums, Sigstore bundles, provenance attestations,
  and CycloneDX SBOMs.

### Changed

- Rename the root architecture audit briefing from `for agents. md` to `architecture_audit_briefing_ru.md`
  so the first-party Markdown briefing is named and indexed like the other root planning artifacts.
- Release tags are gated by repository-owned package-version, changelog, artifact, signature, provenance,
  SBOM, and release-note checks.
- Release preparation rejects occupied tag names and pins an annotated tag to the exact verified candidate
  SHA instead of assuming the current `main` HEAD is still the candidate.

### Security

- Exact commit statuses are published for the CI matrix, AppSec, and Store Conformance workflows.
- Release workflows use pinned actions, least-privilege job permissions, signed assets, and provenance.
- Release publication rejects malformed or impossible calendar dates, duplicate version sections,
  decorative-only notes, and empty fenced blocks before artifact jobs run.

## [0.1.0] - 2026-06-25

### Added

- Historical development snapshot establishing the initial `ath` and `athd` package versions.
