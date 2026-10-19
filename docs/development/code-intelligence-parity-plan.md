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

- Rust: trait implementation (`Implements`) and supertrait (`Extends`) edges; P2P resolves a method
  call through the receiver's declared type, so a `dyn`/`impl Trait` receiver reaches the trait
  declaration it states, and P2Q records the declared implementations that call can reach.
- JS/TS: class `extends`/`implements` and interface `extends` with distinct canonical relation kinds;
  P2B covers explicit named re-export chains, P2C bounded single-source wildcard re-exports, P2D
  named default function exports/re-exports, P2E direct namespace re-exports for single-member calls
  through named imports, P2F bounded multiple-source wildcard aggregation, P2G callback and
  promise-handler bridges for allowlisted combinators, P2J module-resolution configuration with
  TypeScript path aliases and workspace-package specifiers, P2K anonymous default exports bound
  to the canonical `default` name, and P2L package `exports` subpaths with conditional exports,
  pnpm workspace layouts, and merged `rootDirs` roots, P2M typed receivers and deep member paths,
  P2N typed receiver paths that cross module boundaries, P2O receivers typed by a factory's
  declared return type, and P2P/P2Q method calls resolved through the receiver's declared type: a
  `dyn` or `impl Trait` receiver reaches the trait declaration and then every declared implementation
  of it. Type inference for a receiver whose type the source never states remains open.
- Both: symbol-level incremental invalidation ships with P2R, so a changed declaration re-resolves
  only its closure; what remains open is reusing a declaration's relations across a change that does
  not touch it at all, without recomputing the closure it sits in.

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

#### P2C — Single-source Wildcard Re-exports (bounded JS/TS slice)

- Mark top-level `export * from "./module"` separately from `export * as namespace from "./module"`
  in the normalized export payload, preserving source and line evidence.
- Resolve a named import through a source-backed wildcard re-export when each module in the chain has
  exactly one wildcard source. The existing 32-hop bound also applies to mixed named/star chains.
- Explicit named mappings take precedence; an unresolved explicit mapping must not fall through to a
  wildcard. Default exports, namespace exports, multiple wildcard clauses in one module, unresolved
  sources, cycles, and ambiguous targets remain unresolved.
- Reuse the verified module `Imports` edge for wildcard sources and bump normal/precision index-state
  schema families from v48 to v49 because the normalized module payload changes.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | Plain `export *` is marked separately from namespace exports; source ranges and empty bindings remain explicit |
| Import linker | A relative wildcard source emits a verified module `Imports` edge with re-export evidence |
| Call linker | Named imports resolve through bounded single-source star chains; aliases are preserved; defaults, namespace exports, multiple-star modules, cycles, depth overflow, and unresolved explicit mappings stay unresolved |
| Persistence | v48 normal/precision state is rejected and forces a v49 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2D — Named Default Function Exports (bounded JS/TS slice)

- Mark `export default` statements separately from named export clauses, and record the local binding
  only for a named function or a statically named local identifier.
- Resolve default imports to a unique function declaration in the same module; follow relative local
  identifier bindings and explicit named re-exports such as `export { default as signIn } from` within
  the existing 32-hop/cycle bounds.
- Do not expose `default` through `export *`. Anonymous default expressions, class-valued default
  exports as call targets, unresolved imports, cycles, and ambiguous local bindings stay unresolved.
- Bump normal/precision index-state schemas from v49 to v50 because export payloads gain
  `default_export` and `default_name` fields.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | Named default functions and local identifier defaults record their binding; `default` inside `{ default as name }` remains an ordinary named re-export |
| Call linker | Default imports resolve to a named function; a relative named re-export chain can forward a default function; wildcard exports still do not expose `default` |
| Persistence | v49 normal/precision state is rejected and forces a v50 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2E — Direct Namespace Re-export Member Calls (bounded JS/TS slice)

- Record `export * as name from './module'` as a direct namespace alias (`namespace_name`) separately
  from bare `export *`; preserve the source-backed mapping in the normalized module payload.
- Resolve namespace-object calls such as `import { name as local } from './barrel'; local.member()`
  only through one unique, direct, source-backed namespace mapping, then use the existing bounded
  export resolver for that one member. The namespace hop counts toward the existing 32-hop
  named/star/default export bound; previously supported directly qualified imported-class methods
  and namespace-import receiver calls retain their existing resolution paths.
- Keep missing or duplicate namespace aliases, unresolved sources, cycles, over-limit chains, and
  multi-segment member paths through named-import namespace aliases unresolved. P2E does not add
  multi-hop namespace aliases or deep member-path resolution; multiple-source wildcard aggregation is
  covered separately by P2F. Bare `export *` still does not expose `default`.
- Bump normal/precision index-state schemas from v50 to v51 because normalized export payloads gain
  the `namespace_name` field.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | `export * as ns` records `namespace_name: "ns"`; bare `export *` remains a separate star export |
| Call linker | An aliased named import resolves one member through a unique direct namespace mapping; missing/duplicate mappings and its multi-segment paths stay unresolved |
| Bounds | Namespace-source cycles and chains beyond 32 total export hops remain unresolved; source-backed resolution uses existing named/star rules |
| Persistence | v50 normal/precision state is rejected and forces a v51 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2F — Multiple-source Wildcard Aggregation (bounded JS/TS slice)

- Resolve a requested named export across every relative, source-backed `export *` clause in a
  module. Ignore a branch only when it definitively does not export that name; accept one canonical
  declaration when every applicable branch agrees on its entity identity, including repeated paths
  to that same declaration. Each call resolution is bounded to 1,024 resolver visits in addition to
  the existing 32-hop path limit.
- Keep distinct exported declarations ambiguous. A source that is missing or unresolved, an
  ambiguous nested export, a cycle, a branch beyond the 32-hop bound, or exhausted visit budget blocks
  the whole aggregate; a successful sibling must not hide an invalid branch. Explicit named mappings
  retain precedence, and bare `export *` continues to exclude `default`.
- The resolver uses the existing normalized export payload and module `Imports` evidence; no parser
  schema field changes. Bump normal/precision index-state schemas from v51 to v52 anyway: the index
  pipeline reuses the prior canonical snapshot when source hashes are unchanged, so this linker-only
  semantic change requires a one-time rebuild to refresh persisted call relations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Call linker | A unique exporting source plus an absent-name source resolves; multiple paths to the same declaration resolve; distinct declarations remain ambiguous |
