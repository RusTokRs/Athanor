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
  of it, and P2AJ constrained type parameters so `fn run<T: Reader>(item: T)` types `item` as
  `impl Reader`, and P2X index-access element typing so `items[0].save()` resolves from the collection
  annotation, and P2Y array-literal element constructions so `const items = [new Item()]; items[0].save()`
  resolves from the construction, and P2Z factory wrappers that only return a call so
  `function createRepo() { return makeRepo(); }` types a receiver from `makeRepo` one hop, and P2AA
  peeled returns and awaited async wrappers so `return await makeRepo()` and
  `const repo = await wrap()` resolve, and P2AB type assertions so `return load() as Repo` and
  `const repo = load() as Repo` type a receiver, and P2AC call-result receivers so
  `createRepo().save()` and `new Repo().save()` resolve from the factory or constructor the source
  names, and P2AD member-factory receivers so `svc.build().save()` and
  `createRepo().getStore().save()` resolve from the method's stated return type, and P2AE inline
  receiver assertions so `(value as Repo).save()` resolves from the type the source writes, and P2AF
  qualified constructions so `new ns.Repo().save()` resolves from the terminal constructor name, and
  P2AG `super` calls so `super.save()` resolves through the declared `extends` clause, and P2AH
  type-alias unfolding so `type Client = Repo` types `repo.save()` as `Repo.save` one hop, and P2AI
  constrained type parameters so `function run<T extends Repo>(repo: T)` types `repo` as `Repo`, and
  P2AJ Rust constrained type parameters so `fn run<T: Reader>(item: T)` types `item` as `impl Reader`,
  and P2AK Rust type-alias unfolding so `type Client = Store` types `item.write()` as `Store::write`
  one hop, and P2AL Rust nested `self.field` receivers so `self.reader.write()` resolves from the
  named field the owner struct declares, and P2AM JS/TS constrained return types so
  `function create<T extends Repo>(): T` types `create().save()` as `Repo.save`, and P2AN
  invoked factory results so `function create(): () => Repo` types `create()().save()` as
  `Repo.save`, and P2AO Rust factory return types so `fn create_repo() -> Store` types
  `create_repo().write()` as `Store::write`, and P2AP Rust constrained factory returns so
  `fn wrap<T: Reader>() -> T` types `wrap().read()` as `Reader::read`, and P2AQ Rust
  member-factory receivers so `svc.create().write()` and `self.create().write()` type
  from the method's declared return, and P2AR Rust nested factory hops so
  `create_repo().get().write()` types from the inner method's declared return, and P2AS Rust
  associated-function factories so `Store::new().write()` types from that associated
  function's declared return, and P2AT Rust factory-initialized bindings so
  `let store = create_repo(); store.write()` types from that factory's declared
  return, and P2AU Rust member-factory initialized bindings so
  `let store = svc.create(); store.write()` types from that method's declared
  return, and P2AV Rust crate-path factories so `crate::create_repo().write()`
  and `crate::app::Store::new().write()` type from that path, and P2AW Rust nested
  associated-factory hops so `Store::new().get().write()` types from the inner
  method's declared return, and P2AX Rust nested factory-initialized bindings so
  `let store = create_repo().get(); store.write()` types from that inner method's
  declared return, and P2AY Rust try-operator factory yields so
  `create_repo()?.write()` and `let store = create_repo()?; store.write()` take
  the Ok/Some type a `Result`/`Option` factory declares, and P2AZ Rust nested
  crate-path factory hops so `crate::create_repo().get().write()` types from
  the inner method's declared return, and P2BA Rust inner-try factory hops so
  `create_repo()?.get().write()` and `let store = create_repo()?.get(); store.write()`
  type from the inner Result/Option factory then the next method, and P2BB Rust unwrap/expect
  factory yields so `create_repo().unwrap().write()` and `let store = create_repo().unwrap(); store.write()`
  take the same Ok/Some type, and P2BC Rust inner-unwrap factory hops so
  `create_repo().unwrap().get().write()` and `let store = create_repo().unwrap().get(); store.write()`
  type from the inner Result/Option factory then the next method, and P2BD Rust inherent `-> Self`
  factory returns so `impl Store { fn new() -> Self }` types `Store::new().write()` as `Store::write`. Type inference for a receiver whose type the source never states — a factory whose body names
  neither a construction, a callee, nor an assertion — remains open.
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

#### P2X — The Element Type An Index Access Reaches (bounded JS/TS slice)

- A call on an indexed element of a collection now resolves. Until now `items[0].save()` and
  `this.fields[i].save()` were not even recorded as call sites: `callee_path` omitted a
  `subscript_expression`, so every method call through an index was missing from the call graph.
  Indexing a collection is as ordinary as iterating it, and P2T only typed the loop variable and
  the combinator callback — not the element reached by `[]`.
- Nothing is inferred. The element type is the same annotation P2T already reads — `Item[]` and
  `Array<Item>` both name `Item` — recorded under the synthetic name `items[]` / `Class.fields[]`
  so the existing typed path can walk it. A class field is scoped by its owner, so
  `repo.items[0].save()` reads `Repo.items[]` from the module that declares `Repo`.
- The call is named `items[].save`: the index is not a name, so `items[0]` and `items[i]` are the
  same receiver. A computed call of the element itself (`handlers[key]()`), nested index access
  (`items[0][1].save()`), and a parenthesized or call-result index stay omitted rather than guessed.
- An indexed element resolves at `0.65`, the same derived-type tier as a `for...of` variable: the
  source writes the type on the collection, not on the receiver. An untyped index does not fall
  through to a unique member name — a unique `save` in the file is not evidence that `items[i]`
  holds that type.
- The shapes that name no single element type still end the walk: a nested array, a tuple, a union,
  a generic element, and a collection the file never annotates. A collection annotation still
  records no row for the collection itself.
- Bump both index-state schema families from v62 to v63 because module payloads gain `name[]`
  element rows for every collection annotation and declaration payloads gain index-access call
  sites, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Element rows | A local `Item[]` / `Array<Item>`, a parameter, a class field, and an interface property record `name[]` with kind `element` |
