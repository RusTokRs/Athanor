---
id: doc://docs/development/dartscope-dart-flutter-plan.md
kind: development_plan
language: en
source_language: en
status: draft
---

# DartScope And Dart/Flutter Adapter Plan

This is the project-side plan for creating DartScope as a separate long-lived Rust
library and using it from Athanor through a small Dart/Flutter adapter wrapper.

DartScope is intentionally not an Athanor crate. It should live in its own Git
repository at `D:\DartScope` and be designed for the wider Dart and Flutter
community. Athanor should consume DartScope as one user of the library, not own the
library's public purpose.

## Purpose

DartScope should become a reusable Rust toolkit for Dart and Flutter code
intelligence. It should expose stable APIs for parsing, indexing, relationship models,
framework convention extraction, rule engines, migration tools, code generation, and
other downstream tools. Its output should be a DartScope-owned analysis model, not an
Athanor canonical model.

Athanor should provide a separate adapter, likely `athanor-extractor-dart-flutter`,
that maps DartScope output into Athanor canonical entities, facts, relations,
diagnostics, evidence, ownership, and stable keys.

## Naming

The library name is `DartScope`.

Rationale:

- `scope` is a normal programming term for visibility, bindings, symbols,
  declarations, and references.
- The name is broad enough for parsing, indexing, analysis, framework extraction,
  architecture rules, migration tools, and code generation.
- The name is not tied to Athanor, visualization, or a lightweight-only promise.
- The name works for pure Dart packages, Flutter apps, monorepos, CLI tools, and
  server-side Dart.

Candidate package layout:

```text
D:\DartScope\
  Cargo.toml
  crates\
    dartscope\
    dartscope-cli\
    dartscope-parse\
    dartscope-index\
    dartscope-flutter\
    dartscope-rules\
```

The exact workspace split can wait until the first implementation. The important
boundary is that DartScope remains reusable without Athanor.

## Boundary Summary

Keep the split strict:

- DartScope owns reusable Dart/Flutter analysis: source spans, syntax structure,
  declarations, imports, exports, package metadata, project relationships, framework
  hints, rule inputs, and library-level diagnostics.
- Athanor owns canonical knowledge integration: stable keys, canonical ids, evidence,
  ownership, entities, facts, relations, diagnostics, adapter validation, bounded
  agent-facing commands, generated read models, and reports.
- DartScope may expose relationship data and JSON for other tools, but Athanor decides
  how that data becomes a canonical graph or visualization inside Athanor.
- DartScope should never import Athanor crates or emit Athanor domain types.
- The Athanor adapter should never become a second Dart parser. If analysis behavior is
  generally useful, implement it in DartScope and map it in the adapter.

## Community Use Cases

DartScope should be useful to Dart and Flutter developers even when Athanor is not
installed.

Target use cases:

- parser and syntax-tree access for Rust tools that need to read Dart code
- symbol indexing for classes, mixins, extensions, enums, constructors, fields,
  functions, imports, exports, parts, and part-of declarations
- import/export graph construction for Dart packages and Flutter monorepos
- Flutter structure extraction for widgets, screens, routes, navigation declarations,
  assets, localization usage, and theme or design-system usage
- rule engines for architecture checks, naming conventions, forbidden imports, layer
  boundaries, and clean-architecture policies
- CI tools that detect architecture violations, orphan files, dead screens, missing
  route coverage, deprecated API usage, hardcoded strings, or unsafe SDK usage
- code review bots that summarize changed public APIs, routes, screens, widgets,
  providers, dependencies, and likely impact
- migration and codemod tools that need a project index before rewriting code
- code generation tools that produce route manifests, feature manifests, documentation,
  dependency manifests, widget catalogs, or test plans
- security and compliance scanners for platform channels, permissions, network calls,
  storage calls, analytics SDKs, logging, and possible secret handling
- editor, IDE, LSP, or analysis-server-adjacent tooling that needs a fast Rust-side
  project model
- JSON export for graph visualizers and other external tools

Visualization is not a primary DartScope responsibility. DartScope should provide the
structured data that visualizers can consume. Athanor can build graph views, reports,
and agent-facing slices from its canonical model after the adapter maps DartScope
output.

## Repository Boundary

DartScope:

- lives at `D:\DartScope`
- has its own Git repository, README, license, issues, releases, and future crates.io
  publishing path
- does not depend on Athanor crates
- does not emit Athanor `Entity`, `Fact`, `Relation`, or `Diagnostic` objects
- can be used by any Rust project
- may provide a CLI for community workflows, but the library API is the primary
  contract