| Safety | An unresolved source, cyclic branch, over-limit path, or exhausted visit budget cannot be hidden by a successful sibling; explicit mappings win and wildcard `default` stays excluded |
| Persistence | v51 normal/precision state is rejected and forces a v52 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

Exact-commit CI run `38785145039` on `a67782f` passed the Rust 1.95 Ubuntu/macOS/Windows matrix,
all feature matrices, workspace tests, Clippy, source coverage, smoke, editable docs, security and
license checks. The companion AppSec, Store Conformance, Production Gate, focused documentation, and
DOCGEN checks also passed for that exact commit.

#### P2G — Callback And Promise-Handler Bridges (bounded JS/TS slice)

- The extractor records a bounded `callback_arguments` array on call sites whose terminal callee
  name is a known callback combinator: array iteration methods (`map`, `filter`, `forEach`, `find`,
  `findIndex`, `findLast`, `findLastIndex`, `some`, `every`, `reduce`, `reduceRight`, `flatMap`,
  `sort`), promise handlers (`then`, `catch`, `finally`), timers (`setTimeout`, `setInterval`),
  and event/observable registration (`addEventListener`, `addListener`, `on`, `once`,
  `subscribe`). Only plain identifiers and identifier-rooted member chains are recorded; inline
  arrows, call results, literals, and computed expressions are skipped, and at most four
  arguments are kept per call site.
- The call linker resolves each recorded argument through the existing ordered rules
  (same-module exact/`this.` member, import bindings, re-export chains, unique names) and emits
  its own `Calls` relation with reason `js_ts_callback_bridge`, the hosting combinator in
  `host_callee`, and `callee_kind: "callback"`. A dedicated relation-id material prefix keeps a
  direct call and a callback bridge to the same target as two distinct edges.
- Bridges inherit the fail-closed rules: an unresolvable, ambiguous, cyclic, or over-budget
  argument stays unresolved, and the hosting call's own resolution (or its failure) never gates
  the bridge, so `list.map(filterActive)` bridges even though `list.map` itself stays
  unresolved. Bridges from a test location additionally emit `TestedBy`.
- Non-allowlisted callees never record callback arguments, so an ordinary `queue.push(handler)`
  call does not become a guessed edge. Callbacks passed as inline arrows, call results, or
  computed expressions remain outside the model because they have no statically nameable
  target. Receiver typing for callback arguments is unchanged: resolution is name-and-binding
  based only.
- Bump normal/precision index-state schemas from v52 to v53 because the normalized call-site
  payload gains the `callback_arguments` field, so cached module payloads are rebuilt in both
  configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | Allowlisted combinator calls record bounded, statically nameable `callback_arguments`; non-allowlisted callees and literal/arrow/call-result arguments record none |
| Call linker | Combinator arguments resolve through the existing ordered rules and emit `Calls` with reason `js_ts_callback_bridge` and `host_callee`; unresolvable and ambiguous arguments stay unresolved; test-location bridges emit `TestedBy` |
| Persistence | v52 normal/precision state is rejected and forces a v53 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2H — Default Class Exports And Class Construction Targets (bounded JS/TS slice)

- Extend the default-export binding to named `export default class Foo {}` declarations, so
  the normalized export payload records `default_name` for a default class exactly as it
  already does for a default function. Anonymous default classes (`export default class {}`)
  and every other default expression stay explicitly unbound and cannot be guessed.
- Admit `EntityKind::Class` declarations into the call linker's resolvable declaration set,
  so a class is a valid `Calls` target for a same-module exact-name match and for a
  default-imported construction (`new Foo()`), reusing the existing ordered resolution and
  its fail-closed rules. A named class and a same-named function in different modules stay
  ambiguous and unresolved at the unique-name steps.
- Receiver typing is unchanged: a class member reached through an imported instance
  (`instance.method()`) is not resolved by this slice, and deep member paths stay open.
- Bump both index-state schema families from v53 to v54 because the normalized export
  payload now carries a default-class binding, so cached module payloads are rebuilt in both
  configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | `export default class Foo {}` records `default_name: "Foo"`; anonymous `export default class {}` and non-class default expressions stay null |
| Call linker | A default import of a named default class resolves a `new Foo()` site to the class with `imported_default_export`; a same-module `new Foo()` resolves with `same_module_exact_name` |
| Persistence | v53 normal/precision state is rejected and forces a v54 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2I — Multi-Hop Namespace Aliases (bounded JS/TS slice)

- A namespace member call `alias.member()` resolves when the namespace name is
  forwarded across modules by source-backed named re-exports, not only when the
  importing module declares the `export * as alias` clause directly. The chain
  follows each `export { alias as renamed } from "./source"` forwarding until it
  reaches the module that declares the `export * as alias` namespace re-export,
  then resolves `member` in that namespace's source with the existing ordered
  rules. The namespace name may change at each hop through the re-export alias.
- Fail-closed bounds: an absent forwarding keeps the existing non-namespace
  fallbacks; multiple forwardings of one namespace name, an unresolved or cyclic
  forwarding, and a chain over the 32-hop or 1,024-visit bound all stay
  unresolved. A namespace member is never guessed from a snapshot-wide name match.
- Extractor payloads are unchanged: named re-export bindings already record the
  imported and exported spellings, so this is a linker-only resolution extension.
- Bump both index-state schema families from v54 to v55 so cached relations are
  recomputed with the extended namespace resolution in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Call linker | A renamed forwarded namespace alias (`export { api as client }` then `export * as api`) resolves `client.member()`; a same-name forwarding resolves too |
| Fail-closed | Two forwardings of one namespace name, a forwarding chain that ends without a namespace declaration, cycles, and over-bound chains stay unresolved |
| Persistence | v54 normal/precision state is rejected and forces a v55 rebuild |

Focused verification:

