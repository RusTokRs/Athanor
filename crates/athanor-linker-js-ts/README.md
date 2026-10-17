# athanor-linker-js-ts

JavaScript and TypeScript linkers: module imports, the static call graph, and type heritage.

Implements: `Linker`

Adapter ids: `builtin.linker.js_ts_imports`, `builtin.linker.js_ts_calls`, `builtin.linker.js_ts_implements`

## What It Emits

`JsTsImportLinker` emits:

- `RelationKind::Imports` from a JS/TS module to an existing local JS/TS module when a static
  relative import or source-backed re-export resolves exactly. Re-export edges use reason
  `relative_module_re_export`, and an edge resolved through configuration evidence uses
  `path_alias_module_import`/`path_alias_module_re_export` or
  `workspace_package_module_import`/`workspace_package_module_re_export`.

`JsTsCallLinker` emits:

- `RelationKind::Calls` from a calling declaration to a resolved declaration, when the call site is
  statically named and resolution has evidence.
- `RelationKind::Calls` for a callback bridge: a call site recorded by a known callback combinator
  (`map`, `then`, `setTimeout`, …) also resolves each statically named argument, so
  `list.map(filterActive)` links the caller to `filterActive` with reason
  `js_ts_callback_bridge` and the hosting combinator in `host_callee`.
- `RelationKind::TestedBy` from a tested declaration to a declaration in a test location that calls
  it, including callback bridges from test locations.

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

Non-relative specifiers resolve only through configuration evidence recorded by
`athanor-extractor-js-ts`:

1. **Path aliases.** The nearest `tsconfig.json`/`jsconfig.json` at or above the importing module's
   directory supplies the `paths` mappings, resolved against that file's recorded base directory.
   A configuration without mappings inherits the first ancestor along its relative `extends` chain,
   up to 8 hops, because TypeScript does not merge path mappings across `extends`. A pattern matches
   with at most one `*`; the captured text is substituted into each target, and the first target that
   names an existing module wins.
2. **Workspace packages.** A bare specifier resolves when a `package.json` whose `name` equals the
   specifier is a member of a workspace declared by another `package.json` in the snapshot: a
   `packages/*`-style glob, where `*` matches one path segment and `**` matches any number of them,
   must cover the package directory. The package entry point is its recorded `main` when it resolves,
   otherwise `index` in the package root. A `name/subpath` specifier resolves the subpath inside the
   package directory with the standard extension and `index` candidates.

Every other bare specifier stays unresolved: `node_modules` contents, `exports` and conditional
exports maps, TypeScript `rootDirs`, package-manager installs, and `pnpm-workspace.yaml` are never
read. A package that no workspace declaration covers, a duplicated package name, a configuration
without a usable base directory, and a specifier that matches no mapping all fail closed instead of
falling back to a name guess.

## Call Resolution

The call linker consumes the bounded `calls` arrays and the import bindings that
`athanor-extractor-js-ts` records per declaration, and resolves each call site in this order:

1. `same_module_this_method` — `this.method()` inside `Class.method`.
2. `same_module_exact_name` — exact qualified name inside the calling module.
3. `imported_exact_name` / `imported_symbol_name` — through a namespace or named import binding of
   the calling module. The binding's specifier resolves relatively, through a path alias, or through
   a workspace package, using the same rules as the import linker.
4. `imported_default_export` — a default import resolves directly to its named default
   function or class declaration.
5. `imported_reexport_chain` — through explicit named local exports, relative named/default
   re-exports, source-backed wildcard clauses, a unique direct `export * as name` mapping for
   a named-import single-member call, or a namespace alias forwarded across modules by
   source-backed named re-exports of the namespace name itself. Multiple wildcard sources
   resolve only when applicable branches agree on one declaration; absent names are skipped,
   while ambiguous, unresolved, cyclic, or over-limit branches block the aggregate. Namespace
   hops count toward the 32-hop bound, and each
   call's recursive export resolution is capped at 1,024 module visits.
6. `same_module_member_name` — a unique member name inside the calling module.
7. `unique_exported_name` — a unique exported declaration elsewhere in the snapshot.

Steps 5 and 6 accept a candidate only when it is unique, so ambiguity stays unresolved. Confidence
runs from `0.9` for an exact same-module match down to `0.5` for a unique name match, and the payload
records the callee, call kind, resolution reason, line, and column. Callback arguments run through
the same ordered steps with the same fail-closed rules; the hosting call's own resolution never
gates the bridge. Output is sorted by relation id
and capped at 10,000 relations per snapshot.

## Side Effects

None. All three linkers read canonical entities and write relations. They do not read files, run
commands, use the network, or modify project files.

## Limitations

- Heritage resolution is name-based; imported bindings, package/workspace aliases, and TypeScript
  path aliases are not used to resolve `extends` or `implements` names.
- Node built-in imports, bare specifiers that no configuration or workspace declaration proves,
  `exports` and conditional-exports subpaths, and package-manager resolution are not linked.
- A plain source-backed `export *` participates in named call resolution across multiple sources,
  but never forwards `default`. Missing names are ignored; distinct targets, ambiguous nested exports,
  unresolved sources, cycles, and chains deeper than 32 hops keep the aggregate unresolved. Named
  default functions, statically named local identifier defaults, and anonymous defaults bound to
  the canonical `default` name can resolve through relative imports, path aliases, workspace
  packages, and explicit named re-export chains. Named imports can resolve a single member through
  one unique, direct, source-backed `export * as name` mapping; multi-hop namespace aliases and
  multi-segment paths through named-import namespace aliases stay unresolved. Existing
  namespace-import receiver behavior is unchanged. A default expression that declares nothing
  callable or constructible stays unresolved because it has no statically nameable local binding.
- Dynamic `import()`, CommonJS `require()`, computed calls (`handlers[key]()`), calls through call
  results, reflective dispatch, and receiver typing are not resolved.
- Test detection is location-based because no test framework runs: `.test.`/`.spec.` file names and
  `tests`/`test`/`__tests__`/`e2e` directories.

## Test

```bash
cargo test -p athanor-linker-js-ts
```
