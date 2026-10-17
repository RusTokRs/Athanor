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
  P2B covers explicit named re-export chains, P2C bounded single-source wildcard re-exports, P2D
  named default function exports/re-exports, P2E direct namespace re-exports for single-member calls
  through named imports, P2F bounded multiple-source wildcard aggregation, P2G callback and
  promise-handler bridges for allowlisted combinators, P2J module-resolution configuration with
  TypeScript path aliases and workspace-package specifiers, P2K anonymous default exports bound
  to the canonical `default` name, and P2L package `exports` subpaths with conditional exports,
  pnpm workspace layouts, and merged `rootDirs` roots, and P2M typed receivers and deep member
  paths. Receiver/type-aware dispatch across module boundaries remains open.
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

#### Remaining P2 Gaps

- Receiver/type-aware dispatch across module boundaries, where the receiver's type is itself
  imported; package-manager-specific resolution that needs an installed tree, such as Yarn
  `nodeLinker` layouts and patched dependencies.
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
