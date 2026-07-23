---
id: doc://docs/README.md
kind: project_overview
language: en
source_language: en
status: active
---
# Athanor Documentation Map

Use this index to locate current architecture, implementation workflow, adapter contracts, operations
guides, and long-range plans. A document's frontmatter describes that document's content lifecycle; it
does not prove that the current repository commit passed Rust verification.

## Start Here

- [Project overview](../README.md): product summary, quick start, and user-facing commands.
- [Agent entrypoint](../AGENTS.md): mandatory repository instructions for implementation agents.
- [Agent workflow](development/agent-workflow.md): planning, implementation, documentation, and
  verification workflow.
- [Roadmap status](development/roadmap-status.md): compact current-state ledger and active work.
- [Implementation plan](../athanor_implementation_plan_ru.md): detailed package status and one-commit
  verification matrix.
- [Long-range architecture plan](../start.md): product direction and future phases.

## Current Architecture

- [Indexing pipeline](architecture/pipeline.md): current composition, incremental phases,
  transactional publication, target work, and historical layouts.
- [Adapter architecture](architecture/adapters.md): Source/Extractor/Linker/Checker boundaries,
  built-in adapters, plugin discovery, and external process policy.
- [Store conformance](architecture/store-conformance.md): snapshot visibility and backend behavior.
- [Publication semantics](development/publication-semantics-inventory.md): durable publication,
  recovery, pointer ownership, and source enforcement.
- [Runtime composition migration](development/legacy-runtime-compatibility.md): removed globals,
  retained composition APIs, bounded owners, and embedder migration.
- [Direct operation context](development/direct-operation-context.md): CLI/MCP cancellation,
  deadlines, worker drain, and transactional Index semantics.
- [JSON contract inventory](development/json-contract-inventory.md): public, persisted, generated,
  adapter, MCP, and process protocol ownership.

## Development And Governance

- [Contributing](../CONTRIBUTING.md): contribution and review expectations.
- [Coding standards](development/athanor-coding-standards.md): normative Rust and architecture rules.
- [Definition of done](development/definition-of-done.md): completion requirements.
- [Continuous integration](development/ci.md): intended GitHub Actions matrix and commands.
- [Release procedure](development/release.md): version, changelog, artifact, publication, and recovery
  contract.
- [Production operation](development/production.md): daemon, runtime permissions, release, and
  external-adapter policy.
- [Documentation completeness](development/docs-completeness-policy.md): editable documentation
  policy and `ath docs` workflows.
- [Library adoption](development/library-adoption-plan.md): dependency choices and adapter seams.
- [ADR template](development/adr-template.md): required structure for material architecture changes.

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

Crate-local README files document implementation-specific configuration and fixtures. Prefer the
high-level documents above for cross-crate architecture.

## Operations Documentation

Operations pages under `operations/` document:

- environment variables and runtime directories;
- CI, production, release, and security workflows;
- workflow jobs and individual generated command/action steps;
- daemon lifecycle and release procedures.

Important entry pages include:

- [CI workflow](operations/script-script-command-github-workflows-ci-yml-github-actions-workflow.md)
- [Production gate](operations/script-script-command-github-workflows-production-yml-github-actions-workflow.md)
- [Release workflow](operations/script-script-command-github-workflows-release-yml-github-actions-workflow.md)
- [Security audit workflow](operations/script-script-command-github-workflows-security-yml-github-actions-workflow.md)
- [ATHANOR_RUNTIME_DIR](operations/env-athanor-runtime-dir.md)
- [ATHANOR_PROJECT_REGISTRY](operations/env-athanor-project-registry.md)
- [ATHANOR_ADAPTER_TRUST](operations/env-athanor-adapter-trust.md)

Use `ath docs operations check` to evaluate canonical operations-documentation diagnostics rather than
assuming this index proves completeness.

## Generated And Editable Documentation

`ath generate` publishes immutable JSONL, Wiki, and HTML generation artifacts from one canonical
snapshot. Generated artifacts are disposable projections and are not editable source documentation.

```bash
cargo run -p ath --quiet -- generate .
cargo run -p ath --quiet -- repair inspect .
```

Editable documentation workflows are review-oriented:

```bash
cargo run -p ath --quiet -- docs check .
cargo run -p ath --quiet -- docs drift .
cargo run -p ath --quiet -- docs propose-fix .
cargo run -p ath --quiet -- docs apply-patch . <patch-id-or-path>
```

The Docs application services load canonical snapshots through explicit runtime composition. Patch
proposals remain reviewable JSON interchange documents before explicit application.

Evidence-backed documentation generation is currently implemented only through strict versioned
request and manifest contracts. There is no architecture-document runtime generator, model provider,
new projector wiring, or editable-document write path in the current slice.

## Plans

Focused plans include:

- [Evidence-backed documentation generation](development/evidence-backed-documentation-generation-plan.md)
- [RusTok FFA/FBA improvements](development/rustok-ffa-fba-adapter-improvement-plan.md)
- [Dart/Flutter adapter integration](development/dart-flutter-adapter-plan.md)

Plans describe target work. They must not be cited as evidence that a feature is implemented or
verified.

## Verification

The authoritative current matrix is in `athanor_implementation_plan_ru.md`. At minimum, documentation
status changes should include:

```bash
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p athanor-app --test release_readiness_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo fmt --all -- --check
```

Current direct-to-main architecture changes remain implemented, not verified, until the complete
matrix runs on one commit.