| Call sites | `items[0].save()` and `this.fields[i].save()` record as `items[].save` / `this.fields[].save`; `handlers[key]()` and `items[0][1].save()` stay omitted |
| Rejected forms | A nested array, a tuple, an unannotated collection, and the collection binding itself still record nothing for the wrapper |
| Dispatch | A call on an indexed element resolves at confidence 0.65; an untyped index does not guess a unique member |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2Y — The Element Type An Array Literal Constructs (bounded JS/TS slice)

- A call on an element of an array literal now resolves. Until now `const items = [new Item()];
  items[0].save()` recorded the call as `items[].save` (P2X) but had no type for `items[]` unless the
  source also wrote `Item[]`. Array literals are how a JS/TS file seeds a collection without an
  annotation, so every method call on those elements was missing from the call graph.
- Nothing is inferred. Every named element of the literal must be a `new Type()` of the same type —
  the same evidence as a `return new Type()` and as an object-literal member that constructs. An
  empty array, a hole, a spread, a nested array, a call element, and two different constructed types
  name nothing.
- The row is `items[]` / `Class.fields[]` with kind `construction`, so P2X's indexed callee path
  walks it unchanged and the call resolves at `0.8`. An annotation on the same collection still
  records `element` at `0.65`; the linker keeps the stronger construction when both name the same
  type, and an annotation that disagrees with the constructed type stays unresolved.
- Bump both index-state schema families from v63 to v64 because module payloads gain construction
  rows for array literals, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Construction rows | A local `[new Item(), new Item()]` and a class field record `name[]` with kind `construction` |
| Rejected forms | An empty array, mixed constructed types, a nested array, a spread, and a call element record nothing |
| Dispatch | A call on a constructed array element resolves at confidence 0.8 |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2Z — The Type A Wrapper Forwards From The Factory It Calls (bounded JS/TS slice)

- A call on a receiver a wrapping factory produced now resolves. Until now `function createRepo() {
  return makeRepo(); }` recorded no type: the body names no construction, so P2V left the wrapper
  untyped and `const repo = createRepo(); repo.save()` stayed off the call graph. Wrappers that only
  forward to another factory are how a JS/TS file hides a construction behind a name, and the callee
  is written in the source.
- Nothing is inferred. Every `return` of the wrapper must be a call to the same bare identifier —
  the same uniqueness as a constructed return. A construction already names the type, so it wins and
  records no callee. Two callees, a mix of construction and call, a member callee, and an `async`
  wrapper (it returns a promise) name nothing. A nested function's return is skipped, not attributed
  to the wrapper.
- The linker resolves that callee from the wrapper's module, one hop, and inherits the inner
  factory's annotation or constructed return. It does not follow another `returned_callee`: a chain
  of wrappers is not the type the source states. A factory reached only by a unique-name guess, and
  an inner factory that yields a promise, still type nothing.
- Bump both index-state schema families from v64 to v65 because declaration payloads gain
  `returned_callee`, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Returned callee | A function, arrow, and method that only `return makeRepo()` record `returned_callee`; a nested `function () {}` is skipped |
| Rejected forms | Mixed construction and call, two callees, a member callee, and an `async` wrapper record nothing |
| Dispatch | A receiver a wrapper produced resolves through the inner factory one hop; a second hop and an inner promise factory stay unresolved |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2AA — Peeled Returns And Awaited Async Wrappers (bounded JS/TS slice)

- A construction or call wrapped in parentheses, `await`, `as Type`, or a non-null assertion is the
  same evidence as the bare form: `return (new Repo())` records `returned_type`, and
  `return await makeRepo()` records `returned_callee`. An arrow expression body peels the same way.
- An `async` wrapper records `returned_callee` too. The linker types `const repo = await wrap()`
  from the inner factory one hop (annotation, constructed return, or awaited type) and refuses
  `const pending = wrap()`: that binding holds a promise. A second hop still names nothing.
- Call-initialized bindings peel the same wrappers, so `const repo = await (createRepo())` is
  awaited and `const pending = (createRepo())` is not.
- Bump both index-state schema families from v65 to v66 because async declarations gain
  `returned_callee` and cached awaited relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Peeled construction | `return (new Repo())`, `() => (new Store())`, and `return await new Repo()` record `returned_type` |
| Peeled callee | `return (makeRepo())`, `() => (makeStore())`, `async function load() { return makeRepo(); }`, and `return await makeRepo()` record `returned_callee` |
| Dispatch | `await wrap()` through an async wrapper resolves; an unawaited async wrapper stays unresolved |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2AB — The Type An Assertion Writes (bounded JS/TS slice)

- A call on a receiver whose type is only written as `as Repo` now resolves. Until now
  `return load() as Repo` and `const repo = load() as Repo` recorded no type: the peel looked
  through `as` for a construction or a factory call, and the asserted name was discarded. The
  assertion is written in the source.
- Nothing is inferred. Every `return` must assert the same bare type. `as const`, a union, an
  array, and two disagreeing assertions name nothing. A construction already names the type, so it
  wins and records no assertion.
- The declaration records `asserted_type`, and an initializer assertion records a typed-binding
  row of kind `annotation`, so `repo.save()` resolves at `0.7`. An `async` wrapper awaits to the
  asserted type the same way it awaits to a constructed return.
- Bump both index-state schema families from v66 to v67 because declaration payloads gain
  `asserted_type`, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Asserted return | A function, arrow, and method that `return load() as Repo` record `asserted_type` |
| Rejected forms | `as const`, mixed assertions, a construction, and a `return` with no assertion record nothing |
| Dispatch | A receiver a factory or initializer asserts resolves at confidence 0.7 |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2AC — Call-Result Receivers (bounded JS/TS slice)

- A member call whose receiver is a factory or construction the source names now resolves. Until
  now `createRepo().save()` and `new Repo().save()` were omitted: `callee_path` required an
  identifier-rooted member, and a call or `new` as the object named nothing. The factory or
  constructor is written in the source.
- Nothing is inferred. The factory must be a bare identifier. A member factory
  (`svc.build().save()`), a nested call result (`createRepo().getStore().save()`), a call of a
  call result (`factory()()`), and an inline `as` / type assertion on the receiver stay omitted.
  An untyped factory result does not fall through to a unique member name.
