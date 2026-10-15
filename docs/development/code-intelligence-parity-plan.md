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

- Rust: trait implementation (`Implements`) and supertrait (`Extends`) edges; dyn-dispatch candidates
  remain a separate deferred capability.
- JS/TS: class `extends`/`implements` and interface `extends` with distinct canonical relation kinds;
  P2B covers explicit named re-export chains, while callbacks and promise handlers, wildcard/default
  re-exports, path aliases, and workspace-package specifiers remain open.
- Both: symbol-level incremental invalidation so a changed declaration re-resolves only its closure.

#### P2A — Heritage Relation Semantics (bounded Rust/TypeScript slice)

- Add canonical `RelationKind::Extends`; keep `Implements` for Rust `impl Trait for Type` and JS/TS
  class `implements` bridges.
- Extract TypeScript interface parent types and Rust trait supertraits; Rust trait method declaration
  extraction remains a separate deferred slice.
- Propagate changes from a base class/trait backward to derived types and implementations, rank
  `Extends` with the other implementation edges, and include it in the architecture profile.
- Bump both index-state schema families from v46 to v47, so old entity payloads are not incorrectly
  reused after extractor semantics change (normal and `js-ts-precision` builds).

Focused source regression matrix:

| Language | Extractor | Linker | Application behavior |
| --- | --- | --- | --- |
| Rust | trait `supertraits` paths plus containing-module imports | unambiguous local/imported paths emit `Extends`; ambiguous glob targets stay unresolved; `trait_impl` remains `Implements` | base-trait changes reach implementing/derived declarations |
| TypeScript | class `extends`/`implements` plus multiple interface `extends` names | class/interface inheritance emits `Extends`; ambiguous class/interface targets stay unresolved; class contracts emit `Implements` | base-class changes reach subclasses; `Extends` receives high change-map priority |
| Shared persistence | — | — | v46 index state is discarded and forces a full rebuild; v47 fixtures/contracts agree |

Focused verification matrix:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

The full exact-commit verification matrix remains the workspace test, format, Clippy, and indexing
smoke described below.

#### P2B — Explicit Named Re-export Chains (bounded JS/TS slice)

- Preserve both imported/local names for named imports and imported/exported names for explicit
  named export clauses in the normalized module payload; retain the existing local-name `named` list.
- Materialize exact relative source-backed re-exports as verified module `Imports` relations.
- Resolve calls through local export aliases and chains of explicit relative named re-exports, up to
  32 hops. Cycles, ambiguous mappings, unresolved sources, and deeper chains stay unresolved; an
  unresolved explicit import must not fall through to a snapshot-wide name guess.
- Keep `export *`, `export * as namespace`, default-export name resolution, package/workspace imports,
  TypeScript path aliases, receiver typing, and callback/promise-handler resolution out of this slice.
- Bump both index-state schema families from v47 to v48 so cached module payloads are rebuilt in
  normal and `js-ts-precision` configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | Named import aliases preserve imported/local names; local and source-backed export clauses preserve imported/exported names and source ranges |
| Import linker | A relative source-backed re-export emits a verified `Imports` edge with re-export evidence |
| Call linker | Multi-hop named re-exports plus an aliased import resolve to the original declaration; unresolved sources do not fall through to a global guess |
| Persistence | v47 normal/precision state is rejected and forces a v48 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### Remaining P2 Gaps

- JavaScript/TypeScript callbacks and promise handlers; wildcard/default re-exports; package/workspace
  specifiers and TypeScript path aliases; receiver/type-aware dispatch.
- Rust dyn-dispatch candidates and symbol-level incremental invalidation for both Rust and JS/TS.

### P3 — Bounded Language Expansion

- A generic tree-sitter query engine behind the existing `Extractor` port, with per-language query
  packs so a new language is data plus a test fixture, not a new crate.
- Shipped: the engine plus Python and Go packs as `builtin.extractor.treesitter`, and cross-file
  resolution as `builtin.linker.treesitter`: module imports (dotted, relative, and package-suffix),
  calls through resolution units and import bindings, and test bridges, each with a stated confidence.
- Published coverage measurement: `resolution_coverage` reports, per language, recorded import
  statements and call sites inside declaration bodies against the resolved ones, with module-level call
  sites counted separately instead of lowering the denominator; the crate corpus pins the numbers.
