---
id: doc://docs/README.md
kind: project_overview
language: en
last_verified_snapshot: snap_jsonl_00000090
source_language: en
status: verified
---

# Athanor Documentation Map

This is the documentation index for Athanor.

Use this file to find the right document before changing code, adapters, plugins, pipeline behavior, or generated artifacts.

## Entry Points

- [Project overview](../README.md): concise product description, quick start, and verification commands.
- [Agent entrypoint](../AGENTS.md): required first read for agents.
- [Full architectural plan](../start.md): long-form product and architecture plan.
- [Agent workflow](development/agent-workflow.md): required implementation workflow.
- [Roadmap status](development/roadmap-status.md): what is implemented, in progress, and next.

## Architecture

- [Indexing pipeline](architecture/pipeline.md): source discovery, extraction, linking, checking, storage, and JSONL export.
- [Adapter architecture](architecture/adapters.md): adapter-first boundaries and current adapter map.

## Development

- [Agent workflow](development/agent-workflow.md): read-before-coding, planning, documentation, verification, completion notes.
- [Definition of done](development/definition-of-done.md): required checks and documentation expectations.
- [Continuous integration](development/ci.md): GitHub Actions matrix, commands, and security defaults.
- [Library adoption plan](development/library-adoption-plan.md): approved dependencies, adapter boundaries, risks, and acceptance criteria.
- [Documentation completeness policy](development/docs-completeness-policy.md): `athanor.toml` policy fields and the `ath docs check` CI gate.
- [Roadmap status](development/roadmap-status.md): current verified implementation status and next recommended task.

## Operations Documentation

Environment variables:

- [CARGO_PKG_VERSION](operations/env-cargo-pkg-version.md)
- [CARGO_TERM_COLOR](operations/env-cargo-term-color.md)
- [RUST_BACKTRACE](operations/env-rust-backtrace.md)

GitHub Actions workflows and jobs:

- [CI workflow](operations/script-script-command-github-workflows-ci-yml-github-actions-workflow.md)
- [CI quality job](operations/script-script-command-github-workflows-ci-yml-github-actions-job-quality.md)
- [CI security job](operations/script-script-command-github-workflows-ci-yml-github-actions-job-security.md)
- [Security audit workflow](operations/script-script-command-github-workflows-security-yml-github-actions-workflow.md)
- [Security audit job](operations/script-script-command-github-workflows-security-yml-github-actions-job-audit.md)

GitHub Actions steps:

- [CI quality checkout step](operations/script-script-command-github-workflows-ci-yml-github-actions-job-quality-step-1-uses.md)
- [CI quality toolchain step](operations/script-script-command-github-workflows-ci-yml-github-actions-job-quality-step-2-uses.md)
- [CI quality cache step](operations/script-script-command-github-workflows-ci-yml-github-actions-job-quality-step-3-uses.md)
- [CI formatting step](operations/script-script-command-github-workflows-ci-yml-github-actions-job-quality-step-4-run.md)
- [CI workspace tests step](operations/script-script-command-github-workflows-ci-yml-github-actions-job-quality-step-5-run.md)
- [CI Clippy step](operations/script-script-command-github-workflows-ci-yml-github-actions-job-quality-step-6-run.md)
- [CI indexing smoke step](operations/script-script-command-github-workflows-ci-yml-github-actions-job-quality-step-7-run.md)
- [CI docs check step](operations/script-script-command-github-workflows-ci-yml-github-actions-job-quality-step-8-run.md)
- [CI security checkout step](operations/script-script-command-github-workflows-ci-yml-github-actions-job-security-step-1-uses.md)
- [CI cargo-deny step](operations/script-script-command-github-workflows-ci-yml-github-actions-job-security-step-2-uses.md)
- [Security audit checkout step](operations/script-script-command-github-workflows-security-yml-github-actions-job-audit-step-1-uses.md)
- [Security audit run step](operations/script-script-command-github-workflows-security-yml-github-actions-job-audit-step-2-uses.md)

## Adapter Documentation

High-level adapter docs:

- [Basic extractor](adapters/extractor-basic.md)
- [Markdown extractor](adapters/extractor-markdown.md)
- [OpenAPI extractor](adapters/extractor-openapi.md)
- [Operations extractor](adapters/extractor-operations.md)
- [Rust extractor](adapters/extractor-rust.md)
- [API knowledge linker](adapters/linker-api.md)
- [Markdown linker](adapters/linker-markdown.md)
- [Rust linker](adapters/linker-rust.md)
- [Markdown checker](adapters/checker-markdown.md)
- [API consistency & environment checker](adapters/checker-api.md)
- [Code impact analysis](adapters/impact.md)
- [JSONL store](adapters/store-jsonl.md)
- [SurrealDB store](adapters/store-surrealdb.md)
- [Tantivy search index](adapters/search-tantivy.md)
- [Markdown wiki projector](adapters/projector-wiki.md)
- [Model Context Protocol (MCP) transport](adapters/transport-mcp.md)
- [HTML report projector](adapters/projector-html.md)