- The extractor records `createRepo().save`, `(await createRepo()).save()` as `createRepo()@.save`,
  and `new Repo().save()` as `Repo{}.save`. The linker types those synthetic roots with the same
  factory evidence a `const repo = createRepo()` binding uses, so annotation, constructed return,
  asserted return, one forwarded hop, and an awaited promise apply unchanged. A field walk
  (`createRepo().items.save()`) continues from that root.
- Bump both index-state schema families from v67 to v68 because declaration `calls` arrays gain
  these callee shapes, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Factory result | `createRepo().save()`, `(createRepo()).load()`, and `createRepo()!.close()` record `createRepo().…` |
| Awaited result | `(await createRepo()).flush()` records `createRepo()@.flush` |
| Construction | `new Repo().reset()` records `Repo{}.reset` |
| Rejected forms | A member factory, a nested call result, `factory()()`, and `(createRepo() as Repo).save()` record nothing for the outer member |
| Dispatch | A factory-result call resolves at the factory's stated tier; a construction result at 0.8; an untyped factory result stays unresolved |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2AD — Member-Factory Receivers (bounded JS/TS slice)

- A member call whose receiver is a call of a named method now resolves. Until now
  `svc.build().save()` and `createRepo().getStore().save()` were omitted: the factory of a
  call-result receiver had to be a bare identifier. The method and its return type are written
  in the source.
- Nothing is inferred. The factory path is an identifier-rooted member. A call of a call result
  (`factory()()`), an inline `as` / type assertion, and an untyped member factory stay omitted.
  An untyped path does not fall through to a unique member name.
- The extractor records `svc.build().save` and `createRepo().getStore().save`. The linker walks
  a `()` segment as a call of that method on the current type and continues from the method's
  stated return type — annotation, constructed return, asserted return, one forwarded hop, or
  an awaited promise — including a method the current type inherits through `extends`.
- Bump both index-state schema families from v68 to v69 because declaration `calls` arrays gain
  these callee shapes, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Member factory | `svc.build().save()` and `this.factory().save()` record and resolve from the method's return type |
| Nested call | `createRepo().getStore().save()` records and resolves through both stated returns |
| Inherited method | `this.build().save()` inside a subclass resolves from the base method's return type |
| Rejected forms | `factory()()`, `(createRepo() as Repo).save()`, and an untyped `svc.build().save` stay unresolved |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2AE — Inline Receiver Assertions (bounded JS/TS slice)

- A member call whose receiver is typed only by an inline assertion now resolves. Until now
  `(value as Repo).save()` was omitted: the peel treated `as` as unknown and discarded the type
  the source writes. The assertion is written at the call site.
- Nothing is inferred. The assertion must name one bare type. `as const`, a union, an array, and
  a type assertion the grammar does not name stay omitted. An assertion of a type with no such
  member does not fall through to a unique name.
- The extractor records `Repo#.save` for `as Repo`, `<Repo>expr`, and `satisfies Repo`. The linker
  starts the typed walk at `Repo` with annotation confidence 0.7, the same tier as a binding
  assertion. A field walk (`(value as Repo).items.save()`) continues from that root.
- Bump both index-state schema families from v69 to v70 because declaration `calls` arrays gain
  these callee shapes, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| `as` / assertion / satisfies | `(value as Repo).save()`, `(<Store>value).load()`, and `(value satisfies Item).flush()` record `Type#.…` |
| Field walk | `(value as Repo).items.save()` records `Repo#.items.save` |
| Rejected forms | `as const` and `as Repo \| Other` record nothing for the outer member |
| Dispatch | An asserted receiver resolves at 0.7; an assertion of a type with no such member stays unresolved |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2AF — Qualified Construction Receivers (bounded JS/TS slice)

- A member call whose receiver is `new ns.Repo()` now resolves. Until now only a bare
  `new Repo().save()` recorded as `Repo{}.save`: the constructor had to be an identifier.
  `new ns.Repo()` names the type in the source; the constructed type is the terminal identifier,
  the same reduction heritage names already use.
- Nothing is inferred. A computed constructor (`new ns[key].Repo()`) names nothing. The inner
  `new ns.Repo` call site is still recorded separately as `ns.Repo`.
- The extractor records `Repo{}.save` for both `new Repo().save()` and `new ns.Repo().save()`.
  The linker already types `Repo{}` as a construction at 0.8.
- Bump both index-state schema families from v70 to v71 because declaration `calls` arrays gain
  these callee shapes, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Qualified `new` | `new ns.Repo().save()` and `(new ns.Repo()).load()` record `Repo{}.…` |
| Bare `new` | `new Repo().reset()` still records `Repo{}.reset` |
| Rejected forms | `new ns[key].Repo().wipe()` stays omitted |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2AG — Super Calls Through Declared Extends (bounded JS/TS slice)

- A member call whose receiver is `super` now resolves. The extractor already recorded
  `super.save` because `super` is an identifier; the linker treated it as an untyped name and
  either missed the base method or guessed a unique `save` in the calling module.
- Nothing is inferred. `super` names the class the owner `extends`. A method on a class with no
  heritage clause, or a `super` call outside a method owner, stays unresolved. The walk does not
  fall through to a unique member name.
- The linker roots the typed walk at the first `extends` name and dispatches at construction
  confidence 0.8, the same evidence `this.method()` uses for the owner itself.
- Bump both index-state schema families from v71 to v72 because cached `calls` relations now
  include these edges, so earlier snapshots reindex in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Super member | `super.save()` on a class that `extends Base` resolves to `Base.save` |
| Fail-closed | `super.save()` with no declared base does not guess a unique `save` |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2AH — Type-Alias Unfolding (bounded JS/TS slice)

- A member call whose receiver is annotated with a type alias now resolves. Until now
  `type Client = Repo` recorded the alias as a symbol with no right-hand side, so
  `const repo: Client; repo.save()` looked for `Client.save`.
- Nothing is inferred. The extractor records `aliased_type` when the right-hand side is a
  bare named type, including a qualified or generic name reduced to its terminal identifier.
  A union, an array, and a function type name nothing. The linker unfolds one hop; a second
  alias in the chain stays unresolved.
- Type aliases are indexed by name for this unfold and are not call targets: a unique
  exported alias does not resolve a bare `Client()` call.