```bash
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2J — Module Resolution Configuration (bounded JS/TS slice)

- The extractor projects the module-resolution evidence of first-party `tsconfig.json`,
  `jsconfig.json`, and their named `.json` variants. One `EntityKind::Feature` per file
  (`config://<path>#js-ts-module-resolution`) records the `baseUrl`-resolved base directory, each
  `paths` pattern with its substitution targets and declaration line, an inherited `extends`
  specifier, and disclosed `omitted_paths`/`omitted_targets` counts. Bounds are 64 mappings and 8
  targets per file; a pattern with more than one `*`, a non-string target, and an empty target list
  are skipped. An absolute, drive-qualified, or repository-escaping `baseUrl` is reported without a
  base directory, which keeps its mappings unresolvable. A configuration file without
  `compilerOptions` and without `extends` is not projected.
- The same extractor records `main` and `workspaces` on the existing `package.json` package entity,
  in both the array form and the `{ "packages": [...] }` object form, with an `omitted_workspaces`
  count. A bounded JSONC normalization removes comments outside string literals and trailing commas
  before parsing while preserving line structure, so evidence lines stay correct and `//` or `/* */`
  inside a string value survives verbatim. An unreadable configuration file emits
  `js_ts_typescript_config_parse_error` instead of being silently dropped.
- The linkers share one snapshot-scoped resolution configuration built from those entities. A
  non-relative specifier resolves as a path alias through the nearest configuration that applies to
  the importing module — the nearest `tsconfig.json`/`jsconfig.json` at or above its directory, or
  the first ancestor along its relative `extends` chain that declares mappings, up to 8 hops, because
  TypeScript does not merge mappings across `extends` — and otherwise as a workspace package.
- Workspace membership is the gate that keeps a bare specifier honest: a `package.json` resolves only
  when another `package.json` in the snapshot declares a workspace glob, with `*` matching one path
  segment and `**` matching any number of them, that covers its directory. The entry point is the
  recorded `main` when it resolves, otherwise `index` in the package root, and a `name/subpath`
  specifier resolves inside the package directory. A duplicated package name, a package no workspace
  declaration covers, a package-valued `extends`, and a specifier matching no mapping stay
  unresolved; `node_modules`, `exports` and conditional-exports maps, `rootDirs`, package-manager
  installs, and `pnpm-workspace.yaml` are never read.
- The import linker labels the origin in the relation reason (`relative_module_import`,
  `path_alias_module_import`, `workspace_package_module_import`, and the matching `_re_export`
  forms), and the call linker resolves import bindings, re-export sources, and namespace forwards
  through the same rules, so a helper reached through an alias or a workspace package becomes a
  real call edge.
- Bump both index-state schema families from v55 to v56 because the normalized package payload and
  the new configuration entity change the linker input, so cached relations are recomputed in both
  configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | `tsconfig.json`/`jsconfig.json` path mappings, base directory, `extends`, JSONC comments and trailing commas, disclosed bounds, and the parse-error diagnostic; `package.json` `main` and `workspaces` in both shapes |
| Import linker | A path alias and a workspace package emit `Imports` with the matching origin-named reason; a non-member package, a duplicated name, and an unusable base directory emit none |
| Call linker | A call through an aliased import and through a workspace-package import resolves; re-export sources and namespace forwards resolve through aliases |
| Persistence | v55 normal/precision state is rejected and forces a v56 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2K — Anonymous Default Exports (bounded JS/TS slice)

- An anonymous default export is a real declaration with no source name: `export default function
  () {}`, `export default class {}`, and `export default () => {}` are all callable or constructible
  from other modules. The extractor records such a declaration under the canonical name `default`,
  the one name a JavaScript or TypeScript binding cannot have, so the synthesized key cannot collide
  with a declared binding and every consumer sees that the name is not evidence. The declaration
  keeps its real evidence: source range, `exported`, call sites, class heritage, and class members
  (`default.method`), plus an `anonymous_default` marker in the payload.
- The module payload binds the default export to the same name. `default_name` is `default` for an
  anonymous function, class, or arrow function default, so the existing default-export resolution
  rules carry it without a new linker path: a default import, a default-imported `new`, and an
  `export { default as api } from "./anon"` re-export alias all resolve to the recorded declaration.
- Fail-closed bounds: a default expression that declares nothing callable or constructible — an
  object literal, a call result, or a primitive — stays unbound, so `export default { run() {} }`
  does not invent a symbol. At most one anonymous default is recorded per file, because a module has
  at most one default export and a second one would be ambiguous. Precision verification excludes
  synthesized `default` rows, because the second backend names only declarations that carry a source
  identifier and comparing them would report a disagreement Athanor itself introduced.
- Bump both index-state schema families from v56 to v57 because the JS/TS module payload and the set
  of declarations change the linker input, so cached relations are recomputed in both
  configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | Anonymous default function, async function, arrow function, and class declarations recorded as `default` with `anonymous_default`, call sites, class members, and heritage; object, call, and primitive defaults stay unbound; at most one anonymous default per file |
| Call linker | A default import of an anonymous default function, a default-imported `new` of an anonymous default class, and an anonymous default forwarded through `export { default as api }` all resolve |
| Precision | The synthesized `default` row is not reported as a backend disagreement |
| Persistence | v56 normal/precision state is rejected and forces a v57 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2L — Package Exports, Workspace Layouts And Merged Roots (bounded JS/TS slice)

- A `package.json` `exports` map is projected as ordered rows: one row per declared target with its
  subpath key, the condition keys guarding it, and the target path. Sugar forms are normalized — a
  bare string, a bare array, and an object keyed by conditions all describe the `.` subpath — and
  wildcard subpaths keep their `*`. Bounds are 64 rows and 4 condition keys per row, with an
  `omitted_exports` count. A declaration-order walk is exactly the order Node falls back through,
  so the linker proves rows in the package's own preference order instead of reconstructing the
  tree.
- The linker keeps Node's encapsulation rule: once a package declares `exports`, a subpath it does
  not declare is unreachable even when the file exists, so `pkg/tools` stays unresolved unless the
  map declares it. Among matching rows the longest static prefix wins; rows of one subpath are then
  tried in declaration order until a target resolves to a real module. A row guarded by a condition
  Athanor cannot evaluate — `production`, `deno`, `worker`, or any custom key — is skipped rather
  than guessed, and a target that leaves the package directory stays unresolved.
- `pnpm-workspace.yaml` declares workspace membership for layouts that have no npm `workspaces`
  field. Its top-level `packages:` block sequence is projected as a workspace-layout entity, and
  those patterns grant membership exactly like a `workspaces` glob. Flow sequences, anchors, and
  nested blocks are outside the projection, and a manifest without a readable package list is not
  projected at all, so an unreadable layout never turns every package into a workspace member.
