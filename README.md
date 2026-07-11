# Athanor

[![CI](https://github.com/RusTokRs/Athanor/actions/workflows/ci.yml/badge.svg)](https://github.com/RusTokRs/Athanor/actions/workflows/ci.yml)

Athanor is a Rust-based code knowledge engine for AI agents and developers.

It builds an incremental, evidence-backed knowledge model for a repository by discovering source files, extracting canonical facts, linking related objects, running checks, and exporting disposable read models.

Unlike a conventional code graph viewer, Athanor treats the graph as one projection of a
canonical knowledge model. Code, documentation, OpenAPI contracts, operational files, diagnostics,
and generated reports are tied together through stable ids, source evidence, and ownership metadata
so agents can ask bounded questions without losing the trail back to source files.

A useful mental model: Athanor gives an agent the repository's functional schematic before it opens
the detailed circuit schematic. It answers practical orientation questions first: where is this
block, what is it connected to, what influences it, what can it break, where is the documentation,
which diagnostics already exist, and what is the minimum set of files worth reading. The source code
remains the detailed schematic, but the agent opens it only after it has a focused map of the system.
This is how Athanor is intended to reduce token use: read the system map first, then inspect only
the relevant slice of implementation detail.

| Capability | Conventional code graph tools | Athanor |
| --- | --- | --- |
| Source of truth | Graph or search index | Canonical snapshots with evidence and ownership |
| Main purpose | Navigate code structure and dependencies | Verify, query, explain, and project repository knowledge |
| Inputs | Usually source code and symbols | Code, Markdown docs, OpenAPI, runtime configuration, operations files, and adapters |
| Agent context | Retrieval or graph lookup | Bounded `context`, `explain`, `impact`, and `check` queries |
| Documentation | Optional generated docs or summaries | Docs drift checks, completeness gates, and reviewable patch proposals |
| API contracts | Usually outside the core model | API registry, snapshots, diffs, breaking-change diagnostics, and strict checks |
| Outputs | Graph UI or export | Rebuildable JSONL, wiki, HTML report, search index, and future graph views |
| Extensibility | Tool-specific plugins | Adapter-first ports for sources, extractors, linkers, checkers, stores, search, projectors, and transports |

## Current Status

Athanor currently ships a verified local-first knowledge-engine baseline:

```text
local files
  -> file, Markdown, OpenAPI, and Rust extraction
  -> Markdown containment, document references, and cross-source API links
  -> Markdown structure, unresolved reference, duplicate ID, and API consistency checks
  -> JSONL canonical store
  -> overview, context, explain, and scoped diagnostic queries
  -> configurable editable-documentation completeness gate
  -> coordinated JSONL, Markdown wiki, and HTML read models
  -> immutable snapshot-consistent generation pointers
```

Built-in and external-process adapters share one runtime registry, indexing is incremental across
durable snapshots, and the GitHub Actions matrix enforces the workspace checks on Linux, Windows,
and macOS. The implementation remains intentionally adapter-first: domain and core crates define
canonical types and ports, while format, storage, query presentation, and projection behavior stay
outside those boundaries.

The current baseline is an offline CLI with local JSONL canonical storage, repository overview,
lexical search, impact analysis, static projectors, MCP transport, an explicit user-level
repository identity registry, and an initial local daemon lifecycle/read-only protocol slice.
Production multi-process storage, daemon job orchestration, semantic vectors, and daemon-served
multi-repository workflows remain roadmap work.

## Quick Start

Build and test the workspace:

```bash
cargo test --workspace --quiet
```

Index the current project:

```bash
cargo run -p ath --quiet -- index .
```

Validate the effective configuration or inspect local backend/adapter compatibility:

```bash
cargo run -p ath --quiet -- config validate --path .
cargo run -p ath --quiet -- config doctor --path . --json
```

The default CLI build uses the portable JSONL store. SurrealDB is optional and must be enabled
explicitly when a project selects a `surreal-*` storage mode:

```bash
cargo run -p ath --features store-surreal -- index .
```

Validate adapter contracts without writing snapshots, state, or read models:

```bash
cargo run -p ath --quiet -- index . --validate-only
```

Summarize the latest canonical snapshot for repository orientation:

```bash
cargo run -p ath --quiet -- overview .
cargo run -p ath --quiet -- overview . --json
```

Build a task-focused context pack from the latest canonical snapshot:

```bash
cargo run -p ath --quiet -- context "change authentication"
cargo run -p ath --quiet -- context "change authentication" --json
```

For unattended local operation, register the project and install the authenticated per-user daemon:

```bash
cargo run -p ath --quiet -- projects add athanor .
cargo run -p athd --quiet -- service install athanor --transport local-socket --watch
cargo run -p athd --quiet -- doctor athanor --json
```

See [Production operation](docs/development/production.md) for the security model, runtime paths,
service management, external-adapter policy, and signed release verification.

Explain one canonical entity with its facts, relations, evidence, and diagnostics:

```bash
cargo run -p ath --quiet -- explain "api://POST:/login"
cargo run -p ath --quiet -- explain "api://POST:/login" --json
```

Inspect open API or documentation diagnostics from the latest snapshot:

```bash
cargo run -p ath --quiet -- check api
cargo run -p ath --quiet -- check docs --json
```

Run the CI-oriented completeness gate for editable documentation:

```bash
cargo run -p ath --quiet -- docs check
cargo run -p ath --quiet -- docs check --json
cargo run -p ath --quiet -- docs drift
cargo run -p ath --quiet -- docs drift --json
cargo run -p ath --quiet -- api snapshot
cargo run -p ath --quiet -- api diff --from <snapshot> --to <snapshot>
cargo run -p ath --quiet -- api breaking-changes --from <snapshot> --to <snapshot>
cargo run -p ath --quiet -- check api --strict
```

Build the neutral Markdown wiki from the latest canonical snapshot:

```bash
cargo run -p ath --quiet -- wiki .
```

Build the static HTML report:

```bash
cargo run -p ath --quiet -- report html .
```

Publish JSONL, wiki, and HTML as one immutable snapshot-consistent generation:

```bash
cargo run -p ath --quiet -- generate .
```

Coordinated read models are written under `.athanor/generated/generations/<generation>` and selected through `.athanor/generated/current.json`. The individual commands retain their direct `.athanor/generated/current/*` compatibility outputs. Durable canonical snapshots are written under `.athanor/store/canonical/jsonl`.

## Documentation

- [Documentation map](docs/README.md): where to find architecture, development workflow, adapter docs, and generated artifact notes.
- [Agent entrypoint](AGENTS.md): required instructions for agents changing code or documentation.
- [Roadmap status](docs/development/roadmap-status.md): implemented work, current status, and next recommended task.
- [Full architectural plan](start.md): long-form product and architecture direction.

## Verification

GitHub Actions runs the verification matrix on Linux, Windows, and macOS with Rust 1.95. See the
[CI contract](docs/development/ci.md).

For code changes, run:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
```

For indexing/runtime changes, also run:

```bash
cargo run -p ath --quiet -- index .
```
