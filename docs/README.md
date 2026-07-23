---
id: doc://docs/README.md
kind: project_overview
language: en
source_language: en
status: active
---
# Athanor Documentation Map

Frontmatter describes content lifecycle; it does not prove that the current source commit passed Rust
verification. Exact package evidence lives in `athanor_implementation_plan_ru.md`.

## Start Here

- [Project overview](../README.md)
- [Agent entrypoint](../AGENTS.md)
- [Agent workflow](development/agent-workflow.md)
- [Roadmap status](development/roadmap-status.md)
- [Implementation plan](../athanor_implementation_plan_ru.md)
- [Long-range plan](../start.md)

## Current Architecture

- [Indexing pipeline](architecture/pipeline.md)
- [Adapter architecture](architecture/adapters.md)
- [Store conformance](architecture/store-conformance.md)
- [Publication semantics](development/publication-semantics-inventory.md)
- [Runtime composition migration](development/legacy-runtime-compatibility.md)
- [Direct operation context](development/direct-operation-context.md)
- [JSON contract inventory](development/json-contract-inventory.md)
- [Evidence-backed documentation generation](development/evidence-backed-documentation-generation-plan.md)

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

## Adapter Documentation

Extraction: [basic](adapters/extractor-basic.md), [Markdown](adapters/extractor-markdown.md),
[Rust](adapters/extractor-rust.md), [JavaScript/TypeScript](adapters/extractor-js-ts.md),
[OpenAPI](adapters/extractor-openapi.md), [GraphQL](adapters/extractor-graphql.md), and
[operations](adapters/extractor-operations.md).

Linking/checking: [Markdown linker](adapters/linker-markdown.md),
[Rust linker](adapters/linker-rust.md), [JS/TS linker](adapters/linker-js-ts.md),
[API linker](adapters/linker-api.md), [Markdown checker](adapters/checker-markdown.md), and
[API/operations checker](adapters/checker-api.md).

Read models/services: [JSONL](adapters/store-jsonl.md), [SurrealDB](adapters/store-surrealdb.md),
[Tantivy](adapters/search-tantivy.md), [Wiki](adapters/projector-wiki.md),
[HTML](adapters/projector-html.md), [MCP](adapters/transport-mcp.md),
[Impact](adapters/impact.md), and [Change Map](adapters/change-map.md).

RusTok opt-in adapters: [FFA](adapters/rustok-ffa.md), [FBA](adapters/rustok-fba.md),
[Page Builder](adapters/rustok-page-builder.md), and
[Architecture Context](adapters/rustok-architecture-context.md).

## Generated And Editable Documentation

`ath generate` continues to publish coordinated JSONL, Wiki, and HTML outputs. Architecture
documentation uses a separate exact-snapshot command and immutable current pointer:

```bash
ath docs generate-architecture . --snapshot <EXACT-COMMITTED-SNAPSHOT>
ath docs generate-architecture . --snapshot <EXACT-COMMITTED-SNAPSHOT> --json
ath docs architecture current .
ath docs architecture manifest . --json
ath docs architecture validation . --json
```

Generation requires an exact committed snapshot, has no latest fallback, accepts hard-limit and `--force`
flags, and cancels on Ctrl-C. Inspection validates pointer confinement, identities, artifact layout, and
checksums before output. Provider, daemon, and MCP integration are not enabled.

Editable documentation remains review-oriented:

```bash
ath docs check --path .
ath docs drift --path .
ath docs propose-fix --path .
ath docs apply-patch <PATCH> --path .
```

## Operations Documentation

Operations pages cover environment variables, runtime directories, CI, production, release, security,
daemon lifecycle, and generated workflow steps. Important entry pages include the
[CI](operations/script-script-command-github-workflows-ci-yml-github-actions-workflow.md),
[Production Gate](operations/script-script-command-github-workflows-production-yml-github-actions-workflow.md),
[Release](operations/script-script-command-github-workflows-release-yml-github-actions-workflow.md), and
[Security](operations/script-script-command-github-workflows-security-yml-github-actions-workflow.md)
workflows.

Use `ath docs operations check` for canonical completeness diagnostics.

## Plans

- [Evidence-backed documentation generation](development/evidence-backed-documentation-generation-plan.md)
- [RusTok FFA/FBA improvements](development/rustok-ffa-fba-adapter-improvement-plan.md)
- [Dart/Flutter integration](development/dart-flutter-adapter-plan.md)

## Verification

```bash
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
cargo test -p athanor-app --test documentation_architecture_publication_inventory --locked
cargo test -p athanor-app --test documentation_architecture_inspection_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p ath --test documentation_architecture_cli --locked
cargo fmt --all -- --check
```

Slices 0A–1C1 are execution-confirmed; Slice 1C2 CLI source is implemented and pending its exact matrix.
