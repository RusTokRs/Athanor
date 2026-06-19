# AGENTS.md

This is the entrypoint for agents working on Athanor.

Read this file before changing code or documentation.

## Required Reading

Before implementation work:

1. Read this file.
2. Read `docs/development/agent-workflow.md`.
3. Read `docs/development/roadmap-status.md`.
4. If touching indexing/runtime, read `docs/architecture/pipeline.md`.
5. If adding or changing an adapter/plugin, read `docs/architecture/adapters.md`.
6. If editing an existing adapter crate, read that crate's `README.md`.

## Core Rules

- Prefer adapter-first design.
- Keep `athanor-domain` and `athanor-core` free of adapter-specific details.
- Do not duplicate stable ID generation, evidence builders, JSONL writer, path normalization, or registry logic.
- Facts, relations, and diagnostics must include evidence.
- Documentation is part of implementation. Update English docs in the same task.
- Do not mark work complete until verification commands pass.

## Required Verification

For code changes:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
```

For indexing/runtime changes also run:

```bash
cargo run -p ath --quiet -- index .
```

## Current Next Step

Check `docs/development/roadmap-status.md`.