- `compilerOptions.rootDirs` records the roots TypeScript presents as one merged virtual directory.
  A relative specifier that misses in the importing file's own root is retried at the same relative
  path in the other roots of the nearest configuration that declares them; two roots that both hold
  the path are ambiguous and stay unresolved. The origin is recorded in the relation reason as
  `root_directory_module_import`, so a merged-root edge is never mistaken for a plain relative one.
- Bump both index-state schema families from v57 to v58 because the package and configuration
  payloads change the linker input, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | `exports` rows with conditions, arrays, wildcards, and sugar forms; disclosure bounds; the pnpm `packages:` list, comments, and the no-evidence case; `rootDirs` roots and their bounds |
| Import linker | A declared subpath and the package root resolve through `exports`; an undeclared subpath, an unevaluable condition, and a target outside the package stay unresolved |
| Membership | A package covered only by a pnpm layout resolves, and the same package without the layout does not |
| Relative fallback | A relative import resolves in another merged root, and two roots holding the same path stay unresolved |
| Persistence | v57 normal/precision state is rejected and forces a v58 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2M — Typed Receivers And Member Paths (bounded JS/TS slice)

- A receiver that is only identifiable through its declared type is now resolvable. The extractor
  records receiver types as module-level evidence in `typed_bindings`: one row per binding with the
  binding name, the type name, the `kind` (`construction` for a `new Type()` initializer,
  `annotation` for a TypeScript annotation), and the line. Rows come from variable declarators,
  class fields — where the binding name is `Class.field`, so the rows of two classes never collide —
  and TypeScript parameter properties, where `constructor(private repo: Repo)` also yields
  `Class.repo`.
- Only a type the row actually names is recorded. A call result, a union, an array type, a
  destructuring pattern, an aliased initializer, a function type, and a generic instantiation that
  is not a bare identifier stay out, and a qualified or generic name is reduced to its terminal
  identifier, so `Promise<Repo>` and `ns.Repo` both record `Repo`. Collection is bounded at 64 rows
  per module with an `omitted_typed_bindings` count, so one large module cannot dominate a payload.
- The linker walks a receiver as a typed path instead of a bare name: `this` roots at the enclosing
  method owner, every further segment resolves the type the previous segment declared, and the
  terminal member must match `<Type>.<member>` exactly once. A type repeated inside the walk, an
  unknown segment type, or a path longer than four segments ends the walk. `this.repo.items.map()`
  therefore resolves when `App.repo` is a `Repo` and `Repo.items` is an `ItemStore`.
- Ambiguity stays unresolved. Two rows for the same binding name, a receiver with no row, a member
  the type does not declare, and a terminal that matches more than one entity all fail closed and
  fall through to the existing `same_module_member_name` and `unique_exported_name` guesses, which
  remain strictly weaker. No resolution is emitted for a receiver the type evidence does not prove.
- Receiver rows are module-local: a type imported from another module still has to be resolved by
  the import linker before its members can be walked, so cross-module typed dispatch stays a
  remaining gap rather than being inferred here.
- Confidence is stated per resolution: `0.8` for a receiver proven by a construction and `0.7` for
  one proven by an annotation — above the name guesses they replace, below import-proven calls. The
  reason distinguishes `typed_receiver_member` from `typed_receiver_path`.
- Bump both index-state schema families from v58 to v59 because module payloads gain receiver type
  rows, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | Rows from constructions, annotations, and TS parameter properties; rejection of call results, unions, arrays, destructuring, aliases, function types, and generics; the 64-row cap |
| Import/call linker | A single-segment receiver and a deep `this.repo.items.map` path resolve; the same paths with an ambiguous, untyped, cyclic, or over-bound receiver stay unresolved |
| Confidence | Constructions land at `0.8`, annotations at `0.7`, and both stay below import-proven calls |
| Persistence | v58 normal/precision state is rejected and forces a v59 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2N — Typed Receiver Paths Across Modules (bounded JS/TS slice)

- A typed path no longer stops at the calling file. P2M read every receiver segment from the rows
  of the calling module, so `this.repo.items.map()` stayed unresolved when `Repo.items` is declared
  in `repo.ts` even though that module records the field's type. The field lookup now falls through
  to the module that owns the type.
- Ownership is proven before it is used. A type-owner index maps a type name to the module declaring
  it, and only an exported top-level class, interface, or type alias can own field types: a name
  declared in two modules names neither, a shadowed or nested name is not an owner, and a type the
  snapshot does not export leaves the walk unresolved instead of reading rows from whichever module
  happens to declare a field of that name.
- The path itself is unchanged otherwise: `this` still roots at the enclosing method owner, a field
  is read from the calling module first and from the owning module second, a repeated type or a path
  longer than four segments ends the walk, and the terminal member must match `<Type>.<member>`
  exactly once. `App.repo` stays local evidence while `Repo.items` is read where `Repo` lives.
- Interfaces carry the evidence classes already had. The extractor records `property_signature`
  members as typed fields (`Interface.field`), required or optional, because an interface is where a
  TypeScript type usually states what its fields are. Rejection rules are unchanged: a call result,
  a union, an array type, an aliased initializer, a function type, and a generic instantiation that
  is not a bare identifier still name nothing.
- The extractor's module payload shape does not change, so no index-state bump is needed; only the
  set of declarations that contribute rows grew.
- Confidence and the fail-closed order do not move: a cross-module field row is an annotation at
  `0.7`, a construction stays at `0.8`, and an unproven owner leaves the call to the weaker
  `same_module_member_name` and `unique_exported_name` guesses. A receiver whose type is only
  inferred — a function result, an index access, or an unannotated binding — stays unresolved.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | Interface property signatures, required and optional, recorded as `Interface.field` next to class fields |
| Import/call linker | A path through a type declared in another module resolves as `typed_receiver_path`; an unexported owner and an owner declared in two modules stay unresolved |
| Confidence | A cross-module field row resolves at `0.7` and a construction at `0.8`, both below import-proven calls |
| Persistence | The extractor payload shape is unchanged, so v59 state stays valid |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2O — Factory Return Types As Receiver Evidence (bounded JS/TS slice)

