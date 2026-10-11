---
id: doc://docs/development/code-intelligence-parity-plan.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Code Intelligence Parity And Operational Documentation Plan

This plan records the target state for code-intelligence parity with the CodeGraph-class local graph
tools and for the operational documentation surface, plus the slice order that reaches it. It is a
source-state document: a slice is only `Verified` when an exact-commit execution matrix says so, and
that state is recorded in `roadmap-status.md` and `athanor_implementation_plan_ru.md`.

## Goal

Give an AI agent the repository's functional schematic before it opens the detailed circuit
schematic, for the languages this project actually owns, and let the same canonical model publish
operational documentation for the people who run the system, not only developer documentation.

Two concrete targets:

1. **Two-language parity.** Rust and JavaScript/TypeScript reach the same depth as a CodeGraph-class
   index: cross-file symbol resolution, a traversable call graph, implementation edges, and a
   single bounded query that returns relevant source with the call path and blast radius.
2. **Operational documentation.** Alongside the developer-oriented profiles, publish an ops manual
   (deployment, configuration and environment reference, service topology, health and monitoring,
   backup and restore, upgrade and rollback, runbook) from the same canonical evidence.

## Non-Goals

- Language breadth for its own sake. Breadth beyond Rust and JS/TS is a bounded, evidence-driven
  expansion (Slice P3), never a 30-language surface maintained without regression evidence.
- LLM-authored narrative documentation. The documentation profiles stay deterministic and
  citation-bound; a model may draft prose only as an explicitly separate projection.
- A hosted service. Everything here runs locally and offline.

## Slice Order

### P1 — JavaScript/TypeScript Call Graph (source implementation complete)

- Extract import bindings (`default_name`, `namespace_name`, `named`) per import statement.
- Extract per-declaration static call sites (callee path, receiver, kind, argument count, position).
- Register `builtin.linker.js_ts_calls`, resolving same-module, `this.` member, imported-binding,
  and unique-name targets into `Calls` relations, with `TestedBy` for test-location callers.
- Bounds: 256 call sites per declaration, 10,000 call relations per snapshot, deterministic order.

### P2 — Resolution Depth For Both Languages

- Rust: trait implementations and dyn-dispatch candidates (`Implements` edges).
- JS/TS: class `implements`, callbacks and promise handlers, re-export chains, path aliases, and
  workspace-package specifiers.
- Both: symbol-level incremental invalidation so a changed declaration re-resolves only its closure.

### P3 — Bounded Language Expansion

- A generic tree-sitter query engine behind the existing `Extractor` port, with per-language query
  packs so a new language is data plus a test fixture, not a new crate.
- First targets: Python and Go, each with a published cross-file coverage measurement.

### P4 — Operational Documentation Profile

- A deterministic `ops` profile over existing canonical evidence: operations files, environment
  references, scripts, installers, CI jobs, service/bootstrap commands, and API contracts.
- Sections: deployment, configuration and environment reference, service topology, health and
  monitoring, backup and restore, upgrade and rollback, runbook steps.
- Same publication model as the other profiles: exact snapshot binding, hard limits with disclosed
  omissions, citations to evidence, immutable generations, and validated inspection.

### P5 — Agent Surface

- One bounded query that returns relevant declarations' source with the paths between them and the
  blast radius of a change, defaulting the MCP surface to that tool plus orientation.

## Verification Requirements

- Every slice ships code, focused tests, adapter documentation, and a changelog entry.
- Adapter docs describe semantics and boundaries; they never claim execution evidence.
- `Verified` requires an exact-commit matrix: `cargo fmt --check`, workspace tests `--locked`,
  Clippy `-D warnings`, indexing smoke, and the focused test the slice adds, on one commit.
- Coverage claims are measured on real repositories with a stated denominator, not asserted.

## Current Status

Slice P1 is source-implemented and carries focused unit tests in `athanor-extractor-js-ts` and
`athanor-linker-js-ts`; its exact-commit verification is pending and is tracked in
`roadmap-status.md`. Slices P2–P5 remain planned.