Crate-local adapter docs:

- [`athanor-extractor-basic`](../crates/athanor-extractor-basic/README.md)
- [`athanor-extractor-markdown`](../crates/athanor-extractor-markdown/README.md)
- [`athanor-extractor-openapi`](../crates/athanor-extractor-openapi/README.md)
- [`athanor-extractor-operations`](../crates/athanor-extractor-operations/README.md)
- [`athanor-extractor-rust`](../crates/athanor-extractor-rust/README.md)
- [`athanor-linker-api`](../crates/athanor-linker-api/README.md)
- [`athanor-linker-markdown`](../crates/athanor-linker-markdown/README.md)
- [`athanor-linker-rust`](../crates/athanor-linker-rust/README.md)
- [`athanor-checker-markdown`](../crates/athanor-checker-markdown/README.md)
- [`athanor-checker-api`](../crates/athanor-checker-api/README.md)
- [`athanor-store-jsonl`](../crates/athanor-store-jsonl/README.md)
- [`athanor-store-surrealdb`](../crates/athanor-store-surrealdb/README.md)
- [`athanor-search-tantivy`](../crates/athanor-search-tantivy/README.md)
- [`athanor-projector-wiki`](../crates/athanor-projector-wiki/README.md)
- [`athanor-projector-html`](../crates/athanor-projector-html/README.md)
- [`athanor-projector-support`](../crates/athanor-projector-support/README.md)

## Current Generated Read Models

The coordinated generation command publishes all read models from one canonical snapshot:

```bash
cargo run -p ath --quiet -- generate .
```

Local generated and canonical artifacts can be inspected without modification:

```bash
cargo run -p ath --quiet -- repair inspect .
cargo run -p ath --quiet -- repair inspect . --json
cargo run -p ath --quiet -- repair cleanup . --dry-run
cargo run -p ath --quiet -- repair cleanup . --dry-run --keep-canonical 3 --keep-generated 2
cargo run -p ath --quiet -- repair cleanup .
cargo run -p ath --quiet -- repair regenerate . --dry-run
cargo run -p ath --quiet -- repair regenerate .
cargo run -p ath --quiet -- repair recover-canonical . --dry-run
cargo run -p ath --quiet -- repair recover-canonical .
cargo run -p ath --quiet -- repair apply . --dry-run
cargo run -p ath --quiet -- repair apply . --dry-run --keep-canonical 3 --keep-generated 2
cargo run -p ath --quiet -- repair apply .
```

`repair regenerate` handles stale or corrupted generated-current selection by publishing a fresh
coordinated generation. It covers invalid pointer paths, missing selected generation directories,
and missing or mismatched current generation manifests.

It writes immutable generation directories and updates a portable JSON pointer only after every output succeeds:

```text
.athanor/generated/
  current.json
  generations/
    00000001/
      manifest.json
      jsonl/
      wiki/
      html/
```

`current.json` records the generation id, canonical snapshot id, relative generation path, and manifest path. Consumers should resolve coordinated outputs through this pointer.

The individual commands retain direct compatibility outputs under `.athanor/generated/current`.

The current CLI uses `JsonlReadModelWriter` to write generated JSONL read models to:

```text
.athanor/generated/current/jsonl/
  entities.jsonl
  facts.jsonl
  relations.jsonl
  diagnostics.jsonl
  manifest.json
```

Generated files are not source documentation. They are disposable read models. Adapter validation failures are written to `.athanor/generated/current/validation-report.json` by default. Successful `--validate-only` runs write `.athanor/generated/current/validation-result.json` by default. The CLI also persists incremental file-change state at `.athanor/state/index-state.json`.

The CLI stores durable canonical snapshots at:

```text
.athanor/store/canonical/jsonl/
  latest.json
  snapshots/<snapshot-id>/
```

Adapter contracts can be checked without writing snapshots, state, or read models:

```bash
cargo run -p ath --quiet -- index . --validate-only
cargo run -p ath --quiet -- index . --validate-only --validation-result .athanor/generated/current/validation-result.json
```

Detailed indexing diagnostics can be enabled through `RUST_LOG` without changing command output:

```bash
RUST_LOG=athanor_app=info cargo run -p ath --quiet -- index .
RUST_LOG=athanor_app=debug cargo run -p ath --quiet -- index .
```

Tracing output is written to `stderr`, so JSON and normal command output remain on `stdout`.

Changed files can be committed into a fresh durable snapshot through the explicit update command:

```bash
cargo run -p ath --quiet -- update . --changed
cargo run -p ath --quiet -- update . --changed --json
```

Repository overview and task-focused context packs can be read from the latest canonical snapshot:

```bash
cargo run -p ath --quiet -- overview .
cargo run -p ath --quiet -- overview . --json
cargo run -p ath --quiet -- context "task"
cargo run -p ath --quiet -- context "task" --json
cargo run -p ath --quiet -- context --diff
cargo run -p ath --quiet -- context --diff --json
cargo run -p ath --quiet -- context "task" --level summary --budget 2000
cargo run -p ath --quiet -- context "task" --level deep --max-files 20 --max-depth 2
cargo run -p ath --quiet -- graph export --format json
cargo run -p ath --quiet -- graph export --format graphml
cargo run -p ath --quiet -- graph related "api://GET:/health"
cargo run -p ath --quiet -- graph related "api://GET:/health" --depth 2 --json
cargo run -p ath --quiet -- graph path "doc://docs/api/health.md" "rust://src/lib.rs#health"
cargo run -p ath --quiet -- graph path "doc://docs/api/health.md" "rust://src/lib.rs#health" --max-depth 4 --json
cargo run -p ath --quiet -- graph hubs
cargo run -p ath --quiet -- graph hubs --kind module --limit 10 --json
cargo run -p ath --quiet -- graph cycles
cargo run -p ath --quiet -- graph cycles --max-depth 6 --limit 10 --json
```

Canonical entities can be explained directly from the latest snapshot:

```bash
cargo run -p ath --quiet -- explain "api://POST:/login"
cargo run -p ath --quiet -- explain "api://POST:/login" --json
```

Open canonical diagnostics can be inspected by scope:

```bash
cargo run -p ath --quiet -- check api
cargo run -p ath --quiet -- check docs --json
cargo run -p ath --quiet -- check env
cargo run -p ath --quiet -- check env --json
cargo run -p ath --quiet -- check scripts
cargo run -p ath --quiet -- check scripts --json
cargo run -p ath --quiet -- check deployment
cargo run -p ath --quiet -- check deployment --json
cargo run -p ath --quiet -- check runbooks
cargo run -p ath --quiet -- check runbooks --json
cargo run -p ath --quiet -- check affected
cargo run -p ath --quiet -- check affected --json
```

`check affected` is read-only. In addition to matching open diagnostics, it reports stale or
potentially stale local artifacts tied to the latest canonical snapshot: coordinated generated
generations, direct wiki and HTML report outputs, API contract latest pointers, and API diff
directories. It also reports editable documentation drift only for affected documents whose
`last_verified_snapshot` does not match the latest canonical snapshot. It suggests explicit
follow-up commands but does not regenerate, rewrite, or delete artifacts.

Editable documentation can be checked against the project completeness policy:

```bash
cargo run -p ath --quiet -- docs check
cargo run -p ath --quiet -- docs check --json
cargo run -p ath --quiet -- docs drift
cargo run -p ath --quiet -- docs drift --json
cargo run -p ath --quiet -- docs propose-fix
cargo run -p ath --quiet -- docs apply-patch <patch-id-or-path>
cargo run -p ath --quiet -- docs operations check
cargo run -p ath --quiet -- docs operations check --json
cargo run -p ath --quiet -- api snapshot
cargo run -p ath --quiet -- api diff --from <snapshot> --to <snapshot>
cargo run -p ath --quiet -- api breaking-changes --from <snapshot> --to <snapshot>
cargo run -p ath --quiet -- api cleanup --dry-run
cargo run -p ath --quiet -- api cleanup --keep-snapshots 2 --keep-diffs 2
cargo run -p ath --quiet -- api registry
cargo run -p ath --quiet -- api registry --json
cargo run -p ath --quiet -- check api --strict
```

API contract snapshots and diffs are managed separately from `ath index` so contract history is not
deleted accidentally during frequent re-indexing. Use `ath api cleanup` to apply explicit retention;
the latest API contract snapshot is always retained.

The latest canonical snapshot can be projected into a disposable Markdown wiki:

```bash
cargo run -p ath --quiet -- wiki .
```

The wiki is written to `.athanor/generated/current/wiki` by default.

A self-contained browser report can be generated from the same snapshot:

```bash
cargo run -p ath --quiet -- report html .
```

The report is written to `.athanor/generated/current/html` by default.

## Documentation Rule

When code changes, documentation must be updated in the same task.

Typical mapping:

| Change | Update |
| --- | --- |
| Pipeline behavior | `architecture/pipeline.md`, `development/roadmap-status.md` |
| Adapter/plugin behavior | `architecture/adapters.md`, `docs/adapters/*.md`, crate `README.md` |
| CLI behavior | `development/roadmap-status.md`, relevant architecture doc |
| Definition of done or workflow | `development/agent-workflow.md`, `development/definition-of-done.md`, `AGENTS.md` |
| Roadmap progress | `development/roadmap-status.md` |

If documentation is added, renamed, removed, or its purpose changes, update this index in the same task.