- A receiver produced by a call is now typed by the factory. `const repo = createRepo();
  repo.items.map()` stayed unresolved because the type is stated by `createRepo`, not by the
  binding; the extractor records both halves of that evidence and the linker joins them.
- Declarations carry the return type they declare. Functions, methods, arrow functions, and
  variable-bound functions record `return_type`, and only a bare named type is usable: `Promise<Repo>`
  returns a promise rather than a `Repo`, so generic instantiations, unions, arrays, and function
  types are not recorded. A function without an annotation records nothing.
- Modules carry the bindings they initialize with a statically named call: `call_bindings` rows of
  `{name, callee, line}`, bounded to 64 rows with an `omitted_call_bindings` count. A destructuring
  initializer and a non-call initializer are not bindings, because neither names one receiver.
- Provenance decides whether a factory's return type may be used. The factory is resolved through the
  existing ordered resolution, so a local, imported, or re-exported factory counts; a factory reached
  only by `same_module_member_name` or `unique_exported_name` does not, because a name-unique
  function in an unimported module does not prove which function ran. The typed step is disabled for
  that inner lookup, so a factory is never resolved through another receiver's type and the walk
  cannot recurse.
- A return-type annotation is annotation evidence at `0.7`, the same tier as a field annotation, and
  an unproven factory leaves the call to the weaker name guesses. A receiver whose type the source
  never states anywhere — an unannotated factory, an index access, a structural type — stays
  unresolved.
- Bump both index-state schema families from v59 to v60 because declaration payloads gain return
  types and module payloads gain call-initialized bindings, so cached relations are recomputed in
  both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| JS/TS extractor | Return types of functions, methods, and arrow functions; rejection of `Promise<…>`, unions, arrays, function types, and unannotated functions; call-initialized bindings and the destructuring case |
| Import/call linker | A receiver typed by a same-module factory resolves as `typed_receiver_path`; a factory without a return type, a name bound to two factories, and a name-unique factory in an unimported module stay unresolved |
| Recursion | A factory lookup never re-enters the typed step, so cyclic call-initialized bindings cannot loop |
| Persistence | v59 normal/precision state is rejected and forces a v60 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2P — Method Calls Through Declared Receiver Types (bounded Rust slice)

- A Rust method call is now resolved through the receiver's declared type instead of its name.
  `store.write()` was recorded as the bare name `write`, so it linked only when a function of that
  name happened to be in scope, and `reader.read()` on a `&dyn Reader` had nothing to link to at
  all.
- Call sites are objects, not strings: `{callee, name, receiver, kind, line}`. A bare path call
  keeps `receiver: null` and `kind: "call"`; a method call records the receiver expression as
  written and `kind: "method"`.
- A function records the receiver types it states. Parameters and annotated locals — including the
  ones nested in blocks and match arms — become `typed_bindings` rows of `{name, type, kind, line}`,
  where `kind` is `annotation`, `trait_object`, or `impl_trait`. References and `Box`/`Rc`/`Arc` are
  unwrapped to the type they name. A generic argument, a trait object with extra bounds, a
  primitive, `Self`, and an unannotated local name nothing dispatchable, so they are not recorded.
- Resolution is by declaration, not by name. `self.m()` dispatches inside its own impl at `0.8`;
  `x.m()` where `x` is declared as `Store` resolves to `Store::m` at `0.8`; `x.m()` where `x` is a
  `&dyn Trait` or an `impl Trait` resolves to `Trait::m` — the declaration dynamic dispatch calls —
  at `0.7`. A bare path keeps the existing import-aware `resolve_path` at `0.8`. A method no longer
  links to itself.
- Doubt stays unresolved rather than guessed. A type name declared twice owns neither declaration, a
  binding name typed twice types nothing, and a nested receiver such as `self.reader` and a receiver
  the source never types stay unresolved. Finding the concrete impls behind a `dyn` receiver is
  still out of scope: only a type the source states may type a receiver.
- Trait method declarations become entities, because a `dyn` receiver has no target unless the
  trait's own methods exist in the snapshot. This closes the trait-method extraction deferred since
  P2A and does not expand trait implementations into synthetic methods.
- Bump both index-state schema families from v60 to v61 because Rust declaration payloads gain
  structured call sites and receiver types, so cached relations are recomputed in both
  configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Rust extractor | Call sites with receiver, kind, and line; receiver types from parameters, annotated locals, and nested blocks; `Box`/`Rc`/`Arc`, `&dyn`, `impl Trait`; rejection of generics, multi-bound trait objects, primitives, `Self`, and unannotated locals; trait method declarations |
| Rust linker | `rust_self_method`, `rust_typed_receiver_method`, and `rust_dyn_dispatch_method` with their confidences; an untyped, nested, or ambiguously named receiver stays unresolved; a method never links to itself |
| Persistence | v60 normal/precision state is rejected and forces a v61 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2Q — Rust Dyn-Dispatch Candidates (bounded Rust slice)

- A `dyn` call now also names the code that can run. P2P resolved `reader.read()` on a `&dyn Reader`
  to `Reader::read`, the declaration dynamic dispatch invokes, but nothing linked the call to the
  body behind the receiver.
- The types each trait is implemented for come from the `trait_impl` facts that already produce
  `Implements` bridges, so candidates restate evidence the snapshot owns: a trait resolves by unique
  name, a type resolves by unique name, and one type counts once per trait even when it has several
  impl blocks. The order is stable — by qualified name — so the same snapshot yields the same edges.
- A dynamic call site records one candidate edge per implementation's method at `0.5`
  (`rust_dyn_dispatch_candidate`), below the `0.7` declaration edge: the trait proves the interface
  and the impl proves the body exists, not that this call reaches it. A method that does not exist on
  an implementing type contributes nothing.
- A candidate is not a resolution. A concrete receiver claims no candidates, a candidate never
  becomes `TestedBy` coverage — a test that may not run an implementation has not proven it — and a
  candidate never replaces the declaration edge the call dispatches to.
- The list is bounded to eight candidates per call site and the payload discloses the count it
  recorded, so one trait with many implementations cannot flood the graph silently. The receiver rows
  a declaration records are bounded the same way, at 64 with an `omitted_typed_bindings` count; that
  array was the only unbounded payload in either extractor.
