---
id: doc://docs/development/agent-workflow.md
kind: developer_guide
language: en
last_verified_snapshot: snap_jsonl_00000251
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

## 3. Rustok-First Iteration

Until Athanor's internal Rustok workflows are stable, prefer short Rustok-facing iterations over
large speculative implementation batches:

1. Choose one concrete Rustok scenario or pain point.
2. Implement the smallest useful Athanor-side slice.
3. Run the required local verification for the files changed.
4. When the real Rustok repository is available, run the narrowest useful probe first. For
   changed frontend/parser slices, prefer `ath validate-changed --path <repo> --file <path>`
   for the exact file under repair, then `ath validate-changed --path <repo>` for the full changed
   set before a writable index; it extracts only selected or changed files and reports extractor
   diagnostics without writing a snapshot or read model. Then run bounded commands such as
   `ath index`, `ath overview`,
   `ath coverage`, `ath check rustok-*`, `ath rustok ... audit`, `ath graph ...`, and
   `ath context ...` when the slice needs full-graph validation.
5. Choose the real-repository probes by the surface being changed. Rust backend and Rust-native
   frontend work should emphasize Rust/FBA/FFA/Page Builder graph and diagnostic commands. Next.js
   frontend work should emphasize JavaScript/TypeScript coverage, context, search, and any
   Next/Page Builder-specific adapter commands that exist for that slice. Documentation and
   operations changes should emphasize docs, affected, drift, and operations checks.
6. Evaluate signal quality before expanding: diagnostics noise, evidence accuracy, stable keys,
   graph usefulness, context-pack usefulness, runtime cost, and incremental rebuild behavior.
7. Fix the slice until it is useful for real Rustok work, then expand to the next adjacent slice.

Do not rely on reading generated JSONL, wiki, HTML, or graph exports to judge a Rustok iteration.
Use bounded CLI or future daemon/MCP commands.

## 4. Adapter-First Decision

Ask:

```text
Is this Athanor domain meaning,
or a way to read, write, store, search, transport, or display knowledge?
```

If it is format-specific, backend-specific, transport-specific, UI-specific, framework-specific, or service-specific, implement it as an adapter crate.

Change `athanor-domain` or `athanor-core` only when the concept remains valid after replacing adapters.

## 5. Modularity Checklist

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

## 6. Documentation By Default

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

## 7. Agent-Facing Output Rule

Athanor must not require agents to read large generated artifacts directly.

Generated JSONL, Markdown wiki, HTML reports, graph exports, API artifacts, and future search/vector
read models are backing read models or inspection outputs. Agent-facing access must use bounded
query or context commands with:

- explicit file, entity, diagnostic, relation, or token limits
- stable JSON schemas for machine use
- canonical ids and stable keys
- source anchors and evidence links
- omitted/truncated counts when limits hide available data

If a feature creates or depends on a large generated artifact, it is not complete for agent use until
there is a bounded command or API that returns the relevant slice without requiring full-artifact
reading.

## 8. Verification

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

## 9. Completion Note

Final reports should include:

- what changed
- which crates/docs changed
- verification commands run
- generated output changes, if relevant
- known limitations
- recommended next step

Do not hide technical debt. If something is intentionally deferred, say so.
