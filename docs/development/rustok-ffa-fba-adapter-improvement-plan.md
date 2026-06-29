---
id: doc://docs/development/rustok-ffa-fba-adapter-improvement-plan.md
kind: development_plan
language: en
last_verified_snapshot: snap_jsonl_00000261
source_language: en
status: verified
---
# RusTok FFA/FBA Adapter Improvement Plan

This is a project-specific plan for improving RusTok FFA and FBA visibility in Athanor.
It is intentionally separate from the general Athanor roadmap. The goal is to keep
RusTok migration context available without turning the global roadmap into a module
migration backlog.

## Purpose

The FFA and FBA adapters should make module migration visible from code, registries,
relations, and evidence. They should not depend on markdown readiness statuses as the
source of truth.

The adapters do not migrate code by themselves. They give agents bounded, factual
working views:

- what is already translated,
- what is missing,
- what violates the target architecture,
- what files are evidence for each finding,
- what percentage is complete for the current module, surface, or boundary,
- what small slice should be handled next.

Every adapter improvement must be tested against `D:\rustok` immediately after it lands.

## Non-Goals

- Do not merge FFA and FBA into one model.
- Do not make FBA depend on FFA readiness.
- Do not build full-project graphs by default.
- Do not make generated JSONL the agent-facing interface.
- Do not classify readiness from markdown board statuses.

## Core Invariants

- FFA and FBA stay separate adapter namespaces and graph namespaces.
- FFA emits `ffa_surface://...` and `ffa_layer://...` entities.
- Current FFA layer roles are `core`, `transport`, `ui_leptos`, `ui_support`,
  `api`, `host_wiring`, `manifest`, `crate_root`, and `other`.
- FBA emits `fba_module://...`, `fba_contract://...`, `fba_port://...`,
  `fba_operation://...`, `fba_profile://...`, and `fba_dependency://...` entities.
- Facts, relations, and diagnostics include evidence.
- Graph commands return bounded subgraphs with stable schemas and omitted counts.
- Rustok adapters remain opt-in through `.athanor/adapters/*.json`.
- Progress percentages are derived from code/registry facts and open diagnostics, not from
  markdown readiness labels.
- Documentation checks are part of every adapter iteration because docs drift can hide
  stale migration state from agents even when code facts are correct.

## Iteration Loop

Each improvement uses the same loop:

1. Improve one adapter or graph capability.
2. Add focused unit or graph tests.
3. Run Athanor verification.
4. Index `D:\rustok`.
5. Run FFA/FBA audit and graph smoke commands.
6. Run documentation checks for Athanor and the indexed Rustok project.
7. Classify new findings as adapter noise, docs drift, or real Rustok work.
8. Fix adapter noise in Athanor or real drift in Rustok.
9. Re-run the same commands until diagnostics, docs reports, progress metrics, and
   graphs are explainable.

Required Athanor verification:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
cargo run -p ath --quiet -- docs check --path . --json
cargo run -p ath --quiet -- docs drift --path . --json
```

Required Rustok smoke:

```bash
cargo run -p ath --quiet -- index D:\rustok
cargo run -p ath --quiet -- rustok ffa audit D:\rustok --json
cargo run -p ath --quiet -- rustok fba audit D:\rustok --json
cargo run -p ath --quiet -- check rustok-ffa --path D:\rustok --json
cargo run -p ath --quiet -- check rustok-fba --path D:\rustok --json
cargo run -p ath --quiet -- docs check --path D:\rustok --json
cargo run -p ath --quiet -- docs drift --path D:\rustok --json
cargo run -p ath --quiet -- graph ffa violations --path D:\rustok --json
cargo run -p ath --quiet -- graph fba violations --path D:\rustok --json
```

Rustok also has module-local verifier scripts that encode domain-specific documentation
and registry synchronization rules. When an adapter improvement touches a module family,
run the matching verifier script in addition to generic Athanor docs checks. Examples:

```bash
node scripts/verify/verify-channel-fba.mjs
node scripts/verify/verify-ecommerce-fba-registries.mjs
node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs
node scripts/verify/verify-page-builder-fba-baseline.mjs
```

Use focused graph commands for the slice being tested, for example:

```bash
cargo run -p ath --quiet -- graph ffa surface blog admin --path D:\rustok --json
cargo run -p ath --quiet -- graph fba module inventory --path D:\rustok --json
cargo run -p ath --quiet -- graph fba port inventory InventoryReservationPort --path D:\rustok --json
cargo run -p ath --quiet -- graph fba dependencies --module commerce --path D:\rustok --json
```

## Improvement 1: FFA Layer Layout Discovery Hardening

The current FFA adapter already treats flat files and module directories as the same
canonical layer for the main FFA roles. This improvement is about hardening that behavior,
adding missing tests, and making sure graph/audit output stays stable after ordinary Rust
module refactors.

Core layer examples:

- `admin/src/core.rs`
- `admin/src/core/mod.rs`
- `admin/src/core/*.rs`
- `admin/src/core/**/*.rs`

Transport layer examples:

- `admin/src/transport.rs`
- `admin/src/transport/mod.rs`
- `admin/src/transport/*.rs`
- `admin/src/transport/**/*.rs`

UI Leptos adapter examples:

- `admin/src/ui/leptos.rs`
- `admin/src/ui/leptos/mod.rs`
- `admin/src/ui/leptos/*.rs`
- `admin/src/ui/leptos/**/*.rs`

Support and preserve non-completeness roles too:

- `admin/src/ui/*.rs` outside `ui/leptos` is `ui_support`,
- `admin/src/api.rs` and `admin/src/api/**/*.rs` are `api`,
- `admin/src/lib.rs` is `crate_root`,
- `Cargo.toml` and `rustok-module.toml` are `manifest`,
- `apps/admin`, `apps/storefront`, and later `apps/server` route wiring are `host_wiring`.

Expected canonical result:

- all core files attach to `ffa_layer://<module>/<surface>/core`,
- all transport files attach to `ffa_layer://<module>/<surface>/transport`,
- all Leptos UI files attach to `ffa_layer://<module>/<surface>/ui_leptos`,
- support files attach to their explicit non-completeness roles instead of being dropped,
- missing-layer diagnostics do not fire after `core.rs -> core/` refactors.