- Bump both index-state schema families from v72 to v73 because type-alias payloads gain
  `aliased_type`, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Bare alias | `type Client = Repo`; `repo: Client`; `repo.save()` resolves to `Repo.save` |
| Fail-closed | a union or unnamed alias does not unfold to a unique member |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2AI — Constrained Type Parameters (bounded JS/TS slice)

- A member call whose receiver is annotated with a type parameter now resolves when that
  parameter `extends` one named type. Until now `function run<T extends Repo>(repo: T)`
  recorded `repo` as `T`, so `repo.save()` looked for `T.save`.
- Nothing is inferred. The extractor substitutes the innermost usable `extends` bound:
  a class, method, or function type parameter that names one type, including a qualified
  or generic bound reduced to its terminal identifier. An unconstrained parameter, a union
  bound, and a default without `extends` leave the parameter name. The linker already
  types an annotation at 0.7.
- Bump both index-state schema families from v73 to v74 because `typed_bindings` rows
  change, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Constrained param | `function run<T extends Repo>(repo: T)` records `repo` as `Repo` |
| Class / method / interface | `class App<T extends Item> { repo: T }` and `load<T extends Cache>(cache: T)` record the bound |
| Fail-closed | unconstrained `T`, `T extends Repo \| Store`, and `T = Repo` do not record the bound |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-extractor-js-ts --features precision-parser --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
cargo test -p athanor-app --features js-ts-precision --locked
```

#### P2AJ — Constrained Type Parameters (bounded Rust slice)

- A method call whose receiver is a type parameter now resolves when that parameter names one
  trait bound. Until now `fn run<T: Reader>(item: T)` recorded `item` as `T`, so `item.read()`
  looked for a type named `T`.
- Nothing is inferred. The extractor substitutes the one trait `T: Reader` names, including a
  `where T: Reader` clause and a bound on the enclosing impl or trait, and records kind
  `impl_trait` so the linker dispatches like `impl Reader` at 0.7. Unconstrained `T`,
  `T: Reader + Send`, and `T: Into<U>` leave the parameter name.
- Bump both index-state schema families from v74 to v75 because `typed_bindings` rows change,
  so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Constrained param | `fn run<T: Reader>(item: T)` records `item` as `Reader` kind `impl_trait` |
| Where / impl / trait | `where T: Reader`, `impl<T: Reader>`, and `trait BoxLike<T: Reader>` record the bound |
| Fail-closed | unconstrained `T`, `T: Reader + Send`, and `T: Into<U>` do not record the bound |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AK — Type-Alias Unfolding (bounded Rust slice)

- A method call whose receiver is annotated with a type alias now resolves. Until now
  `type Client = Store` recorded the alias as a symbol with no right-hand side, so
  `fn run(item: Client) { item.write(); }` looked for `Client::write`.
- Nothing is inferred. The extractor records `aliased_type` when the right-hand side is a
  bare named type, including a qualified path reduced to its last segment. A generic, a
  tuple, a reference, and a primitive name nothing. The linker unfolds one hop; a second
  alias in the chain stays unresolved.
- Bump both index-state schema families from v75 to v76 because type-alias payloads gain
  `aliased_type`, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Bare alias | `type Client = Store`; `item: Client`; `item.write()` resolves to `Store::write` |
| Fail-closed | a generic, tuple, or unnamed alias does not unfold to a unique member |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AL — Nested `self.field` Receivers (bounded Rust slice)

- A method call whose receiver is a field of `self` now resolves. Until now
  `self.reader.write()` stayed unresolved because the extractor recorded the
  receiver as pretty-printed tokens and the linker only dispatched `self` or a
  binding name the function itself typed.
- Nothing is inferred. Named struct fields are recorded as `typed_bindings` on
  the struct. The extractor collapses spaces around `.` and wrapping parentheses
  so `self.reader` is one token path. The linker peels wrapping parentheses and
  a leading `&`/`mut` and walks one hop from the
  inherent-impl owner to that field's declared type, unfolding a type alias the
  same way a parameter does. A second hop, a missing field, a tuple or unit
  struct, and a trait-impl qualified name that is not the struct stay unresolved.
- Bump both index-state schema families from v76 to v77 because struct payloads
  gain field `typed_bindings`, so cached relations are recomputed in both
  configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Named field | `struct App { reader: Store }`; `self.reader.write()` resolves to `Store::write` |
| Fail-closed | `self.reader.inner.write()`, a missing field, an ambiguous `cfg` field, and a tuple struct do not resolve |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AM — Constrained Return Types (bounded TypeScript slice)

- A factory whose declared return type is a type parameter now records the bound.
  Until now `function create<T extends Repo>(): T` recorded `T`, so
  `const repo = create(); repo.save()` looked for a type named `T`.
- Nothing is inferred. The same `extends` bound a `T` parameter already uses
  substitutes the return type, including a class generic (`class App<T extends
  Item> { build(): T }`), a method generic, and the awaited type of
  `async function wrap<T extends Repo>(): T` / `(): Promise<T>`. An unconstrained
  parameter, a union bound, and a default without `extends` leave the parameter
  name.
- Bump both index-state schema families from v77 to v78 because return-type
  payloads substitute the bound, so cached relations are recomputed in both
  configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Constrained return | `function create<T extends Repo>(): T` records `Repo` |
| Fail-closed | unconstrained `T`, `T extends Repo \| Store`, and `T = Repo` do not take the bound |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
```

#### P2AN — Invoked Factory Results (bounded TypeScript slice)

- A factory whose declared return type is a function type now records the yield.
  Until now `function create(): () => Repo` named nothing, so `create()().save()`
  and `const repo = create()()` stayed unresolved. The annotation is not
  `return_type`: `create()` itself is a function, not a `Repo`.
- Nothing is inferred. One extra call hop on an identifier factory records
  `create()()`, and a `call_bindings` row marks `invoked` so the linker reads
  `returned_function_type`. A parenthesized function type, a parameterized
  function type (`(n: number) => Repo`), and a type-parameter bound
  (`(): () => T` with `T extends Item`) are the same yield. A second function
  hop (`() => () => Repo`), a generic function type, a constructor type, an
  `async` factory, a third call hop, an awaited invocation, and a member factory
  as that extra hop (`svc.build()()`) name nothing.
