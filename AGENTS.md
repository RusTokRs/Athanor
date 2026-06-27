# AGENTS.md

This is the entrypoint for agents working on Athanor.

Read this file before changing code or documentation.

## Required Reading

Before implementation work:

1. Read this file.
2. Read `docs/README.md`.
3. Read `docs/development/agent-workflow.md`.
4. Read `docs/development/roadmap-status.md`.
5. If touching indexing/runtime, read `docs/architecture/pipeline.md`.
6. If adding or changing an adapter/plugin, read `docs/architecture/adapters.md`.
7. If editing an existing adapter crate, read that crate's `README.md`.

## Core Rules

- Prefer adapter-first design.
- While Athanor is primarily being shaped for Rustok, use Rustok-first iterative development: make the smallest useful change, verify it locally, run bounded Athanor commands against the real Rustok repository when available, tune signal quality, and only then expand scope.
- Keep `athanor-domain` and `athanor-core` free of adapter-specific details.
- Do not duplicate stable ID generation, evidence builders, JSONL writer, path normalization, or registry logic.
- Facts, relations, and diagnostics must include evidence.
- Documentation is part of implementation. Update English docs in the same task.
- If documentation is added, renamed, removed, or its purpose changes, update `docs/README.md` in the same task.
- Do not make agents read large generated artifacts directly. Generated JSONL, wiki, HTML, and similar outputs are backing read models or inspection outputs; agent-facing access must go through bounded query/context commands with explicit limits, stable schemas, canonical ids, and evidence links.
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

## Companion Repositories

- `D:\DartScope` is planned as a separate Git repository for the long-lived, community-facing Rust
  toolkit for Dart and Flutter code intelligence. It should stay outside the Athanor workspace.
- Athanor should only contain an adapter wrapper, such as `athanor-extractor-dart-flutter`, that
  consumes DartScope and maps its output into Athanor canonical entities, facts, relations,
  diagnostics, evidence, ownership, and stable keys.
- Do not merge DartScope parser/analyzer implementation into Athanor. Keep the reusable library
  independent so it can be developed, documented, released, and used by the wider Dart/Flutter
  community without depending on Athanor.
