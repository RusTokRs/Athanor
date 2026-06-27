---
id: doc://docs/development/dart-flutter-adapter-plan.md
kind: development_plan
language: en
source_language: en
status: draft
---

# Dart/Flutter Adapter Plan

This is the Athanor-side plan for consuming the external DartScope library from a
small Dart/Flutter extractor adapter.

The DartScope library plan lives in the separate repository at:

```text
D:\DartScope\docs\development\dartscope-library-plan.md
```

Athanor should not own DartScope's parser, analyzer, rule engine, or Flutter
convention implementation. Athanor should only map DartScope output into Athanor's
canonical knowledge model.

## Boundary

DartScope owns:

- Dart and Flutter source analysis
- parser backends and parser recovery
- DartScope spans and diagnostics
- imports, exports, parts, package metadata, and project relationships
- Flutter framework hints and optional rule/lint output
- DartScope JSON output for general tooling

Athanor owns:

- `Extractor` adapter implementation
- mapping DartScope analysis into Athanor entities, facts, relations, diagnostics,
  evidence, ownership, and stable keys
- adapter validation
- canonical storage and generated read models
- bounded agent-facing commands and graph/context views

Rules:

- DartScope must not import Athanor crates.
- The Athanor adapter must not become a second Dart parser.
- Generally useful Dart/Flutter analysis belongs in DartScope first.
- Athanor-specific mapping, evidence construction, ownership, stable keys, and bounded
  commands belong in Athanor.

## Adapter Crate

Candidate crate:

```text
crates/athanor-extractor-dart-flutter/
  Cargo.toml
  README.md
  src/lib.rs
  tests/fixtures/
```

The crate should implement the Athanor `Extractor` port and consume DartScope through
the broad `dartscope` umbrella crate.

During active development, Athanor may use a local path dependency:

```toml
[dependencies]
dartscope = { path = "D:/DartScope/crates/dartscope", features = ["parse", "resolve", "index", "flutter", "json"] }
```

After the DartScope API stabilizes, switch to a published crate version or pinned Git
revision.

## Mapping Responsibilities

The adapter should accept:

- `.dart` files
- `pubspec.yaml`
- package/project metadata exposed by DartScope

The adapter should create canonical objects only for high-confidence findings.

Candidate entities:

- Dart/Flutter file or module
- Dart package
- package dependency
- class, mixin, enum, extension, typedef, function, method, constructor, field, and
  variable symbols
- Flutter widget, screen, route, asset, localization key, and theme/design-system hint
  where DartScope reports sufficient confidence

Candidate facts and relations:

- source file defines symbol
- module imports or exports module/package
- package declares dependency
- Dart part belongs to library
- class extends or mixes in another symbol when DartScope can report it
- widget or screen declares route
- source references asset or localization key

Candidate diagnostics:

- partial DartScope parsing
- unsupported syntax
- parser backend failure
- unresolved package import
- missing part file
- ambiguous route extraction
- low-confidence Flutter convention inference
- DartScope version or feature mismatch

Every emitted Athanor fact, relation, and diagnostic must include evidence and
ownership. Every emitted entity must include ownership.

## Non-Goals

- Do not implement a full Dart type checker inside Athanor.
- Do not run `flutter` or `dart` commands during normal indexing unless a future
  explicit opt-in mode is designed.
- Do not require network access.
- Do not infer every possible dynamic route or widget composition.
- Do not make generated JSONL the agent-facing interface.
- Do not copy DartScope parser/analyzer code into Athanor.
- Do not add Flutter concepts to `athanor-domain` or `athanor-core` unless they become
  language-independent canonical concepts.

## Development Phases

### Phase 1: Adapter Skeleton

Status: planned.

Scope:

- add `athanor-extractor-dart-flutter`
- depend on DartScope through a local path during development
- support `.dart` and `pubspec.yaml` source dispatch
- add crate README and adapter docs
- keep the adapter out of the default registry until validation and tests pass

Acceptance:

- crate builds in the Athanor workspace
- adapter documentation states inputs, outputs, limitations, and verification commands
- no DartScope implementation code is copied into Athanor

### Phase 2: File And Package Mapping

Status: planned.

Scope:

- call DartScope file/package analysis
- map file, package, dependency, module, import/export, part, and declaration findings
  into canonical Athanor output
- attach ownership to every emitted entity
- attach evidence and ownership to facts, relations, and diagnostics
- add focused fixtures

Acceptance:

- adapter validation passes for fixtures
- parser uncertainty is surfaced as diagnostics rather than hidden
- stable keys remain deterministic across Windows and Unix-style path spellings

### Phase 3: Flutter Mapping

Status: planned.

Scope:

- map DartScope Flutter hints for widgets, screens, routes, assets, localization, and
  themes
- distinguish high-confidence facts from heuristic or uncertain findings
- emit diagnostics for ambiguous or unsupported Flutter conventions

Acceptance:

- dynamic or ambiguous routing is not converted into false canonical facts
- evidence links point to source spans reported by DartScope
- pure Dart projects do not receive Flutter-specific entities unless DartScope reports
  Flutter signals

### Phase 4: Bounded Agent-Facing Queries

Status: planned.

Scope:

- add focused graph/context commands only after canonical indexing exists
- expose bounded slices for packages, dependencies, widgets, routes, assets, localization,
  and affected files
- include explicit limits, omitted counts, canonical ids, and evidence links

Acceptance:

- agents do not need to read generated JSONL or DartScope full-project exports
- command outputs are deterministic and stable enough for automated workflows

## Documentation Updates

When the adapter is implemented, update:

- `docs/architecture/adapters.md`
- `docs/architecture/pipeline.md` if runtime behavior changes
- `docs/adapters/extractor-dart-flutter.md`
- `crates/athanor-extractor-dart-flutter/README.md`
- `docs/README.md`
- `docs/development/roadmap-status.md`

## Verification

For adapter code changes in `D:\Athanor`:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
```

For documentation-only changes in Athanor, Rust verification is not required unless code
or generated behavior changed.

## Current Recommended Next Step

1. Build the initial DartScope workspace and file-analysis API in `D:\DartScope`.
2. Keep this Athanor adapter plan as the integration contract.
3. Add `athanor-extractor-dart-flutter` only after DartScope exposes stable spans,
   diagnostics, and JSON/API output for the first fixtures.