- Bump both index-state schema families from v78 to v79 because declaration
  payloads gained `returned_function_type` and `call_bindings` rows may mark
  `invoked`, so cached relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Function-type yield | `function create(): () => Repo` records `Repo` as `returned_function_type`, not `return_type` |
| Extra call hop | `create()().save()` records `create()().save` and resolves from that yield |
| Invoked binding | `const repo = create()()` records `invoked` and types `repo.save()` |
| Fail-closed | `create().save()`, `create()()().save()`, `await create()()`, `svc.build()()`, `(): () => () => Repo`, and `async function wrap(): () => Repo` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-js-ts --locked
cargo test -p athanor-linker-js-ts --locked
cargo test -p athanor-app --locked
```

#### P2AO — Rust Factory Return Types (bounded Rust slice)

- A function whose signature names the type it returns now records that type.
  Until now `fn create_repo() -> Store` left `create_repo().write()` untyped,
  because only parameters, annotated locals, and struct fields were receiver
  evidence.
- Nothing is inferred. The annotation is the same reduction a parameter uses:
  `-> Store`, `-> Box<Store>`, `-> Rc<Store>`, `-> Arc<Store>`, and `-> &Store`
  record `Store`. An `impl`/`dyn` return, a type parameter (`-> T`, even with
  `T: Reader`), a tuple, a generic other than Box/Rc/Arc, and a missing `->`
  name nothing. The linker types `create_repo().write()` from that return type
  when the factory is a local or imported identifier; `svc.create().write()`
  and `create_repo()().write()` stay unresolved.
- Bump both index-state schema families from v79 to v80 because function
  payloads gained `return_type`, so cached relations are recomputed in both
  configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Declared return | `fn create_repo() -> Store` records `Store` |
| Wrapper peel | `-> Box<Store>` and `-> &'static Store` record `Store` |
| Call result | `create_repo().write()` resolves to `Store::write` |
| Fail-closed | `-> T`, `-> Box<dyn Reader>`, `fn unit()`, `create_repo()()`, and `svc.create()` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AP — Constrained Factory Returns (bounded Rust slice)

- A factory whose declared return type is a type parameter now records the bound.
  Until now `fn wrap<T: Reader>() -> T` named nothing, so `wrap().read()` stayed
  unresolved. The same reduction a `T` parameter already uses substitutes the
  return: `T: Reader` and `where T: Reader` record `Reader` as `impl_trait`,
  `-> impl Reader` is the same bound, and `-> Box<dyn Reader>` records
  `trait_object`. Unconstrained `T`, extra bounds (`T: Reader + Send`), and a
  missing `->` name nothing.
- Nothing is inferred. The linker reads `return_kind` so a trait-shaped return
  dispatches at 0.7 with candidates, while `-> Store` stays a concrete
  annotation at 0.8.
- Bump both index-state schema families from v80 to v81 because return-type
  payloads gained `return_kind` and substitute the bound, so cached relations
  are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Constrained return | `fn wrap<T: Reader>() -> T` records `Reader` as `impl_trait` |
| Trait return | `-> impl Reader` and `-> Box<dyn Reader>` record `Reader` |
| Call result | `wrap().read()` resolves to `Reader::read` at 0.7 |
| Fail-closed | unconstrained `T`, `T: Reader + Send`, and a missing `->` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AQ — Member-Factory Receivers (bounded Rust slice)

- A method call whose receiver is a member factory now resolves. Until now
  `svc.create().write()` and `self.create().write()` stayed unresolved: P2AO
  typed only an identifier factory (`create_repo().write()`). The method and
  its return type are already in the snapshot.
- Nothing is inferred. The factory is one identifier or `self`, then one
  method call. The linker looks up `{Type}::{create}` or `{enclosing}::{create}`
  and reads that method's `return_type` / `return_kind`. A two-hop walk
  (`svc.create().inner.write()`), a missing method (`svc.other()`), an untyped
  owner, an untyped method, and a second factory hop (`create_repo().get().write()`)
  name nothing.
- The extractor already records `svc.create()` and `self.create()` as the
  receiver expression of the outer method. Cached call relations change because
  those sites now resolve.
- Bump both index-state schema families from v81 to v82 because cached `Calls`
  relations gain these edges, so they are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Member factory | `svc.create().write()` resolves to `Store::write` from `App::create` |
| Self factory | `self.create().write()` resolves from the enclosing type's `create` |
| Fail-closed | `svc.create().inner.write()`, `svc.other()`, an untyped `svc`, and `svc.build()` without a return type name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AR — Nested Factory Hops (bounded Rust slice)

- A method call whose receiver is one extra factory hop now resolves. Until now
  `create_repo().get().write()` and `svc.create().get().write()` stayed
  unresolved: P2AO typed `create_repo().write()` and P2AQ typed
  `svc.create().write()`, each one call. The inner method and its return type
  are already in the snapshot.
- Nothing is inferred. The chain is at most two call hops. The linker reads
  the first factory's `return_type`, then `{Type}::{get}`'s `return_type`. A
  third hop (`create_repo().get().extra().write()`), a field after the calls
  (`create_repo().get().inner.write()`), a missing method, and an untyped
  inner method name nothing.
- The extractor already records `create_repo().get()` as the receiver of the
  outer method. Cached call relations change because those sites now resolve.
- Bump both index-state schema families from v82 to v83 because cached `Calls`
  relations gain these edges, so they are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Nested identifier | `create_repo().get().write()` resolves to `Inner::write` from `Store::get` |
| Nested member | `svc.create().get().write()` and `self.create().get().write()` resolve the same way |
| Fail-closed | `create_repo().get().extra()`, `create_repo().get().inner.write()`, `create_repo().other()`, and `Store::extra` without a return type name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AS — Associated-Function Factories (bounded Rust slice)

- A method call whose receiver is an associated-function factory now resolves.
  Until now `Store::new().write()` stayed unresolved: P2AO typed a free-function
  factory and P2AQ typed `svc.create()`, but `Type::factory()` is the usual Rust
  constructor. The associated function and its return type are already in the
  snapshot.