- Bump both index-state schema families from v61 to v62 because Rust call relations change, so cached
  relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Rust linker | Candidates for a `&dyn` and an `impl Trait` receiver with their confidence, count, and truncation disclosure; a concrete receiver claims none; a candidate never becomes test coverage; a method missing from an implementation contributes nothing |
| Rust extractor | The receiver-row bound and its omission count |
| Persistence | v61 normal/precision state is rejected and forces a v62 rebuild |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2R — Symbol-Level Incremental Invalidation (bounded core slice)

- A changed file now invalidates the declarations that moved, not every declaration the file happens
  to contain. Extraction is per file, so until now every entity of a changed file was treated as new:
  the dependency closure started from all of them, and every relation the file owned was dropped and
  re-derived, even when the file only gained a blank line.
- A declaration is fingerprinted by everything a linker or checker can read from it — stable key,
  kind, name, title, source span, aliases, ownership, and payload. `id` is left out because it is
  derived from the stable key, and the snapshot is left out because it names the run rather than the
  object. An extracted declaration whose fingerprint equals the one the previous snapshot stated from
  one of the affected paths is reused: it leaves the affected set, so the closure starts smaller.
- Reuse is only ever compared against a previous declaration from an affected path. A declaration
  with no previous state in those paths — a new one, or one that was never extracted from them — is
  new work, and a declaration that moved to another line is reconsidered, because the span is part
  of the fingerprint.
- The objects derived from a reused declaration are carried forward instead of being dropped with the
  file. A relation the changed path owns is still true when every endpoint still exists and one of
  them is a declaration restated identically, because relation evidence is anchored on the endpoints
  themselves and a span that did not move still proves it. Facts follow their subject and diagnostics
  their entities.
- A relation that only passes through the changed file, with both endpoints elsewhere, keeps the
  previous behaviour and is recomputed; anything touching a declaration that moved is dropped as
  before. A relation recomputed later still wins, because canonicalisation keeps the last object per
  id, so carrying an object never masks a newer one.
- No index-state bump: the stored objects are the same canonical objects, and only fewer of them are
  recomputed, so an index built by an earlier version stays valid.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Reuse | A restated declaration is reused; a moved payload, a moved line, an absent previous state, and a previous state from an untouched path are not |
| Carry-forward | Relations, facts, and diagnostics of reused declarations survive a changed path; a relation touching a moved declaration is dropped; a relation through the changed file with both endpoints elsewhere keeps the previous behaviour |
| Pipeline | A changed file invalidates only the declaration that moved, and the relation of the reused declaration is still in the stored snapshot |

Focused verification:

```bash
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2S — Inherited Members Through Declared Heritage (bounded JS/TS slice)

- A call on a typed receiver now also resolves a member the type inherits. Until now the typed step
  read only the members the receiver's own type declares, so `service.reset()` stayed unresolved when
  `reset` lives on `Base` and `Service extends Base` — the single most common shape of polymorphic
  dispatch, and the one a dependency graph misses most often.
- The walk follows only the heritage the source states. A member is claimed when the `extends` chain
  from the receiver's type to the declaring class is unbroken: `Service` names `Base`, so a member of
  `Base` is a member of `Service`. Nothing is inferred about what a class might contain, and a base
  the source never names is never consulted.
- Inheritance is weaker evidence than a direct declaration, so an inherited member resolves at `0.6`,
  below the `0.8`/`0.7` of a member the type declares itself and above the `0.5` name-unique guess it
  replaces. The reason states the difference — `inherited_typed_member` and
  `inherited_typed_receiver_path` instead of `typed_receiver_member` and `typed_receiver_path` — so a
  consumer can read a polymorphic edge as polymorphic.
- The walk is bounded because the chain is resolved by name. At most four hops are followed; a type
  already visited ends the walk, so `Left extends Right extends Left` resolves nothing instead of
  looping. The bound is a constant, not a heuristic that grows with the snapshot.
- Both ends of every hop are resolved, not just the derived type. A base name resolves like the
  heritage linker resolves one — a unique class or interface declaration in the file the current
  type's module points at, and otherwise a name unique across the snapshot — and so does the type
  the walk starts from. Two `Base` declarations in different directories are ambiguous, and a base
  the snapshot never declares ends the walk, so a qualified member name that no declared class owns
  is never claimed by spelling alone. The standing rule that an ambiguous name resolves to nothing
  is not relaxed for inheritance.
- Interfaces are followed the same way but declare no bodies to call, so in practice every resolved
  edge comes from a class chain. The bound and the fail-closed rules are shared with the typed path
  that already existed, so no new knob is introduced.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Direct inheritance | `service.reset()` resolves to `Base.reset` when `Service extends Base` |
| Multi-hop | A member inherited through a chain of base classes resolves inside the four-hop bound |
| Ambiguity, cycles, and absence | Two same-named bases in different directories, a cyclic chain, and a base the snapshot never declares all resolve nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
```

#### P2T — The Element Type A Declared Collection States (bounded JS/TS slice)

- A call on an element of a collection now resolves. Until now the source had to write the type on
  the receiver's own binding, so `for (const item of items) { item.save(); }` and
  `items.forEach((item) => item.save())` stayed unresolved: `item` carries no annotation of its own.
  Iterating a collection is the ordinary way a JS/TS codebase reaches elements, so every method call
  on an element was missing from the call graph.
- Nothing is inferred. The element type is read from the annotation the file writes on the
  collection — `Item[]` and `Array<Item>` both name `Item` — so the type is still one the source
  states, just on the collection rather than on the receiver. A collection the file never annotates
  names nothing.
- Two receivers are typed this way: the iteration variable of a `for...of` loop, and the first
  parameter of an array combinator's callback (`forEach`, `map`, `filter`, `flatMap`, `find`,
  `findIndex`, `findLast`, `findLastIndex`, `some`, `every`). `reduce` takes the accumulator first
  and `for...in` iterates string keys, so neither yields a type.
- An element resolves at `0.65`: below the `0.7` of an annotation the receiver's own binding
  carries, because the source writes it on the collection, and above the `0.6` of an inherited
  member, because it is written in the file itself and needs no name resolution.