Tests:

- `core.rs`, `core/mod.rs`, and nested `core/*.rs` all satisfy core discovery.
- nested `core/*.rs` files are linked to the same layer.
- moving `transport.rs` into `transport/mod.rs` does not change canonical layer id.
- `ui/leptos.rs` and `ui/leptos/mod.rs` both satisfy UI adapter discovery.
- `ui_support`, `api`, `crate_root`, `manifest`, and `host_wiring` are retained in
  detailed graph/audit output but do not count as complete `core_transport_ui` by
  themselves.

## Improvement 2: FBA Port Layout Discovery

Status: implemented. The extractor supports `src/ports.rs`, `src/ports/mod.rs`, and nested
`src/ports/**/*.rs`, then merges module-level code markers before checking registry requirements.

Support equivalent port layouts as one canonical FBA code source.

Current baseline: the FBA extractor only supports `crates/rustok-*/src/ports.rs`.
This is a real gap and should be the first implementation task before large FBA port
refactors.

Examples:

- `src/ports.rs`
- `src/ports/mod.rs`
- `src/ports/*.rs`
- `src/ports/**/*.rs`

Expected canonical result:

- all port files attach to the owning `fba_port://<module>/<port>`,
- trait and impl markers can be found across nested files,
- operation diagnostics still point to the concrete evidence file,
- `ports.rs -> ports/` refactors do not produce false missing-port diagnostics.

Tests:

- trait in `ports/mod.rs` and impl in `ports/service.rs` are both detected.
- operation markers are detected when split across per-port files.
- Windows and Unix path normalization keep the same canonical ids.
- multiple port-code files for one module merge into one module-level code view before
  checker diagnostics run.

## Improvement 3: Canonical Module Slug And Path Alias Handling

Keep canonical ids stable when Rustok uses different slug spellings in crates, registry
JSON, docs, or provider declarations.

Known examples:

- crate path `rustok-page-builder` may map to FBA module `page_builder`,
- registry fields may use `module`, `provider.module_slug`, `manifest_module`, or
  consumer/provider module names,
- generated expected paths must not blindly turn `page_builder` into
  `crates/rustok-page_builder/...`.

Expected behavior:

- every adapter has one canonical module slug per architecture namespace,
- path aliases and registry aliases resolve before diagnostics are emitted,
- diagnostics display the canonical id plus the concrete evidence path,
- FFA and FBA may intentionally use different canonical spellings when the codebase does.

Tests:

- `page_builder` registry does not produce a missing `crates/rustok-page_builder` path.
- hyphenated crate path and underscored registry module resolve to one FBA module id.
- consumer/provider dependency graph keeps canonical ids stable across aliases.

## Improvement 4: Evidence-Aware Diagnostics

Every FFA/FBA diagnostic should tell the agent where to look next.

Expected behavior:

- diagnostics include source file and line evidence,
- graph violations include file nodes for every diagnostic,
- module-level diagnostics link `module/layer -> file://...` with `evidenced_by`,
- port-level diagnostics link `port/operation -> file://...` with `evidenced_by`.