- Nothing is inferred. The type is one identifier. The linker looks up
  `{Type}::{new}` and reads `return_type` / `return_kind`. A path-qualified type
  (`crate::app::Store::new()`), a nested hop (`Store::new().get().write()`), a
  missing method, and an untyped associated function name nothing.
- The extractor already records `Store::new()` as the receiver, collapsing
  pretty-printed `::` the same way it collapses `.`. Cached call relations
  change because those sites now resolve.
- Bump both index-state schema families from v83 to v84 because cached `Calls`
  relations gain these edges, so they are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Associated factory | `Store::new().write()` resolves to `Store::write` from `Store::new` |
| Fail-closed | `crate::app::Store::new()`, `Store::new().get()`, `Store::other()`, and `Store::build` without a return type name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AT — Factory-Initialized Bindings (bounded Rust slice)

- A method call whose receiver is a local initialized by a factory now resolves.
  Until now `let store = create_repo(); store.write()` stayed unresolved: only
  an annotated `let store: Store` or an inline `create_repo().write()` carried
  a type. The factory's declared return is already in the snapshot.
- Nothing is inferred. The extractor records `call_bindings` for an unannotated
  `let ident = factory()` where the factory is one identifier or one associated
  function. The linker reads that callee's `return_type`. An annotated let still
  wins. A nested call (`create_repo().get()`), a member factory (`svc.create()`),
  a path-qualified callee (`crate::create_repo()`), an untyped factory, and a
  name bound to two factories name nothing.
- Bump both index-state schema families from v84 to v85 because function
  payloads gained `call_bindings`, so cached relations are recomputed in both
  configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Identifier init | `let store = create_repo(); store.write()` resolves to `Store::write` |
| Associated init | `let store = Store::new(); store.write()` resolves to `Store::write` |
| Fail-closed | `create_repo().get()`, `svc.create()`, `crate::create_repo()`, an untyped factory, and two disagreeing factories name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AU — Member-Factory Initialized Bindings (bounded Rust slice)

- A local initialized by a member factory now types the receiver. Until now
  `let store = svc.create(); store.write()` stayed unresolved: P2AT recorded
  `create_repo()` and `Store::new()`, but `svc.create()` is a method call.
  The method's declared return is already in the snapshot.
- Nothing is inferred. The extractor records `call_bindings` callee
  `svc.create` / `self.create`. The linker reads `{Type}::{create}` or
  `{enclosing}::{create}`. An untyped owner, a missing method, and a nested
  init (`svc.create().get()`) name nothing.
- Bump both index-state schema families from v85 to v86 because `call_bindings`
  callees gained member-factory shapes, so cached relations are recomputed in
  both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Member init | `let store = svc.create(); store.write()` resolves to `Store::write` |
| Self init | `let store = self.create(); store.write()` resolves from the enclosing type's `create` |
| Fail-closed | an untyped `svc`, `svc.other()`, and `svc.create().get()` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AV — Crate-Path Factories (bounded Rust slice)

- A method call whose factory is a `crate::` / `self::` / `super::` path now
  resolves. Until now `crate::create_repo().write()` and
  `crate::app::Store::new().write()` stayed unresolved: P2AO and P2AS typed a
  bare identifier or `Type::new()`, but real Rust constructors are often
  crate-qualified. The path resolves like a static call.
- Nothing is inferred. The extractor records `crate::create_repo()` as the
  receiver and `crate::create_repo` as a `call_bindings` callee. The linker
  reads that function's `return_type`. A missing path, a nested hop
  (`crate::create_repo().get()`), and a relative `app::create_repo()` name
  nothing.
- Bump both index-state schema families from v86 to v87 because cached `Calls`
  relations gain these edges and `call_bindings` callees gained crate-path
  shapes, so they are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Path identifier | `crate::create_repo().write()` resolves to `Store::write` |
| Path associated | `crate::app::Store::new().write()` resolves to `Store::write` |
| Path init | `let store = crate::create_repo(); store.write()` resolves |
| Fail-closed | `crate::other()`, `crate::create_repo().get()` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AW — Nested Associated-Factory Hops (bounded Rust slice)

- A method call one hop past an associated-function factory now resolves. Until
  now `Store::new().get().write()` stayed unresolved: P2AS typed `Store::new().write()`
  and P2AR typed `create_repo().get().write()`, but `Store::new().get()` is the
  associated constructor plus one extra stated method. The inner method's
  declared return is already in the snapshot.
- Nothing is inferred. The extractor records receiver `Store::new().get()`. The
  linker reads `Store::new`'s return, then that type's `get`. A third hop
  (`Store::new().get().extra()`), a missing inner method, an untyped inner
  method, and a path-qualified nested hop (`crate::app::Store::new().get()`)
  name nothing.
- Bump both index-state schema families from v87 to v88 because cached `Calls`
  relations gain these edges, so they are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Nested associated | `Store::new().get().write()` resolves to `Inner::write` |
| Fail-closed | `Store::new().other()`, `Store::new().extra()`, `Store::new().get().extra()`, `crate::app::Store::new().get()` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AX — Nested Factory-Initialized Bindings (bounded Rust slice)

- A local initialized by one extra factory hop now types the receiver. Until now
  `let store = create_repo().get(); store.write()` stayed unresolved: P2AT recorded
  `create_repo()` and P2AR typed `create_repo().get().write()`, but the nested
  init was omitted. The inner method's declared return is already in the snapshot.
- Nothing is inferred. The extractor records `call_bindings` callee
  `create_repo().get` / `Store::new().get` / `svc.create().get`. The linker
  reads the inner factory return, then that type's method. A nested crate-path
  (`crate::create_repo().get()`), a third hop, a missing inner method, and an
  untyped inner method name nothing.
- Bump both index-state schema families from v88 to v89 because `call_bindings`
  callees gained nested-hop shapes, so cached relations are recomputed in both
  configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Nested ident init | `let store = create_repo().get(); store.write()` resolves to `Inner::write` |
| Nested associated init | `let store = Store::new().get(); store.write()` resolves |
| Nested member init | `let store = svc.create().get(); store.write()` resolves |
| Fail-closed | `crate::create_repo().get()`, `create_repo().get().extra()`, `create_repo().other()` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AY — Try-Operator Factory Yields (bounded Rust slice)

