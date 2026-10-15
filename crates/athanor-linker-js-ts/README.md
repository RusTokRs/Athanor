# athanor-linker-js-ts

JavaScript and TypeScript linkers: module imports, the static call graph, and type heritage.

Implements: `Linker`

Adapter ids: `builtin.linker.js_ts_imports`, `builtin.linker.js_ts_calls`, `builtin.linker.js_ts_implements`

## What It Emits

`JsTsImportLinker` emits:

- `RelationKind::Imports` from a JS/TS module to an existing local JS/TS module when a static
  relative import or source-backed re-export resolves exactly. Re-export edges use reason
  `relative_module_re_export`.

`JsTsCallLinker` emits:

- `RelationKind::Calls` from a calling declaration to a resolved declaration, when the call site is
  statically named and resolution has evidence.
- `RelationKind::TestedBy` from a tested declaration to a declaration in a test location that calls
  it.

`JsTsImplementsLinker` emits:

- `RelationKind::Extends` from a class to its resolved base class and from an interface to each
  resolved parent interface.
- `RelationKind::Implements` from a class to each resolved interface named by its `implements`
  clause.

Heritage names prefer a unique same-file declaration, then a unique snapshot-wide match; ambiguous
or external names remain unresolved instead of being guessed. The extractor records `extends` and
`implements` separately, and the linker preserves that distinction in the canonical relation kind.

Import relations are verified; call relations are inferred, because JavaScript dispatch is dynamic
even when the call shape is static. Heritage relations are inferred at 0.7 confidence because
name-only resolution cannot prove a program-level binding. Every relation is evidence-backed, owned
by both source files, and carries the matched value plus the resolution reason.

## Import Resolution

The import linker resolves `./` and `../` imports against canonical JS/TS module paths. It supports
exact paths, extensionless source paths, and directory `index` modules for `.js`, `.jsx`, `.mjs`,
`.cjs`, `.ts`, `.tsx`, `.mts`, and `.cts`.

## Call Resolution

The call linker consumes the bounded `calls` arrays and the import bindings that
`athanor-extractor-js-ts` records per declaration, and resolves each call site in this order:

1. `same_module_this_method` — `this.method()` inside `Class.method`.
2. `same_module_exact_name` — exact qualified name inside the calling module.
3. `imported_exact_name` / `imported_symbol_name` — through a namespace or named import binding of
   the calling module.
4. `imported_default_export` — a default import resolves directly to its named default function.
5. `imported_reexport_chain` — through explicit named local exports, relative named/default
   re-exports, one source-backed wildcard export per module, or a unique direct `export * as name`
   mapping for a named-import single-member call. Namespace hops count toward the 32-hop bound.
6. `same_module_member_name` — a unique member name inside the calling module.
7. `unique_exported_name` — a unique exported declaration elsewhere in the snapshot.

Steps 5 and 6 accept a candidate only when it is unique, so ambiguity stays unresolved. Confidence
runs from `0.9` for an exact same-module match down to `0.5` for a unique name match, and the payload
records the callee, call kind, resolution reason, line, and column. Output is sorted by relation id
and capped at 10,000 relations per snapshot.

## Side Effects

None. All three linkers read canonical entities and write relations. They do not read files, run
commands, use the network, or modify project files.

## Limitations

- Heritage resolution is name-based; imported bindings, package/workspace aliases, and TypeScript
  path aliases are not used to resolve `extends` or `implements` names.
- Package, workspace alias, TypeScript path alias, and Node built-in imports are not linked.
- A plain source-backed `export *` participates in named call resolution when each module in the
  chain has exactly one wildcard source, but never forwards `default`. Named default functions and
  statically named local identifier defaults can resolve through relative imports and explicit named
  re-export chains. Named imports can resolve a single member through one unique, direct,
  source-backed `export * as name` mapping; multi-hop namespace aliases and multi-segment paths
  through named-import namespace aliases stay unresolved. Existing namespace-import receiver behavior
  is unchanged. Multiple wildcard sources, anonymous/default expressions, and default class call
  targets also stay unresolved; cycles, ambiguity, unresolved sources, and chains deeper than 32 hops
  remain unresolved.
- Dynamic `import()`, CommonJS `require()`, computed calls (`handlers[key]()`), calls through call
  results, reflective dispatch, and receiver typing are not resolved.
- Test detection is location-based because no test framework runs: `.test.`/`.spec.` file names and
  `tests`/`test`/`__tests__`/`e2e` directories.

## Test

```bash
cargo test -p athanor-linker-js-ts
```
