---
id: doc://docs/adapters/linker-js-ts.md
kind: adapter
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# JavaScript/TypeScript Linkers

Crate: `athanor-linker-js-ts`

Port: `Linker`

The crate owns three linkers over the same normalized extractor payloads: `JsTsImportLinker`
(`builtin.linker.js_ts_imports`) for module imports, `JsTsCallLinker` (`builtin.linker.js_ts_calls`)
for the static call graph, and `JsTsImplementsLinker` (`builtin.linker.js_ts_implements`) for class
and interface heritage. All three read canonical entities from the current snapshot and never touch
the filesystem.

## Import Linker

The linker converts normalized static import and source-backed export payloads from
`athanor-extractor-js-ts` into canonical `imports` relations between existing JavaScript and
TypeScript module entities. A relative `export { name as alias } from './module'` is also a module
dependency, so it emits the same relation kind with reason `relative_module_re_export`.

### Resolution

Only relative `./` and `../` specifiers are resolved. The linker supports exact module paths,
extensionless source paths, and directory `index` modules across the built-in JS/TS extensions.
Resolution is deterministic and uses canonical entities already present in the current snapshot; it
does not read the filesystem.

### Emitted Knowledge

Each exact match emits a verified `RelationKind::Imports` relation from the source module to the
target module. Evidence includes the import or export source range and target module source; the
payload reason distinguishes `relative_module_import` from `relative_module_re_export`. Ownership is
the union of both module source files so incremental removal prunes stale relations.

## Call Linker

`JsTsCallLinker` resolves the `calls` arrays recorded per declaration into `Calls` relations, so
impact, blast-radius, and orientation queries can traverse JavaScript and TypeScript call paths
instead of stopping at imports.

### Resolution Order

Resolution is ordered from the strongest evidence to the weakest, and each step only fires when the
previous one is silent:

1. `same_module_this_method` — `this.method()` inside `Class.method` resolves to the sibling member.
2. `same_module_exact_name` — an exact qualified-name match inside the calling module.
3. `imported_exact_name` / `imported_symbol_name` — the callee head matches an import binding
   (default, namespace, or named) of the calling module, and the remainder matches an exported
   declaration in the resolved relative target module. Named import aliases retain both the local and
   imported spelling.
4. `imported_reexport_chain` — a named import resolves through explicit local export aliases and/or
   relative `export { name as alias } from './module'` clauses, following at most 32 export hops.
5. `same_module_member_name` — a member call with an unknown receiver resolves only when the calling
   module contains exactly one member with that name.
6. `unique_exported_name` — a bare call with no local or imported target resolves only when exactly
   one exported declaration across the snapshot carries that name.

An unresolved explicit import does not fall through to a weaker snapshot-wide guess. Steps 5 and 6
require a unique candidate, so ambiguity stays unresolved rather than becoming a guessed fact.
Confidence reflects the step (0.9 exact same-module down to 0.5 unique-name), and every relation is
`RelationStatus::Inferred`, because JavaScript dispatch is dynamic even when the call shape is static.

### Emitted Knowledge

Each resolved call emits a `RelationKind::Calls` relation from the calling declaration to the target
declaration. The payload records the matched `callee`, `callee_kind` (`call` or `new`), `resolution`
reason, and the call-site `line` and `column`; evidence covers the call-site file and line plus the
target declaration range, and ownership is the union of both source files.

Calls from a test location (`.test.`/`.spec.` file names, or `tests`/`test`/`__tests__`/`e2e`
directories) into non-test declarations additionally emit `RelationKind::TestedBy` from the tested
declaration to the test declaration, mirroring the Rust linker.

Relations are sorted by id and capped at 10,000 per snapshot, so output is bounded and reproducible
for one snapshot.

## Heritage Linker

`JsTsImplementsLinker` turns class `extends`/`implements` and interface `extends` names into
canonical heritage relations. `extends` emits `RelationKind::Extends`; class `implements` emits
`RelationKind::Implements`. This keeps subclass/interface inheritance distinct from a class's
contract implementation.

### Resolution

A class `extends` name resolves to exactly one class, a class `implements` name to exactly one
interface, and an interface `extends` name to exactly one interface. A candidate declared in the same
module wins; otherwise the name must be unique across the snapshot, and the source never resolves to
itself. Ambiguous names stay unresolved rather than being guessed, mirroring the Rust linker's
trait-implementation bridges. Package, workspace-alias, and TypeScript path-alias targets remain
outside this name-only resolution.

### Emitted Knowledge

Each bridge is inferred with evidence on both source ranges at 0.7 confidence: name-based resolution
cannot prove that a rebuilt program links the same symbol. The payload records the `bridge`
(`extends` or `implements`), the `matched_value`, and a source-kind-specific reason. The same
`Extends` kind is used for class and interface inheritance; `Implements` is reserved for the class
contract relationship.

## Boundaries

Package imports, Node built-ins, workspace aliases, TypeScript path aliases, dynamic imports,
CommonJS `require` calls, `export *`, `export * as namespace`, default-export name resolution,
computed member calls, and receiver typing are not resolved in this slice. Explicit named export
clauses and relative named re-export chains are followed up to 32 hops; cycles, ambiguous mappings,
unresolved sources, and deeper chains remain unresolved. All linkers emit relations only when either
endpoint is in the affected subset, which keeps incremental updates proportional to the change.

## Verification

```bash
cargo test -p athanor-linker-js-ts
```
