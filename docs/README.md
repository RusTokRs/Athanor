---
id: doc://docs/README.md
kind: project_overview
language: en
source_language: en
status: active
---
# Athanor Documentation Map

Use this index to locate current architecture, implementation workflow, adapter contracts, operations
guides, and long-range plans. Frontmatter describes content lifecycle; it does not prove that the
current source commit passed Rust verification.

## Start Here

- [Project overview](../README.md): product summary, quick start, and user-facing commands.
- [Agent entrypoint](../AGENTS.md): mandatory repository instructions.
- [Agent workflow](development/agent-workflow.md): planning, implementation, and verification.
- [Roadmap status](development/roadmap-status.md): compact current-state ledger.
- [Implementation plan](../athanor_implementation_plan_ru.md): package status and exact matrix.
- [Long-range architecture plan](../start.md): future product direction.

## Current Architecture

- [Indexing pipeline](architecture/pipeline.md): composition, incremental phases, and publication.
- [Adapter architecture](architecture/adapters.md): Source/Extractor/Linker/Checker boundaries.
- [Store conformance](architecture/store-conformance.md): snapshot visibility and backend behavior.
- [Publication semantics](development/publication-semantics-inventory.md): staging, recovery, and pointers.
- [Runtime composition migration](development/legacy-runtime-compatibility.md): removed globals and
  bounded owners.
- [Direct operation context](development/direct-operation-context.md): cancellation and deadlines.
- [JSON contract inventory](development/json-contract-inventory.md): schema and protocol ownership.
- [Evidence-backed documentation generation](development/evidence-backed-documentation-generation-plan.md):
  strict contracts, deterministic architecture profile, immutable app-layer publication, and next CLI
  integration slice.

## Development And Governance

- [Contributing](../CONTRIBUTING.md)
- [Coding standards](development/athanor-coding-standards.md)
- [Definition of done](development/definition-of-done.md)
- [Continuous integration](development/ci.md)
- [Release procedure](development/release.md)
- [Production operation](development/production.md)
- [Documentation completeness](development/docs-completeness-policy.md)
- [Library adoption](development/library-adoption-plan.md)
- [ADR template](development/adr-template.md)

## Core Adapter Documentation

### Extraction

- [Basic/file inventory](adapters/extractor-basic.md)
- [Markdown](adapters/extractor-markdown.md)
- [Rust](adapters/extractor-rust.md)
- [JavaScript/TypeScript](adapters/extractor-js-ts.md)
- [OpenAPI](adapters/extractor-openapi.md)
- [GraphQL](adapters/extractor-graphql.md)
- [Operations sources](adapters/extractor-operations.md)

### Linking And Checking

- [Markdown linker](adapters/linker-markdown.md)
- [Rust linker](adapters/linker-rust.md)
- [JavaScript/TypeScript linker](adapters/linker-js-ts.md)
- [API knowledge linker](adapters/linker-api.md)
- [Markdown checker](adapters/checker-markdown.md)
- [API and operations checker](adapters/checker-api.md)

### Read Models And Services

- [JSONL Store](adapters/store-jsonl.md)
- [SurrealDB Store](adapters/store-surrealdb.md)
- [Tantivy search](adapters/search-tantivy.md)
- [Markdown Wiki projector](adapters/projector-wiki.md)
- [HTML report projector](adapters/projector-html.md)
- [MCP transport](adapters/transport-mcp.md)
- [Impact analysis](adapters/impact.md)
- [Change Map](adapters/change-map.md)

### RusTok Opt-In Adapters

- [FFA](adapters/rustok-ffa.md)
- [FBA](adapters/rustok-fba.md)
- [Page Builder](adapters/rustok-page-builder.md)
- [Architecture Context](adapters/rustok-architecture-context.md)

Crate-local README files describe implementation-specific fixtures and configuration. Prefer the
high-level documents above for cross-crate architecture.

## Operations Documentation

Operations pages under `operations/` document environment variables, runtime directories, CI,
production, release, security, workflow jobs, daemon lifecycle, and generated command steps.

Important entry pages include:

- [CI workflow](operations/script-script-command-github-workflows-ci-yml-github-actions-workflow.md)
- [Production gate](operations/script-script-command-github-workflows-production-yml-github-actions-workflow.md)
- [Release workflow](operations/script-script-command-github-workflows-release-yml-github-actions-workflow.md)
- [Security audit](operations/script-script-command-github-workflows-security-yml-github-actions-workflow.md)
- [ATHANOR_RUNTIME_DIR](operations/env-athanor-runtime-dir.md)
- [ATHANOR_PROJECT_REGISTRY](operations/env-athanor-project-registry.md)
- [ATHANOR_ADAPTER_TRUST](operations/env-athanor-adapter-trust.md)

Use `ath docs operations check` for canonical completeness diagnostics; this map is not evidence of
operations-documentation completeness.

## Generated And Editable Documentation

`ath generate` continues to publish coordinated JSONL, Wiki, and HTML outputs from one canonical
snapshot. Those artifacts are disposable projections and are not editable source documentation.

```bash
cargo run -p ath --quiet -- generate .
cargo run -p ath --quiet -- repair inspect .
```

Editable documentation remains review-oriented:

```bash
cargo run -p ath --quiet -- docs check .
cargo run -p ath --quiet -- docs drift .
cargo run -p ath --quiet -- docs propose-fix .
cargo run -p ath --quiet -- docs apply-patch . <patch-id-or-path>
```

Evidence-backed architecture generation is implemented as an application API through Slice 1B. It
creates cited deterministic Markdown and validation output under immutable documentation generations,
checks exact artifact hashes, and advances a separate atomic current pointer. There is not yet a
supported `ath` command that loads a committed snapshot and invokes this publisher; Slice 1C owns that
user-facing boundary. No provider, daemon, MCP, or editable-document write path is enabled.

## Plans

- [Evidence-backed documentation generation](development/evidence-backed-documentation-generation-plan.md)
- [RusTok FFA/FBA improvements](development/rustok-ffa-fba-adapter-improvement-plan.md)
- [Dart/Flutter adapter integration](development/dart-flutter-adapter-plan.md)

Plans define bounded target work and must not be cited as implementation or execution evidence.

## Verification

The authoritative matrix is in `athanor_implementation_plan_ru.md`. Documentation-generation status
changes include:

```bash
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
cargo test -p athanor-app --test documentation_architecture_publication_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p athanor-app --test release_readiness_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo fmt --all -- --check
```

Slices 1A–1B are execution-confirmed on source commit
`0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`; the package remains in progress until Slice 1C provides
committed-snapshot loading and a bounded CLI surface.