- The shapes that name no single element type end the walk instead of being guessed: a nested array,
  a tuple, a union, a generic element, and a wrapper that is not an array. A collection annotation
  no longer records a row for the collection itself, because the wrapper is not a receiver the
  source ever writes — that row previously reduced `Array<Item>` to `Array`, a type no call
  dispatches on.
- The rows join the module's existing `typed_bindings`, so the typed path, the deep member path, and
  the inherited-member walk all apply to an element receiver unchanged. A class field is scoped by
  its owner, so `for (const field of this.fields)` uses `Class.fields`.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Element rows | A `for...of` variable, a `forEach`/`map`/`filter` callback parameter, and a loop over a class field record the element type |
| Rejected forms | `for...in`, `reduce`, a nested array, a tuple, an unannotated collection, and the collection binding itself record nothing |
| Dispatch | A call on an element receiver resolves at confidence 0.65 |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2U — The Awaited Type A Declared Promise States (bounded JS/TS slice)

- A call on a receiver the module awaits now resolves. Until now the factory step read the type a
  factory *returns*, and a factory that returns `Promise<Repo>` declares no bare named return type,
  so `const repo = await createRepo(); repo.save()` stayed unresolved. Nearly every modern
  TypeScript factory is `async`, so the factory slice covered only the synchronous half of the code.
- Nothing is inferred. `Promise<Repo>` and `PromiseLike<Repo>` await to `Repo`, and so does the bare
  annotation of an `async` declaration, because `async function load(): Repo` awaits to a `Repo`. A
  promise of a union, an array, or a nested generic awaits to nothing, because the awaited value is
  not the one the source states.
- An `async` declaration no longer records its bare annotation as the type it returns. It returns a
  promise, so `const pending = createRepo()` holds no `Repo`: the annotation names the awaited value
  and nothing else. A synchronous factory is unchanged.
- The two initializers are distinct receivers even when they call the same factory, because one
  holds the promise and the other holds what it yields: the module records whether the binding
  awaits the call that initializes it, and the linker asks for the awaited type only when it does.
- An awaited receiver resolves at `0.65`, the tier shared with the element of a collection: `0.65` is
  the tier for a type the source writes somewhere other than the receiver's own binding, whether
  that is the collection it came out of or the promise it was awaited from.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Awaited return types | `Promise<Repo>`, `async function load(): Repo`, and an `async` function declaring `Promise<Repo>` all await to `Repo`; a promise of a union or an array awaits to nothing, and an `async` function records no declared return type |
| Awaited bindings | `await f()` records `awaited`, and a bare `f()` does not |
| Dispatch | A call on an awaited receiver resolves at 0.65, and a call on an unawaited promise factory resolves to nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2V — The Type A Return Statement States (bounded JS/TS slice)

- A factory that constructs what it returns now types its receiver. Until now the factory step read
  only the declared return type, so `function createRepo() { return new Repo(); }` — the ordinary way
  to write a factory — declared no type at all and `const repo = createRepo(); repo.save()` stayed
  unresolved. The factory slice had therefore covered only the factories that annotate.
- A declaration records the one type every `return` of its body constructs. A body cannot lie about
  what it builds, so this is the same class of evidence as a `new Type()` initializer, just performed
  in the declaration rather than at the binding. An arrow function with an expression body counts:
  `const makeStore = () => new Store()` returns a `Store`.
- A construction the source performs already outranks an annotation in `typed_bindings` — `const s =
  new Service()` resolves at 0.8 and `let r: Repo` at 0.7 — and the same ordering is kept here, so a
  receiver typed by a constructed return resolves at `0.8` rather than the `0.7` of an annotated
  factory. An annotation still wins when the declaration has one, so this only fills in the
  factories that have none.
- Nothing is inferred from a name or from a call. A `return` that is not a construction, two returns
  that construct different types, a declaration that returns nothing, and a `return` of a nested
  function all name nothing. The nested guard is what keeps a nested arrow's own value out of the
  enclosing declaration's type.
- An `async` declaration that constructs what it returns awaits to that type and returns nothing: it
  returns a promise, so the constructed type is what `await` yields, never what the call returns.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Constructed return types | A function, an arrow with an expression body, a method, and an `async` function record the type they construct; a nested arrow's return does not leak into its enclosing declaration |
| Rejected forms | Two different constructed types, a `return` of a call, a bare `return`, and a declaration with no return record nothing |
| Dispatch | A call on a receiver a factory constructs resolves at 0.8, and a declared annotation beats the constructed type |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2W — The Members An Object Literal States (bounded JS/TS slice)

- A member of an object literal now types its receiver. `const config = { logger: new Logger() }`
  states the type of `config.logger` exactly as an annotation would, and nothing recorded it, so
  `config.logger.flush()` stayed unresolved. Object literals are how a JS/TS codebase wires its
  dependencies, so a whole class of dispatch was missing from the call graph.
- Only a construction is evidence. A property whose value is a literal, a call, a shorthand, or
  another object names nothing, and a literal that constructs nothing records no rows at all, because
  a binding whose members are all unknown cannot be walked into.
- A literal declares no type of its own, so its members are recorded under a **structural type named
  for the binding** — `config$object` for `const config = { … }`, `App.deps$object` for a class field
  — and the binding is recorded with that type. That is what lets the existing typed path walk into
  the literal with no linker change: `config.logger.flush()` resolves to `Logger.flush` with reason
  `typed_receiver_path`.
- The `$` in the name is what keeps a structural type from colliding with a declared one: no
  declaration can contain it, so a structural type can never be mistaken for a class the snapshot
  declares, and `heritage_base` never resolves it.
- A member the literal constructs resolves at `0.8`, the construction tier, because the literal
  performs the construction — the same reason a `return new Type()` outranks an annotated return in
  the slice above. A class field is scoped by its owner, so `this.deps.repo.read()` walks through
  `App.deps` into the literal.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Structural rows | A local literal and a class field record the type of each member they construct, under a structural type named for the binding |
| Rejected forms | A literal that constructs nothing records no rows |
| Dispatch | A member call through a literal resolves at 0.8, and a literal with unknown members resolves to nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### Remaining P2 Gaps

- Type inference for a receiver whose type the source never states — an index access and a literal or
  factory whose body names no construction; package-manager-specific resolution that needs an
  installed tree, such as Yarn `nodeLinker` layouts and patched dependencies. The element, awaited,
  returned, and structural slices below closed the parts of this gap that a construction or an
  annotation the file already writes can answer without inference.

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