- A trailing `?` now types the receiver from the Ok/Some type a factory
  declares. Until now `create_repo()?.write()` and `let store = create_repo()?;`
  stayed unresolved: P2AO typed `create_repo().write()` from `-> Store`, but
  real constructors return `Result<Store, E>` / `Option<Store>` and the `?`
  is what yields `Store`. The inner type is already in the signature.
- Nothing is inferred. The extractor records `try_yield` on `-> Result<T, E>`
  / `-> Option<T>` and `try: true` on a trailing-`?` init. The linker takes
  that yield only when `?` is present. A Result factory without `?`, a `?` on
  a non-Result factory, a `?` in the middle (`create_repo()?.get()`), and a
  custom `MyResult<T>` name nothing.
- Bump both index-state schema families from v89 to v90 because function
  payloads gained `try_yield` and `call_bindings` gained `try`, so cached
  relations are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Try call | `create_repo()?.write()` resolves to `Store::write` |
| Try nested | `create_repo().get()?.write()` resolves to `Inner::write` |
| Try init | `let store = create_repo()?; store.write()` resolves |
| Fail-closed | `create_repo().write()` on a Result factory, `build()?.write()` on `-> Store`, `create_repo()?.get()` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2AZ — Nested Crate-Path Factory Hops (bounded Rust slice)

- A method call one hop past a `crate::` / `self::` / `super::` factory now
  resolves. Until now `crate::create_repo().get().write()` stayed unresolved:
  P2AV typed `crate::create_repo().write()` and P2AR typed `create_repo().get().write()`,
  but crate-qualified constructors often chain one extra stated method. The
  inner method's declared return is already in the snapshot.
- Nothing is inferred. The extractor records receiver `crate::create_repo().get()`
  and `call_bindings` callee `crate::create_repo().get`. The linker reads that
  factory's return, then that type's `get`. A third hop, a missing inner method,
  an untyped inner method, and a relative nested path (`app::create_repo().get()`)
  name nothing.
- Bump both index-state schema families from v90 to v91 because cached `Calls`
  relations gain these edges and `call_bindings` callees gained nested crate-path
  shapes, so they are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Nested crate ident | `crate::create_repo().get().write()` resolves to `Inner::write` |
| Nested crate associated | `crate::app::Store::new().get().write()` resolves |
| Nested crate init | `let store = crate::create_repo().get(); store.write()` resolves |
| Fail-closed | `crate::create_repo().other()`, `crate::create_repo().extra()`, `crate::create_repo().get().extra()`, `app::create_repo().get()` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2BA — Inner-Try Factory Hops (bounded Rust slice)

- A `?` on the inner factory hop now types that hop from the Ok/Some yield.
  Until now `create_repo()?.get().write()` stayed unresolved: P2AY typed a
  trailing `?` (`create_repo()?.write()`, `create_repo().get()?.write()`) and
  P2AR typed `create_repo().get().write()`, but a Result factory in the middle
  of a two-hop chain is the common constructor shape. The inner type is already
  in the signature.
- Nothing is inferred. The extractor keeps `?` on the inner hop
  (`create_repo()?.get`, receiver `create_repo()?.get()`). The linker peels that
  hop's `?` and passes `try_yield` to that hop only; a trailing `?` is still
  last-hop `allow_try`. A third hop, a Result factory without inner `?`, a `?`
  on a non-Result inner factory, and a relative nested path name nothing.
- Bump both index-state schema families from v91 to v92 because cached `Calls`
  relations gain these edges and `call_bindings` callees gained inner-`?`
  shapes, so they are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Inner-try ident | `create_repo()?.get().write()` resolves to `Inner::write` |
| Inner-try member | `svc.create()?.get().write()` / `self.create()?.get().write()` resolve |
| Inner-try associated | `Store::new()?.get().write()` resolves |
| Inner-try crate-path | `crate::create_repo()?.get().write()` resolves |
| Inner-try init | `let store = create_repo()?.get(); store.write()` resolves |
| Fail-closed | `create_repo().get()` on a Result factory, `build()?.get()` on `-> Store`, `create_repo()?.get().extra()` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2BB — Unwrap/Expect Factory Yields (bounded Rust slice)

- A trailing `.unwrap()` or `.expect(...)` now types the receiver from the
  Ok/Some type a factory declares. Until now `create_repo().unwrap().write()`
  stayed unresolved: P2AY typed a trailing `?`, but constructors often surface
  the value with `unwrap`/`expect` instead. The inner type is already in the
  signature.
- Nothing is inferred. The extractor peels trailing unwrap/expect from
  `call_bindings` (`try: true`) and keeps them on the recorded receiver. The
  linker peels a trailing unwrap/expect only when the prefix is a factory call
  (`create_repo().unwrap()`, not `store.unwrap()`). An inner unwrap hop
  (`create_repo().unwrap().get()`), `unwrap_or` / `unwrap_or_default`, unwrap
  on a non-Result factory, and a turbofish name nothing.
- Bump both index-state schema families from v92 to v93 because cached `Calls`
  relations gain these edges and `call_bindings` gained unwrap inits, so they
  are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Unwrap call | `create_repo().unwrap().write()` resolves to `Store::write` |
| Expect call | `create_repo().expect("failed").write()` resolves |
| Unwrap nested | `create_repo().get().unwrap().write()` resolves to `Inner::write` |
| Unwrap member / associated / crate-path | `svc.create().unwrap().write()`, `Store::new().unwrap().write()`, `crate::create_repo().unwrap().write()` resolve |
| Unwrap init | `let store = create_repo().unwrap(); store.write()` resolves |
| Fail-closed | `build().unwrap()`, `create_repo().unwrap_or_default()`, `create_repo().unwrap().get()`, `store.unwrap()` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2BC — Inner-Unwrap Factory Hops (bounded Rust slice)

- A `.unwrap()` / `.expect(...)` on the inner factory hop now types that hop
  from the Ok/Some yield. Until now `create_repo().unwrap().get().write()`
  stayed unresolved: P2BB typed a trailing unwrap, P2BA typed an inner `?`,
  but constructors often unwrap the factory and then call one stated method.
  The inner type is already in the signature.
