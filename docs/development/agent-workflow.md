---
id: doc://docs/development/agent-workflow.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000090
source_language: en
status: verified
---
# Agent Workflow

This document defines the working procedure for agents implementing Athanor.

## 1. Read Before Coding

Before changing files, read:

1. `AGENTS.md`
2. `docs/README.md`
3. `docs/development/roadmap-status.md`
4. Relevant architecture docs under `docs/architecture/`
5. Relevant adapter docs under `docs/adapters/`
6. Relevant crate `README.md`

Use `start.md` as the full architectural plan, not as the daily task checklist.

## 2. Plan The Task

Before implementation, identify:

- roadmap section or current status item
- crates likely to change
- whether the change belongs in core/domain or an adapter
- tests and runtime checks needed
- documentation files that must be updated

Use a short task plan with statuses:

```text
planned
in_progress
implemented
verified
deferred
blocked
```

Do not mark a task `verified` without running checks.

## 3. Adapter-First Decision

Ask:

```text
Is this Athanor domain meaning,
or a way to read, write, store, search, transport, or display knowledge?
```

If it is format-specific, backend-specific, transport-specific, UI-specific, framework-specific, or service-specific, implement it as an adapter crate.

Change `athanor-domain` or `athanor-core` only when the concept remains valid after replacing adapters.

## 4. Modularity Checklist

Before adding a feature, check:

- Can it be an isolated crate?
- Can implementation be replaced without changing domain/core?
- Can it work without Rustok?
- Can it work without MCP?
- Can it work offline?
- Are inputs and outputs explicit?
- Do emitted facts, relations, or diagnostics have evidence?
- Is there a useful unit or contract test?

If the answer is no, document why.

## 5. Documentation By Default

Documentation is required in the same task when changing:

- crate structure
- adapter behavior
- plugin behavior
- pipeline steps
- CLI behavior
- generated artifacts
- roadmap status
- definition of done

Update the relevant files:

- `docs/development/roadmap-status.md`
- `docs/README.md`
- `docs/architecture/pipeline.md`
- `docs/architecture/adapters.md`
- `docs/adapters/*.md`
- crate `README.md`

New adapter crates must have a `README.md`.

If documentation is added, renamed, removed, or its purpose changes, update `docs/README.md` in the same task.

## 6. Verification

Run for code changes:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
```

Run for indexing/runtime changes:

```bash
cargo run -p ath --quiet -- index .
```

For docs-only changes, no Rust checks are required unless code or generated behavior changed.

## 7. Completion Note

Final reports should include:

- what changed
- which crates/docs changed
- verification commands run
- generated output changes, if relevant
- known limitations
- recommended next step

Do not hide technical debt. If something is intentionally deferred, say so.
