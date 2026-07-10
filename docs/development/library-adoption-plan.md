---
id: doc://docs/development/library-adoption-plan.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Library Adoption Plan

This document records where Athanor should use maintained third-party libraries instead of
reimplementing generic infrastructure. It is a dependency decision log, not permission to expose
library-specific types through domain or core contracts.

## Decision Rules

A library may be adopted when it has:

- active maintenance and recent releases or commits
- a license compatible with Athanor
- an MSRV compatible with the workspace
- coverage for the standards and platforms Athanor claims to support
- a stable adapter boundary and contract tests
- a clear removal or replacement path

Every adoption must keep third-party types inside its adapter crate. `athanor-domain` and
`athanor-core` continue to expose Athanor-owned contracts only.

Version numbers in this plan describe the version inspected when the decision was recorded. Actual
implementation tasks must pin a compatible version in `Cargo.lock` and rerun the dependency audit.

## Existing Foundation

| Library | Decision | Why |
| --- | --- | --- |
| `serde`, `serde_json` | Retain | Canonical serialization foundation with no leakage beyond normal serialized values. |
| `clap` | Retain | Maintained CLI parser; CLI-specific types stay in `apps/ath`. |
| `tokio` | Retain | Async runtime needed by app services and future daemon work. |
| `syn`, `proc-macro2` | Retain | Appropriate parser foundation for the Rust extractor; emitted knowledge remains Athanor-owned. |
| `anyhow`, `thiserror` | Retain | App/adapter error handling and typed core errors. |
| `async-trait` | Retain for current ports | Reassess only if native async trait ergonomics justify a contract migration. |

## Approved Adoptions

### Markdown Parsing: `pulldown-cmark`

Status: adopted and verified with `pulldown-cmark` 0.13.4.

Use it for CommonMark/GFM event parsing and source offsets. Replace the current line-based heading
scanner while preserving the existing canonical page/section keys, evidence lines, ownership, and
incremental behavior.

Boundary:

```text
pulldown-cmark events
  -> athanor-extractor-markdown internal mapping
  -> Entity / Fact
```

Acceptance criteria:

- headings inside fenced code blocks are not extracted
- ATX and supported setext headings have correct source evidence
- duplicate heading stable keys remain deterministic
- current Markdown contract tests remain valid
- GFM extensions are enabled explicitly rather than inherited accidentally

Implemented in `athanor-extractor-markdown`; canonical types and stable slug generation remain
Athanor-owned.

Project: <https://github.com/pulldown-cmark/pulldown-cmark>

### OpenAPI 3.1 Parsing: `oas3`

Status: adopted and verified with `oas3` 0.22.0 for OpenAPI 3.1.

The inspected `oas3` 0.22 line provides typed OpenAPI 3.1.x parsing, navigation, validation,
order-preserving maps, and optional YAML support. It becomes the preferred parser for OpenAPI 3.1,
not a domain dependency.

Required adapter family:

```text
OpenApiDocumentParser
  -> Oas3Parser              (OpenAPI 3.1)
  -> LegacyValueParser       (OpenAPI 3.0 fallback)
  -> NormalizedOpenApiDocument
  -> OpenApiExtractor
```

The normalized document is private to the OpenAPI adapter family. `oas3` types must not appear in
`athanor-domain`, `athanor-core`, canonical payload contracts, or process adapter protocols.

Acceptance criteria:

- contract corpus covers OpenAPI 3.0.3, 3.1.0, and 3.1.1 in YAML and JSON
- endpoint/schema stable keys do not change
- request/response schema relations and evidence remain deterministic
- local `$ref` behavior is no worse than the current extractor
- unsupported 3.0 documents continue through the legacy parser
- parser selection and error messages identify the selected backend

Project: <https://github.com/x52dev/oas3-rs>

### JSON Schema Validation: `jsonschema`

Status: adopted and verified with `jsonschema` 0.46.5 for OpenAPI media-example validation.

Use it to validate JSON examples and instances against OpenAPI-compatible schemas. Do not implement
a custom JSON Schema evaluator. Keep network and file reference resolution disabled by default so
offline indexing stays deterministic; expose reference loading through an explicit adapter policy.

Acceptance criteria:

- supported JSON Schema dialect is derived from the OpenAPI version
- validation errors map to Athanor diagnostics with evidence
- remote resolution is opt-in and testable
- validators are cached per canonical schema within one run

Implemented in `athanor-checker-api` with default features disabled. OpenAPI 3.0 selects Draft 4;
OpenAPI 3.1 selects Draft 2020-12. Same-document component schemas are assembled in memory, while
external references are skipped so indexing remains offline and deterministic.

Project: <https://github.com/Stranger6667/jsonschema>

### Filesystem Watching: `notify`

Status: adopted for Phase 7 daemon work through `notify-debouncer-mini` 0.7.0, which depends on
`notify` 8.2.0.