#### P5b — Multi-Hop Chains, Blast Radius, And An Anchored Entry (bounded agent-surface slice)

- `explore` keeps one call per question and now also returns the shortest code-flow chain from each entry
  to every entity it reaches two hops or deeper, plus an aggregate blast radius for the same hop budget:
  distinct reached entities, the relations that carried the traversal, per-hop reach counts, open
  diagnostics over the closure, and a bounded sorted file list.
- Traversal is limited to the relation kinds that carry code influence (`Calls`, `Imports`, `Contains`,
  `Extends`, `Implements`, `ImplementedBy`, `TestedBy`, `CoveredByTest`, `Documents`) and walks them in
  both directions; contract-support, environment, and drift edges never carry a chain. `ath impact` stays
  the authoritative deep blast-radius query and `ath change-map` stays the ranked edit-candidate surface.
- Explicit budgets: `depth` (default 2, max 4 hops), `paths` (default 16, max 64 chains), the existing
  `entries`/`links` budgets, a shared 4,096 relation-edge visit bound, and a 24-file blast list. Every
  dropped candidate, truncated link list, truncated chain list, and omitted file is counted and narrated in
  `omitted`, so a partial answer always says that it is partial.
- An entry may be anchored on an exact stable key or repository path through `target`, which reuses the
  `impact`/`change map` target resolver instead of a second one and skips retrieval entirely; a query that
  resolves nothing stays an empty disclosure rather than a guess.
- The same application service is exposed as `ath explore`, so the single-call answer no longer requires an
  MCP client. Both surfaces print the identical versioned `athanor.explore.v1` report; the contract gains
  fields additively and its schema id stays v1.
- Verbatim source text and `node`-class per-symbol file dumps stay out of scope: `explore` cites paths and
  line ranges, and the caller reads the slice it asked for.
- `explore` is still MCP/CLI-only: daemon-served multi-repository explore and auto-synced freshness signals
  remain open on axis C5.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Explore report | One-hop links, chains, blast aggregate, budgets, and disclosures agree for one snapshot; a shallow request reports blast without chains |
| Traversal | Chains start at two hops, keep the shortest path per reached entity, order adjacency and output deterministically, respect the path and visit budgets, and never traverse a non-code-flow relation |
| Anchored entry | `target` seeds entries without retrieval, over-budget candidates are counted, and an unresolvable target fails closed |
| Surfaces | MCP `explore` and `ath explore` accept `depth`/`paths`/`target`; an empty request is rejected before the store is touched |

Focused verification:

```bash
cargo test -p athanor-app explore --locked
cargo test -p ath --test direct_read_cli --locked
cargo test -p athanor-transport-mcp --locked
```

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
test, Clippy, smoke, docs, security, feature, and coverage matrix. P2B is verified by GitHub Actions
run `38742833180` on `5b5f8a1`, including the complete Rust 1.95 matrix on Ubuntu, macOS, and Windows,
source coverage, feature matrices, and the companion docs/security/store gates. P2C adds bounded
source-backed wildcard re-export resolution with v49 index-state rebuilds; exact-commit GitHub
Actions run `38748788007` on `49444d5` passed. P2D adds named default-function import/re-export
resolution and the v50 index-state rebuild; exact-commit GitHub Actions run `38757025492` on
`c105cad` passed on Ubuntu, macOS, and Windows, including workspace tests, Clippy, feature matrices,
coverage, smoke, and editable-doc checks. P2E adds direct `export * as name` mappings for single-member
calls through named imports, keeps missing/ambiguous/cyclic/over-limit namespace targets unresolved,
and bumps index state to v51. Exact-commit CI run `38772624382` on `6aefc28` passed the Ubuntu,
macOS, and Windows Rust 1.95 matrix, all feature matrices, source coverage, smoke, Clippy, editable docs,
and companion AppSec, store, production, and documentation checks. The Windows process-test fixtures
write byte-exact data to redirected standard streams, and CI panic summaries now preserve assertion
context. Slice P4
is covered by the shipped `operations`
documentation profile
(DOCGEN-001 Slices 4A–4C); its monitoring category derives declared health, status, and metrics endpoints
and telemetry environment-variable names from canonical evidence, while container and Kubernetes probes,
alert rules, dashboards, and SLOs still need their own evidence source before they may be documented. Slice P5 is source-implemented as the bounded `explore` MCP tool: one query
returns search-ranked entries, their citations, and the one-hop relation neighbourhood with disclosed
budgets. Slice P5b adds the multi-hop code-flow chains, the aggregate blast radius, an exact
stable-key/path anchor, and the `ath explore` command over the same report; verbatim source bodies and
`node`-class file dumps stay out of scope, and daemon-served freshness stays open on axis C5. That P5b
slice is exact-commit verified on `c48ca69` (CI run `38869503550`, AppSec `38869503667`, Store Conformance
`38869503725`), covering the workspace suite, Clippy `-D warnings`, `cargo fmt --check`, the indexing smoke,
and `ath docs check` across ubuntu, windows, and macos; the axis stays open because the remaining C1/C2
coverage and C5 freshness gaps are untouched. Slice P2G adds callback and promise-handler bridges: the extractor records bounded
`callback_arguments` on allowlisted combinator call sites, and the call linker resolves each
recorded argument through the existing ordered rules into an evidence-backed indirect `Calls`
edge with reason `js_ts_callback_bridge`, bumping both index-state schema families from v52 to
v53. Slice P2H binds named default class exports and admits classes as resolvable call
targets: `export default class Foo {}` records its default name, a default import of it
resolves a `new Foo()` site with reason `imported_default_export`, and a same-module
construction resolves with `same_module_exact_name`, bumping both index-state schema
families from v53 to v54; anonymous default expressions and anonymous default classes stay
unbound. Slice P2I resolves multi-hop namespace aliases: a namespace name forwarded across
modules by source-backed named re-exports resolves a single member call through the declaring
`export * as name` clause, renamed or same-name, within the existing 32-hop and 1,024-visit
bounds, bumping both index-state schema families from v54 to v55; ambiguous, cyclic, and
over-bound forwarded chains stay unresolved. Slice P3 is source-implemented as `builtin.extractor.treesitter`: one engine walks a
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
