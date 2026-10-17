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
3. `imported_exact_name` / `imported_symbol_name` — a named or namespace import resolves its
   requested member to a declaration in the resolved relative target module. Named import aliases
   retain both the local and imported spelling; directly qualified imported-class methods keep their
   existing import-member resolution behavior.
4. `imported_default_export` — a default import resolves directly to the named function
   or class binding marked by `export default` in its relative target module.
5. `imported_reexport_chain` — calls resolve through explicit local aliases, relative named
   re-exports (including default aliases), source-backed wildcard clauses, one direct
   `export * as name` mapping used by a named-import single-member call, or a namespace
   alias forwarded across modules by source-backed named re-exports of the namespace
   name itself. Multiple wildcard sources
   resolve only when applicable branches agree on one declaration; absent names are skipped, while
   ambiguous, unresolved, cyclic, or over-limit branches block the aggregate. The namespace hop
   counts against the 32-export-hop bound, and recursive export resolution is capped at 1,024 module
   visits per call; multi-segment member paths through named-import namespace aliases are not resolved.
6. `receiver_dispatch` — a single-segment member call on a local recorded as
   `new ClassName()` resolves to the bound class's member in the class's own
   module. The constructor name resolves through the ordered steps above, so
   same-module and imported classes both work, and the member must be a unique
   qualified `ClassName.member` in that module.
7. `same_module_member_name` — a member call with an unknown receiver resolves only when the calling
   module contains exactly one member with that name.
8. `unique_exported_name` — a bare call with no local or imported target resolves only when exactly
   one exported declaration across the snapshot carries that name.

### Callback Bridges

Call sites recorded with `callback_arguments` (see the extractor's callback-combinator allowlist)
are resolved a second time: each statically named argument is treated as a callee and run through
the same ordered rules above, because `list.map(filterActive)` invokes `filterActive` indirectly.
A resolved argument emits its own `Calls` relation with reason `js_ts_callback_bridge`, the hosting
combinator in `host_callee`, and `callee_kind: "callback"`; the relation id uses a dedicated
material prefix, so a declaration that is both called directly and passed as a callback keeps both
edges. Bridges inherit the fail-closed rules: an unresolvable or ambiguous argument stays
unresolved, and the hosting call's own resolution (or its failure) never gates the bridge. Promise
handlers (`then`/`catch`/`finally`), timers, and event/observable registrations bridge the same
way. Callback bridges from a test location additionally emit `TestedBy`, mirroring direct calls.

An unresolved explicit import does not fall through to a weaker snapshot-wide guess. Steps 5 and 6
require a unique candidate, so ambiguity stays unresolved rather than becoming a guessed fact.
Confidence reflects the step (0.9 exact same-module down to 0.5 unique-name), and every relation is
`RelationStatus::Inferred`, because JavaScript dispatch is dynamic even when the call shape is static.

### Emitted Knowledge

Each resolved call emits a `RelationKind::Calls` relation from the calling declaration to the target
declaration. The payload records the matched `callee`, `callee_kind` (`call`, `new`, or `callback`),
`resolution` reason, and the call-site `line` and `column`; callback bridges additionally record the
hosting `host_callee`. Evidence covers the call-site file and line plus the target declaration
range, and ownership is the union of both source files.

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
CommonJS `require` calls, anonymous/default-expression exports,
anonymous default classes, computed member calls, multi-segment named-import namespace paths,
and receiver typing beyond recorded constructor bindings are not resolved. Receiver dispatch
fires only for a local bound to a statically named `new ClassName()` call; computed
constructors, destructuring bindings, deeper member paths, and inherited members stay
unresolved. Callback bridging only fires for the extractor's combinator allowlist and
only for statically nameable arguments; callbacks passed as inline arrows, call results, or
computed expressions are never recorded, and a callback argument whose name does not resolve
uniquely stays unresolved like any other callee. A named import representing a namespace object can use one unique, direct,
source-backed `export * as name` mapping for a single member call; missing or ambiguous namespace
mappings stay unresolved. Named default functions, named default classes, and statically named local default bindings
resolve through relative imports and explicit named re-export clauses; plain `export *` still does not
expose `default`. Named, default, wildcard, and namespace hops share the 32-hop bound. Multiple
wildcard sources combine only when applicable branches agree on one declaration; absent names are
ignored, while cycles, ambiguous mappings, unresolved sources, and over-limit branches stay
unresolved. Each call's recursive export resolution is capped at 1,024 module visits.
All linkers emit relations only when either endpoint is in the affected subset, which keeps incremental
updates proportional to the change.

## Verification

```bash
cargo test -p athanor-linker-js-ts
```