Tests:

- violations graph includes evidence files for module-level diagnostics.
- violations graph includes evidence files for missing operation diagnostics.
- graph omitted counts remain accurate under low node and edge limits.

## Improvement 5: FBA Consumer/Provider Cross-Checks

Status: implemented. FBA now cross-checks provider dependencies against provider registries for
contract versions, declared ports, provider profiles, provider-side consumer profile entries,
fallback profiles, and degraded modes. The checks tolerate module slug aliases and valid composite
contract-version/profile forms used by existing RusTok registries. Consumer dependency declarations
also create generic graph paths from consumer module/contract to dependency to provider, while
provider-side consumer declarations preserve hyphenated module slugs such as `ai-media`.
Placeholder entities avoid claiming primary source ownership from the real provider registry or
port-code source, and registry facts plus registry-derived entity sources now anchor at the first
registry identity line instead of defaulting to line 1 or leaving entity source lines empty.

Deepen FBA relationship checks beyond registry presence.

Expected diagnostics:

- unresolved provider module or provider registry,
- unknown provider profile,
- consumer profile missing from provider consumers,
- fallback profile mismatch,
- missing degraded mode,
- registry says write idempotency but source lacks write semantics,
- registry says read-only but source requires write semantics.

Expected graph behavior:

- dependencies graph shows only FBA entities and FBA relation edges,
- diagnostics appear next to the violated dependency,
- clean provider/consumer edges remain visible in dependency graphs,
- violations graph excludes clean edges.

Tests:

- clean `commerce -> inventory` dependency has no diagnostics.
- unknown profile emits a diagnostic with registry evidence.
- missing provider emits a diagnostic without fabricating a provider node.
- provider/consumer checks distinguish declared provider dependencies from provider-side
  `consumers` declarations.

## Improvement 6: Documentation Drift Coverage

Status: implemented. FBA now emits the separate `rustok_fba_docs_drift` diagnostic for registry,
local-plan, and central-board disagreements in status, contract versions, verifier/evidence
references, and duplicate board rows. Documentation remains secondary evidence and is not used by
the progress calculation.

Make documentation drift visible without using docs as the source of readiness.

Current baseline:

- FFA already extracts `rustok_ffa_docs_status` from `docs/modules/registry.md` and
  module-local `docs/implementation-plan.md`.
- FFA already emits `rustok_ffa_docs_drift` for duplicate board rows and local/central
  FFA/FBA status mismatches, missing board coverage, and structural shape drift.
- FBA currently validates registry evidence and verifier references, but does not yet
  provide the same local/central documentation drift model as FFA.

Expected behavior:

- `ath docs check --path D:\rustok --json` and `ath docs drift --path D:\rustok --json`
  are part of every migration visibility run.
- FBA emits documentation drift diagnostics when a registry, local implementation plan,
  and central board disagree about contract version, evidence paths, verifier runner,
  or boundary status.
- FBA docs drift uses a separate `rustok_fba_docs_drift` diagnostic kind; it should not
  reuse `rustok_ffa_docs_drift`.
- FFA/FBA docs drift diagnostics remain secondary evidence only; they do not make a
  surface or contract "ready".
- Graph violations include docs drift evidence files when docs are the thing to fix.

Tests:

- FBA registry evidence path missing from local plan emits `rustok_fba_docs_drift`.
- central board points at a stale contract version and emits a docs drift diagnostic.
- docs-only status changes do not change readiness percentage unless code/registry facts
  also changed.

## Improvement 7: FFA Host Wiring Visibility

Connect host route wiring to module-owned UI surfaces.

Expected canonical relations:

- host route file implements or mounts module-owned surface,
- module surface owns UI implementation,
- host wiring does not own module UI logic.
- host wiring is represented as `ffa_layer://<module>/<surface>/host_wiring`, but it is
  not counted as a complete surface layer by itself.

Expected diagnostics:

- `rustok_ffa_host_owns_module_ui`,
- missing module-owned route registration,
- host route points at legacy UI surface,
- module surface exists but is not wired by expected host.

Tests:

- `apps/admin` route wiring links to module admin surface.
- `apps/storefront` route wiring links to module storefront surface.
- `apps/server` route/controller wiring can be linked for FBA-adjacent backend surfaces
  without mixing FBA entities into FFA graph output.
- host-owned UI file for module behavior emits a diagnostic.

## Improvement 8: Progress Metrics