- Shipped after the first linker slice: Go methods are identified by their receiver type
  (`func (s *Server) Serve` is `#Server.Serve` with `receiver_binding: "s"`), so same-named methods of
  different types in one file stay distinct and a call through the caller's own receiver resolves at the
  own-instance step instead of the unique-member fallback.
- Remaining: receiver typing for calls on variables that are not the caller's own receiver, wildcard
  import bindings, and dynamic dispatch, all of which stay unresolved rather than guessed.

### P4 — Operational Documentation Profile

- A deterministic `ops` profile over existing canonical evidence: operations files, environment
  references, scripts, installers, CI jobs, service/bootstrap commands, and API contracts.
- Sections: deployment, configuration and environment reference, service topology, health and
  monitoring, backup and restore, upgrade and rollback, runbook steps.
- Same publication model as the other profiles: exact snapshot binding, hard limits with disclosed
  omissions, citations to evidence, immutable generations, and validated inspection.

### P4b — Monitoring Evidence Classes

- Monitoring endpoints: declared routes whose path is exactly `/health`, `/healthz`, `/health/live`,
  `/health/ready`, `/healthcheck`, `/live`, `/livez`, `/ready`, `/readyz`, `/ping`, `/status`,
  `/metrics`, `/debug/metrics`, `/actuator/health`, or `/actuator/prometheus`, taken from
  `ApiEndpoint` entities and the adapter-scoped axum/express/nextjs route kinds.
- Telemetry variables: environment-variable names carrying an `OTEL_`, `OTLP_`, `PROMETHEUS_`,
  `SENTRY_`, `DATADOG_`, `DD_`, `NEW_RELIC_`, `GRAFANA_`, `LOKI_`, `TEMPO_`, `JAEGER_`,
  `ELASTIC_APM_`, `HONEYCOMB_`, `SIGNALFX_`, or `STATSD_` prefix, or the exact names `LOG_LEVEL`,
  `RUST_LOG`, `METRICS_PORT`, and `TELEMETRY_ENABLED`. Names only; a value is never read, and a
  telemetry variable stays environment evidence as well.
- Still without an evidence source: container and Kubernetes probes, Dockerfile and compose
  healthchecks, alert rules, dashboards, and SLOs. They need extractor evidence before the category may
  name them, which is the next bounded slice for this axis.

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
`athanor-linker-js-ts`. The original P2 `Implements` bridges are source-implemented in both halves.
P2A adds a separate `Extends` relation for Rust trait supertraits and JS/TS class/interface
inheritance, updates the impact/ranking/architecture consumers, and bumps index state to v47; focused
Rust/TypeScript regressions are included; CI run `38722056518` on `4a23498` passed the full format,
test, Clippy, smoke, docs, security, feature, and coverage matrix. P2B now has source implementation
for explicit JS/TS named re-export chains and the v48 index-state rebuild. CI run `38729768792` on
`7b90b5e` passed formatting, Linux/macOS tests and Clippy, coverage, feature matrix, and cargo-deny;
the Windows job failed in the unchanged process stderr test, so P2B remains unverified pending an
exact green run. Slice P4 is covered by the shipped `operations` documentation profile
(DOCGEN-001 Slices 4A–4C); its monitoring category derives declared health, status, and metrics endpoints
and telemetry environment-variable names from canonical evidence, while container and Kubernetes probes,
alert rules, dashboards, and SLOs still need their own evidence source before they may be documented. Slice P5 is source-implemented as the bounded `explore` MCP tool: one query
returns search-ranked entries, their citations, and the one-hop relation neighbourhood with disclosed
budgets; multi-hop call paths and full blast radius remain planned. Slice P3 is source-implemented as `builtin.extractor.treesitter`: one engine walks a
per-language pack of node kinds and field names and records declaration, import, export, and call
evidence for Python and Go, with the existing cross-file linkers still owning resolution. Historical
exact-commit verification for P1, the pre-P2A P2 slice, and P5 is green on `191c65e` (sandbox
diagnostics run `38152033388`); it does not verify P2A or the current source state. The P3 engine and
packs are green on `e07547e` (run `38157055780`), and the monitoring category is green on
`69d8918` (run `38160162759`); each run reports every log PASSED with a
`rustfmt` diff of 0 lines, and each commit's repository check matrix is fully successful or intentionally
skipped. Cross-file resolution for Python and Go, and the published coverage measurement, stay open, so P3
is implemented-and-verified but not complete. The status ledger in `roadmap-status.md` carries this
evidence.
