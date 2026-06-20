# Athanor

Athanor is a Rust-based code knowledge engine for AI agents and developers.

It builds an incremental, evidence-backed knowledge model for a repository by discovering source files, extracting canonical facts, linking related objects, running checks, and exporting disposable read models.

## Current Status

Athanor currently ships a verified indexing vertical slice:

```text
local files
  -> file and Markdown extraction
  -> Markdown containment links
  -> Markdown structure diagnostics
  -> JSONL canonical store
  -> JSONL read model
```

The implementation is intentionally adapter-first. Domain and core crates define canonical types and ports; source discovery, extraction, linking, checking, and storage live in adapter crates.

## Quick Start

Build and test the workspace:

```bash
cargo test --workspace --quiet
```

Index the current project:

```bash
cargo run -p ath --quiet -- index .
```

Validate adapter contracts without writing snapshots, state, or read models:

```bash
cargo run -p ath --quiet -- index . --validate-only
```

Build a task-focused context pack from the latest canonical snapshot:

```bash
cargo run -p ath --quiet -- context "change authentication"
cargo run -p ath --quiet -- context "change authentication" --json
```

Generated read models are written under `.athanor/generated/current/jsonl`. Durable canonical snapshots are written under `.athanor/store/canonical/jsonl`.

## Documentation

- [Documentation map](docs/README.md): where to find architecture, development workflow, adapter docs, and generated artifact notes.
- [Agent entrypoint](AGENTS.md): required instructions for agents changing code or documentation.
- [Roadmap status](docs/development/roadmap-status.md): implemented work, current status, and next recommended task.
- [Full architectural plan](start.md): long-form product and architecture direction.

## Verification

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