- Nothing is inferred. The extractor peels inner unwrap/expect from
  `call_bindings` as `create_repo()?.get`. The linker folds
  `.unwrap().` / `.expect(...).` to `?.` on a factory hop so P2BA's per-hop
  try applies. A third hop, unwrap on a non-Result inner factory,
  `unwrap_or_default`, and `store.unwrap().get()` name nothing.
- Bump both index-state schema families from v93 to v94 because cached `Calls`
  relations gain these edges and `call_bindings` callees gained inner-unwrap
  shapes, so they are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Inner-unwrap ident | `create_repo().unwrap().get().write()` resolves to `Inner::write` |
| Inner-expect ident | `create_repo().expect("failed").get().write()` resolves |
| Inner-unwrap member / associated / crate-path | `svc.create().unwrap().get().write()`, `Store::new().unwrap().get().write()`, `crate::create_repo().unwrap().get().write()` resolve |
| Inner-unwrap init | `let store = create_repo().unwrap().get(); store.write()` resolves |
| Fail-closed | `build().unwrap().get()`, `create_repo().unwrap().get().extra()`, `store.unwrap().get()` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### P2BD — Inherent Self Factory Returns (bounded Rust slice)

- An inherent `-> Self` now types the factory from the impl type. Until now
  `impl Store { fn new() -> Self { Self } }` recorded no `return_type`: P2AS
  typed `Store::new().write()` from `-> Store`, but the usual constructor
  writes `Self`. The impl type is already in the source.
- Nothing is inferred. The extractor substitutes `Self` / `&Self` / `Box<Self>`
  / `Result<Self, E>` with the inherent impl's type. A free-function `-> Self`,
  a trait method `-> Self`, and a trait-impl `-> Self` name nothing: those
  `Self`s are not the construction type an inherent factory states.
- Bump both index-state schema families from v94 to v95 because methods that
  previously had no `return_type` now have one, so cached `Calls` relations
  gain these edges and are recomputed in both configurations.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Inherent Self | `impl Store { fn new() -> Self }` records `Store` |
| Member Self | `fn wrap(&self) -> Self` records `Store` |
| Peeled Self | `-> Box<Self>`, `-> &Self`, `-> Result<Self, E>` record `Store` |
| Fail-closed | `fn free() -> Self`, trait `fn fresh() -> Self`, `impl Reader for Store { fn fresh() -> Self }` name nothing |

Focused verification:

```bash
cargo test -p athanor-extractor-rust --locked
cargo test -p athanor-linker-rust --locked
cargo test -p athanor-app --locked
```

#### Remaining P2 Gaps

- Type inference for a receiver whose type the source never states — a factory whose body names
  neither a construction, a callee, nor an assertion; package-manager-specific resolution that
  needs an installed tree, such as Yarn `nodeLinker` layouts and patched dependencies. The element,
  awaited, returned, structural, index-access, array-literal, returned-callee, peeled-return,
  assertion, call-result-receiver, member-factory, inline-receiver-assertion, qualified-
  construction, super-through-extends, type-alias-unfolding, constrained-type-parameter,
  rust-constrained-type-parameter, rust-type-alias-unfolding, rust-struct-field-receiver,
  js-constrained-return-type, js-invoked-factory-result, rust-factory-return-type,
  rust-constrained-factory-return, rust-member-factory-return,
  rust-nested-factory-hop, rust-associated-factory,
  rust-factory-initialized-binding, rust-member-factory-initialized-binding,
  rust-crate-path-factory, rust-nested-associated-factory,
  rust-nested-factory-initialized-binding, rust-try-factory-yield,
  rust-nested-crate-path-factory, rust-inner-try-factory-hop,
  rust-unwrap-factory-yield, rust-inner-unwrap-factory-hop,
  and rust-self-factory-return
  slices closed the parts of this
  gap that a construction, an annotation, a heritage clause, a type alias, a type-parameter bound,
  a named struct field, one forwarded factory call, one extra call of a function-typed factory,
  a Rust `-> Type` the file already writes, a method factory the receiver already names,
  one extra stated factory hop, an associated-function factory, a local
  initialized by such a factory, a `crate::` / `self::` / `super::` factory path,
  one extra hop past an associated factory, a local initialized by that extra hop,
  a trailing `?` on a `Result`/`Option` factory, one extra hop past a crate-path factory,
  a `?` on the inner hop of that extra factory, a trailing `.unwrap()` / `.expect(...)`
  on a `Result`/`Option` factory, an unwrap/expect on the inner hop of that extra factory,
  or an inherent `-> Self`
  can answer without inference.

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

#### P5c — Bounded Declaration Excerpts (bounded agent-surface slice)

- The Goal asked for relevant source in the same call that returns the call path and blast radius.
  P5b left verbatim bodies out of scope, so an agent still had to open every cited file itself.
  `explore` now inlines the cited declaration range of each entry and each path target.
- Nothing is a file dump. The slice is the `SourceLocation` the snapshot already recorded, clipped
  to 32 lines and 4,096 UTF-8 bytes, with `truncated` set when either bound clips the range. Blast
  radius stays a path list; one-hop counterparts are not inlined, because their neighbourhood is
  already the link list.
- Fail closed. An absolute path, a `..` component, a missing file, a symlink that leaves the
  working tree, and non-UTF-8 contents produce no excerpt and increment `omitted.source_unread`.
  The schema id stays `athanor.explore.v1`; the fields are additive.
- MCP `explore` and `ath explore` print the same report. Text output shows the entry excerpt under
  `source:`; JSON carries `excerpt` on entries and paths.

Focused source regression matrix:

| Surface | Regression |
| --- | --- |
| Excerpt | A cited range is inlined; an over-long range is clipped and marked truncated |
| Fail closed | `../secret.rs`, a missing file, and a working-tree-less fixture disclose `source_unread` |
| Contract | `athanor.explore.v1` still validates; omitted notes disclose unread excerpts |

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
stable-key/path anchor, and the `ath explore` command over the same report. Slice P5c inlines the
cited declaration range of each entry and path target, clipped to 32 lines and 4,096 bytes, so the
single-call answer includes source without becoming a file dump; daemon-served freshness stays open
on axis C5. That P5b
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