Use `notify` through a maintained debouncer crate for cross-platform filesystem events. Athanor
still owns event normalization, affected-file classification, queueing, cancellation, and snapshot
semantics.

Acceptance criteria:

- Linux, macOS, and Windows behavior is covered by adapter contract tests where CI permits
- rename/create/remove bursts normalize into deterministic project-relative changes
- debounce policy is configurable and does not replace persisted content hashing
- polling remains available as a fallback

Implemented in `athanor-app` daemon serving only. `notify` and debouncer types remain private to
the app layer; daemon protocol payloads expose only Athanor-owned watcher settings and job records.
The implemented watcher ignores `.athanor` artifact events and schedules debounced background index
jobs; persisted content hashes still decide actual changed/unchanged/removed files. The default
daemon watcher uses the platform-recommended backend, and `athd serve --watch --watch-poll` selects
the polling backend when native filesystem events are not suitable.

Project: <https://github.com/notify-rs/notify>

### Lexical Search: `tantivy`

Status: adopt in Phase 9 as a `SearchIndex` adapter.

Use Tantivy for BM25 lexical search, tokenization, and incremental search-index updates. Tantivy is
a disposable read model and must never become the canonical knowledge store.

Acceptance criteria:

- index can be deleted and rebuilt from a canonical snapshot
- search results retain canonical entity ids and evidence references
- tokenizer configuration is versioned
- commits and reader reloads are coordinated by the adapter
- context generation can fall back to the current deterministic lexical path

Project: <https://github.com/quickwit-oss/tantivy>

## Required Replacement Spike

### YAML Backend

Status: replacement completed and verified.

The unmaintained `serde_yaml` dependency has been removed. The implemented direction is:

- use the YAML backend selected by `oas3` inside the OpenAPI 3.1 parser adapter
- use `serde_yaml_ng` for preflight normalization, the OpenAPI 3.0 legacy parser, and the
  adapter-private Markdown frontmatter contract
- do not expose YAML-library values outside parser adapters

The current compatibility corpus covers OpenAPI 3.0.3, 3.1.0, and 3.1.1 in YAML and JSON. Markdown
frontmatter tests cover CRLF delimiters, body offsets, malformed YAML, and missing closing
delimiters. YAML anchors, aliases, tags, duplicate-key policy, and detailed error-location parity
remain follow-up contract cases before broader generic YAML ingestion.

Projects:

- unmaintained current dependency: <https://github.com/dtolnay/serde-yaml>
- maintained candidate: <https://github.com/acatton/serde-yaml-ng>

## Conditional Adoptions

| Area | Candidate | Decision condition |
| --- | --- | --- |
| Impact graph algorithms | `petgraph` | Adopt only if Phase 6 benchmarks show current indexed adjacency traversal is insufficient. Build a temporary graph from canonical relations; do not replace them. |
| Markdown wiki templating | `minijinja` or `askama` | Defer. Start with a deterministic writer; adopt templates only when multiple projectors duplicate presentation logic. |
| Standalone database | SurrealDB Rust SDK | Phase 1/standalone adapter only after persistence, migration, backup, and embedded-mode tests. Keep JSONL as portable fallback. |
| Rustok persistence | SeaORM/Postgres | Phase 10 adapter only. Never make Postgres mandatory for standalone use. |
| Semantic embeddings/vector index | To be selected | Do not select before Phase 9 retrieval benchmarks define dimensions, offline requirements, hardware targets, and replacement contracts. |
| SCIP/LSIF import | To be selected | Do not select before Phase 12 importer contracts and fixture corpus exist. |
| AI documentation generation | Individual libraries to be selected | Do not adopt a whole wiki-generator application. First implement Athanor-owned evidence/context, citation, validation, and publication contracts; then evaluate each provider, template, or diagram library behind a replaceable adapter. |

## Athanor-Owned Implementations

The following are product semantics and must not be delegated wholesale to third-party libraries:

- canonical `Entity`, `Fact`, `Relation`, `Diagnostic`, and `Evidence` models
- stable keys and stable ID compatibility
- ownership metadata and invalidation rules
- adapter registry and process adapter protocol
- canonical snapshot commit semantics
- incremental merge, pruning, and affected-subset contracts
- diagnostic classification and context-pack contracts

Libraries may implement adapter internals around these contracts, but they do not define the
contracts themselves.

## Adoption Workflow

Every library task follows this sequence:

1. Record inspected version, license, MSRV, maintenance evidence, and feature flags.
2. Add a fixture-based spike behind an adapter or private parser trait.
3. Run existing behavior as the baseline and compare canonical output.
4. Add contract tests for known edge cases and replacement behavior.
5. Migrate without changing stable keys unless a documented schema migration is approved.
6. Update adapter docs, architecture docs, roadmap status, and `Cargo.lock`.
7. Run full workspace and runtime verification.