Status: implemented. FFA reports actionable core/transport/UI requirements with
explicit numerators, denominators, missing-layer counts, and integer completion percentages while
excluding scaffold/host rows as not applicable. FBA reports registry-backed contract requirements
for applicable port code, traits, operations, context/error, policy/idempotency, evidence,
contract-test, and dependency-resolution signals. Dependency-only rows are unscored, and migration
status plus diagnostics remain explicit independent signals.

Add explicit progress metrics to FFA and FBA audit reports so agents can rank work and
users can see whether migration is moving.

Expected schema additions:

- FFA audit summary extends the current fields `surfaces_total`, `core_transport_ui`,
  `incomplete`, and `diagnostics_open` with complete surfaces, partial surfaces,
  missing-layer counts, and `completion_percent`.
- FFA surface entries include `completion_percent` and a reasoned breakdown:
  core present, transport present, UI adapter present, host wiring present, diagnostics
  open.
- FBA audit summary extends the current fields `modules_total`, `provider_modules`,
  `consumer_modules`, `ports_total`, `operations_total`, and `diagnostics_open` with
  modules with registry, modules with port code, operations implemented, dependency
  edges resolved, and `completion_percent`.
- FBA module entries include `completion_percent` and a breakdown:
  registry present, port traits present, operations present, context/error present,
  policy semantics present, evidence present, dependencies resolved.
- Percentages are integer or fixed one-decimal values derived from explicit numerator and
  denominator fields also present in JSON.

Rules:

- A percentage must never hide diagnostics. Reports show both `completion_percent` and
  `diagnostics_open`.
- A module with missing evidence cannot be 100 percent complete.
- A docs-only readiness label cannot increase completion percentage.
- Unknown or intentionally not-applicable requirements must be represented as excluded
  denominators, not silently counted as complete.

Candidate output shape:

```json
{
  "summary": {
    "completion_percent": 87.5,
    "completed": 21,
    "total": 24,
    "diagnostics_open": 3
  }
}
```

Tests:

- clean fixture reports `100.0`.
- fixture missing UI adapter reports less than `100.0` with a missing UI denominator item.
- fixture with docs-only status set to ready still reports based on code facts.
- Rustok baseline percentages are stable across repeated indexes with no source changes.
- text output keeps existing counts and may add percentage, but JSON remains the contract.

## Improvement 9: Next-Slice Context Graphs

Add graph commands that return the smallest useful context for the next migration step.

Candidate commands:

```bash
cargo run -p ath --quiet -- graph ffa next --module blog --surface admin --path D:\rustok --json
cargo run -p ath --quiet -- graph fba next --module commerce --path D:\rustok --json
```

Expected result:

- only files and entities needed for the next action,
- open diagnostics first,
- direct evidence files included,
- adjacent clean dependencies included only when useful for the fix,
- default limits remain small.

Tests:

- clean module returns no next-slice diagnostics and a small context.
- module with missing layer returns layer, evidence, host wiring, and related files.
- FBA dependency mismatch returns consumer registry, provider registry, and port source.

## Noise Budget

An improvement is not accepted if it makes agents read more without improving actionability.

Reject or rework changes that:

- add broad full-project graphs by default,
- emit diagnostics without evidence,
- report percentages without numerator and denominator fields,
- let docs-only status text change completion percentages,
- duplicate existing stable id or path normalization logic,
- classify readiness from markdown status text,
- mix FFA and FBA entities in the same command output,
- create false positives after ordinary Rust module refactors.

## Gaps Found During Plan Review

- Generic Athanor documentation gates were missing from the iteration loop.
- FBA lacks FFA-like documentation drift coverage for local implementation plans and
  central board synchronization.
- FFA/FBA audit reports expose counts but not explicit completion percentages.
- Next-slice graph commands are planned but not implemented.
- FFA layer discovery already handles the main flat-file and directory layouts, but needs
  stronger tests and explicit non-completeness role accounting.
- FBA port discovery must be generalized before `ports.rs -> ports/` refactors, otherwise
  agents can see false missing-port diagnostics.
- FBA module slug/path aliases need explicit handling for remaining mixed hyphen/underscore modules;
  provider-side consumer declarations already preserve hyphenated slugs such as `ai-media`.

## Current Baseline

As of the first practical FBA run against `D:\rustok`, the FBA chain can index, audit,
check, and graph the repository with zero open FBA diagnostics after Rustok registry
metadata and write-semantics marker drift were fixed.

Known next priority:

1. Add canonical module slug/path alias handling.
2. Deepen evidence-aware dependency diagnostics and consumer/provider cross-checks.
3. Add FFA host-wiring visibility.
4. Add bounded next-slice context graphs.
5. Re-run the full iteration loop against `D:\rustok` after each slice.