Athanor:

- keeps DartScope outside the Athanor workspace
- may include `crates/athanor-extractor-dart-flutter`
- consumes DartScope through a local path during active development, then through a
  published crate or pinned Git revision after the API stabilizes
- maps DartScope findings into canonical Athanor output with evidence and ownership
- exposes bounded agent-facing commands through Athanor only after canonical indexing
  exists

Do not copy DartScope parser or analyzer implementation into Athanor. If Athanor needs a
Dart/Flutter behavior improvement, prefer improving DartScope and then updating the
adapter mapping.

## DartScope Core Model

DartScope should expose its own stable model rather than leaking parser-specific AST
structures as the main public API.

Candidate top-level API:

```rust
pub fn analyze_file(input: DartFileInput) -> DartFileAnalysis;
pub fn analyze_project(input: DartProjectInput) -> DartProjectAnalysis;
pub fn parse_pubspec(input: PubspecInput) -> PubspecAnalysis;
pub fn build_import_graph(project: &DartProjectAnalysis) -> ImportGraph;
pub fn extract_flutter_inventory(project: &DartProjectAnalysis) -> FlutterInventory;
```

Candidate file analysis output:

```rust
pub struct DartFileAnalysis {
    pub path: String,
    pub language: DartFileLanguage,
    pub imports: Vec<DartImport>,
    pub exports: Vec<DartExport>,
    pub parts: Vec<DartPart>,
    pub part_of: Option<DartPartOf>,
    pub declarations: Vec<DartDeclaration>,
    pub flutter: FlutterFileHints,
    pub diagnostics: Vec<DartDiagnostic>,
}
```

Initial declaration kinds:

- class
- mixin
- enum
- extension
- typedef
- top-level function
- method
- constructor
- field
- variable

Initial Flutter hints:

- class extends `Widget`, `StatelessWidget`, `StatefulWidget`, `InheritedWidget`, or
  `State`
- `build` method declarations
- `MaterialApp`, `CupertinoApp`, `WidgetsApp`, `Navigator`, `GoRouter`, and common route
  declarations as DartScope framework hints when they can be identified with high
  confidence
- asset and localization key usage when the syntax is straightforward

Diagnostics should represent uncertainty explicitly. The library should not silently
pretend that a heuristic result is complete.

## Parser Strategy

The long-term library should be able to support multiple parser backends, but the public
model should stay backend-independent.

Possible implementation stages:

1. Start with a conservative pure-Rust scanner/parser for high-confidence declarations,
   imports, exports, parts, and simple Flutter patterns.
2. Add a parser backend abstraction before the first public API hardens.
3. Evaluate `tree-sitter-dart`, a native Rust Dart parser, or an official Dart analyzer
   bridge as optional backends when the project needs deeper syntax coverage.
4. Keep the official analyzer bridge optional because it likely runs an external Dart
   process and is not the same deployment shape as a pure Rust library.

Even if the initial implementation is scanner-based, the public story should be
`DartScope: a Dart and Flutter code intelligence toolkit`, not "a lightweight parser".

## Athanor Adapter Plan

The Athanor adapter should be small and replaceable.

Candidate crate:

```text
crates/athanor-extractor-dart-flutter/
  Cargo.toml
  README.md
  src/lib.rs
  tests/fixtures/
```

Responsibilities:

- implement the Athanor `Extractor` port
- accept `.dart` files and `pubspec.yaml`
- call DartScope for file and project-level analysis where available
- create canonical file, package, module, symbol, widget, route, asset, and localization
  entities only when the findings are high confidence
- create facts and relations for symbol definitions, imports, exports, package
  dependencies, widget/screen declarations, route declarations, and evidence-backed
  Flutter conventions
- emit diagnostics for partial parsing, unsupported syntax, ambiguous route extraction,
  missing package metadata, parser backend failures, and low-confidence framework
  inference
- include evidence and ownership on every emitted object that requires it
- keep framework-specific route or state-management heuristics out of Athanor core and
  domain

Non-goals for the first adapter slice:

- do not implement a full Dart type checker
- do not run `flutter` or `dart` commands during normal indexing
- do not require network access
- do not infer every possible dynamic route or widget composition
- do not make generated JSONL the agent-facing interface
- do not merge DartScope into the Athanor workspace

## Development Phases

### Phase 1: DartScope Repository Bootstrap

Status: planned.

Scope:

- create `D:\DartScope` as a standalone Git repository
- add a Rust workspace with a root README, license, contribution notes, and initial
  package metadata
- define the initial public positioning and non-goals
- add basic fixtures for Dart libraries and Flutter apps
- set up formatting, tests, clippy, and CI

Acceptance:

- repository builds independently from Athanor
- README explains community use cases without mentioning Athanor as the primary reason
  for the library
- public API is minimal but shaped around analysis results, not parser internals

### Phase 2: DartScope File Analysis MVP

Status: planned.

Scope:

- parse imports, exports, parts, part-of declarations, and top-level declarations
- detect class inheritance and common Flutter base classes
- parse `pubspec.yaml` package metadata and dependencies
- expose line/column or byte-range spans for every finding
- report partial parse and unsupported syntax diagnostics

Acceptance:

- fixtures cover pure Dart, Flutter widgets, syntax errors, parts, exports, and package
  dependencies
- results are deterministic on Windows and Unix path spellings
- every finding has a source span suitable for downstream source attribution

### Phase 3: DartScope Project Index

Status: planned.

Scope:

- build a package-level import/export graph
- resolve project-relative Dart file references where straightforward
- connect `part` and `part of` files
- expose a stable JSON export for CLI and downstream tooling
- preserve omitted or unresolved edges explicitly

Acceptance:

- graph output is deterministic and independent from Athanor canonical relation types
- unresolved imports and missing part files are diagnostics, not silent gaps
- monorepo-style package fixtures can be analyzed without Athanor

### Phase 4: DartScope Flutter Conventions

Status: planned.

Scope:

- extract widgets, screens, common route declarations, route names, and route targets
- detect common state-management declarations as optional conventions, starting with
  high-confidence patterns only
- detect asset and localization usage where syntax is direct
- expose confidence or diagnostic metadata for heuristics

Acceptance:

- route and widget fixtures cover `MaterialApp.routes`, `Navigator`, and `GoRouter`
  style declarations where feasible
- uncertain dynamic routing is reported as uncertain in DartScope output instead of
  fabricated as an Athanor fact
- Flutter-specific output remains optional for pure Dart consumers

### Phase 5: Athanor Adapter Wrapper

Status: planned.

Scope:

- add `athanor-extractor-dart-flutter`
- depend on DartScope through a local path during development
- map DartScope findings into Athanor canonical objects
- document the adapter in the crate README, `docs/adapters`, `docs/architecture`, and
  `docs/README.md`
- register the adapter only after metadata validation and tests pass

Acceptance:

- Athanor can index a small Dart/Flutter fixture and emit evidence-backed canonical
  output
- adapter validation catches missing evidence or ownership
- `cargo fmt --all`, workspace tests, clippy, and `cargo run -p ath --quiet -- index .`
  pass after adapter integration

### Phase 6: Bounded Agent-Facing Queries

Status: planned.

Scope:

- add focused commands or extend existing graph/context commands for Dart/Flutter
  entities only after canonical indexing exists
- expose route, widget, dependency, and affected-file slices with explicit limits
- report omitted counts and evidence links

Acceptance:

- agents do not need to read generated JSONL or full project exports
- commands are stable enough for CI or automated review workflows
- output distinguishes agreed facts from heuristic or partial findings

## Verification

DartScope verification should run in `D:\DartScope`:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
```

Athanor adapter verification should run in `D:\Athanor`:

```bash
cargo fmt --all
cargo test --workspace --quiet
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p ath --quiet -- index .
```

For documentation-only changes in Athanor, Rust verification is not required unless code
or generated behavior changed.

## Open Decisions

- Whether DartScope should start as one crate or a small workspace with separate parse,
  index, flutter, rules, and CLI crates.
- Which license to use for community adoption.
- Whether the first implementation should be a conservative pure-Rust scanner, a
  tree-sitter-backed parser, or a backend abstraction with one initial backend.
- How much Flutter route inference belongs in DartScope core versus a `dartscope-flutter`
  crate.
- When Athanor should switch from local path dependency to crates.io or pinned Git
  dependency.
- Whether DartScope should provide a CLI before the Athanor adapter exists.

## Current Recommended Next Step

1. Create `D:\DartScope` as a separate Git repository.
2. Write the DartScope README and minimal Rust workspace.
3. Implement file-level Dart imports/declarations and `pubspec.yaml` dependency parsing.
4. Add a small Athanor adapter only after DartScope has stable spans and diagnostics for
   the first fixtures.
