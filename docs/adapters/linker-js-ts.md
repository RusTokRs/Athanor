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

Relative `./` and `../` specifiers resolve against the importing module: exact module paths,
extensionless source paths, and directory `index` modules across the built-in JS/TS extensions. A
relative specifier that misses is retried at the same path inside the other roots of the nearest
configuration's `rootDirs`, because TypeScript presents those roots as one merged directory; two
roots that both hold the path are ambiguous and stay unresolved.

Non-relative specifiers resolve only through configuration evidence that the extractor recorded:

1. **Path aliases.** The nearest `tsconfig.json`/`jsconfig.json` at or above the importing module
   supplies the `paths` mappings, resolved against the base directory recorded for that file. A
   configuration that declares no mappings inherits the first ancestor along its relative `extends`
   chain, up to 8 hops, because TypeScript does not merge mappings across `extends`. At most one `*`
   per pattern is substituted into each target, and the first target naming an existing module wins.
2. **Workspace packages.** A bare specifier resolves when a `package.json` declaring that `name` is a
   member of a workspace declared by another `package.json` in the snapshot or by the `packages:`
   list of a `pnpm-workspace.yaml`/`.yml` layout. Membership needs a glob covering the package
   directory, where `*` matches one path segment and `**` matches any number of them. When the
   package declares `exports`, the subpath resolves through that map: the longest declared prefix
   wins, rows are tried in the package's own declaration order, a row guarded by an unevaluable
   condition such as `production` is skipped, and a subpath the map does not declare is unreachable
   even when the file exists. Without `exports`, the entry point is the recorded `main` when it
   resolves, otherwise `index` in the package root, and `name/subpath` specifiers resolve inside the
   package directory.

A duplicated package name, a package that no workspace declaration covers, a configuration without a
usable base directory, a package-valued `extends` specifier, an `exports` target that leaves the
package directory, and a specifier matching no mapping all stay unresolved. `node_modules` contents,
package-manager installs, and any resolution that needs an installed tree are never read.

Resolution is deterministic and uses canonical entities already present in the current snapshot; it
does not read the filesystem.

### Emitted Knowledge

Each exact match emits a verified `RelationKind::Imports` relation from the source module to the
target module. Evidence includes the import or export source range and target module source; the
payload reason names both the resolution origin and the syntax that carried it:
`relative_module_import`, `relative_module_re_export`, `path_alias_module_import`,
`path_alias_module_re_export`, `workspace_package_module_import`, or
`workspace_package_module_re_export`. Ownership is the union of both module source files so
incremental removal prunes stale relations.

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
   requested member to a declaration in the resolved target module, where the binding's specifier is
   resolved relatively, through a path alias, or through a workspace package with the same rules as
   the import linker. Named import aliases
   retain both the local and imported spelling; directly qualified imported-class methods keep their
   existing import-member resolution behavior.
4. `typed_receiver_member` / `typed_receiver_path` — a receiver whose declared type the calling
   module records is walked as a typed path: `this` roots at the enclosing method owner, `super`
   roots at the class that owner `extends` so `super.save()` dispatches at construction confidence
   0.8 and a `super` call without a declared base does not fall through to a unique member, a type
   alias unfolds one hop so `type Client = Repo` types `repo.save()` as `Repo.save` at the binding's
   own confidence, a type parameter that `extends` one named type is recorded as that bound so
   `T extends Repo` types `repo.save()` as `Repo.save` and `function create<T extends Repo>(): T`
   types `create().save()` as `Repo.save`, every
   further segment resolves the type the previous segment declared, and the terminal member must
   match `<Type>.<member>` exactly once. A segment is read from the calling module's
   `typed_bindings` first and then from the rows of the module that owns the current type, so
   `this.repo.items.map()` resolves when `App.repo` is a `Repo` declared in `repo.ts` and that
   module records `Repo.items` as an `ItemStore`. The root may instead be a binding the module
   initializes with a call: `const repo = createRepo(); repo.items` takes the return type
   `createRepo` declares, and `const repo = create()()` takes the function-type yield when the
   row marks `invoked`. Only a factory the steps below prove — a local, imported, or
   re-exported declaration, never a name-unique one — types that receiver. Ownership is proven
   first: only an exported
   top-level class, interface, or type alias owns field types, and a name declared in two modules
   owns neither. The typed step is disabled while a factory is resolved, so one receiver's type is
   never derived from another's. A terminal the type does not declare itself may still be a member it
   inherits: the `extends` chain the source states is then walked, up to four hops, and a member of a
   base class resolves at reason `inherited_typed_member` (`inherited_typed_receiver_path` for a path)
   with confidence 0.6. Both the type the walk leaves and the base it enters must be class or
   interface declarations the snapshot states — a base name two files declare, and a base no file
   declares, end the walk. The root itself may be typed by the collection it came out of: the
   extractor records a `for...of` iteration variable and an array combinator's callback parameter
   with the element type of the annotated collection, so `for (const item of items)` dispatches
   `item.save()` at confidence 0.65, and an index access `items[0].save()` walks the same type
   through the synthetic name `items[]`. An array literal that constructs one type records that
   name as a construction, so `const items = [new Item()]; items[0].save()` dispatches at 0.8. The root may also be the factory or construction the callee path itself names: `createRepo().save()`
   walks `createRepo()` with the same type evidence as `const repo = createRepo(); repo.save()`,
   `create()().save()` walks `create()()` from the factory's `returned_function_type`,
   `(await createRepo()).save()` walks `createRepo()@` at the awaited tier, `new Repo().save()`
   and `new ns.Repo().save()` walk `Repo{}` as a construction (the constructed type is the
   terminal identifier), and `(value as Repo).save()` walks `Repo#` as an annotation.
   A later segment may be a call of a method on the current type:
   `svc.build().save()` and `createRepo().getStore().save()` continue from that method's stated
   return type. An untyped factory result does not fall through to a unique member name. The root may also be a binding the module awaits: `const repo =
   await createRepo()` takes the type the factory's declared promise yields, so `repo.save()`
   dispatches at the same 0.65, while an unawaited `const pending = createRepo()` holds the promise
   and names nothing. The root may also be a binding a factory produced: the factory's declared
   return type types it at 0.7, and a factory that declares none is typed by the one type every
   `return` of its body constructs at 0.8, because a body cannot lie about what it builds while an
   annotation can — and an annotation still wins when the declaration has one. An extra call of the
   result takes the function-type yield at 0.7: `create()().save()` does not use `return_type`,
   because `create()` itself is a function. A wrapper that
   constructs nothing and only `return makeRepo()` is typed by that factory's annotation or
   constructed return one hop. An `async` wrapper is typed only when the binding awaits it:
   `const repo = await wrap()` inherits the inner factory one hop, and `const pending = wrap()`
   holds a promise and names nothing. A factory or initializer that asserts `as Repo` types the
   receiver at 0.7, the same tier as a declared annotation. A second hop, a mix of construction
   and call, two callees, a member callee, and an inner factory that yields a promise on an
   unawaited binding name nothing. A segment may also be
   a member of an object literal: the literal states the type of each member it constructs, so
   `config.logger.flush()` walks through `config$object` and dispatches at 0.8. A type repeated inside
   the walk, a path longer than four
   segments, an ambiguous binding, and a terminal that matches more than one declaration end the
   walk.
5. `imported_default_export` — a default import resolves directly to the function or class
   binding marked by `export default` in its relative target module: a named declaration, a
   statically named local identifier, or an anonymous default recorded under the canonical
   `default` name.
6. `imported_reexport_chain` — calls resolve through explicit local aliases, relative named
   re-exports (including default aliases), source-backed wildcard clauses, one direct
   `export * as name` mapping used by a named-import single-member call, or a namespace
   alias forwarded across modules by source-backed named re-exports of the namespace
   name itself. Multiple wildcard sources
   resolve only when applicable branches agree on one declaration; absent names are skipped, while
   ambiguous, unresolved, cyclic, or over-limit branches block the aggregate. The namespace hop
   counts against the 32-export-hop bound, and recursive export resolution is capped at 1,024 module
   visits per call; multi-segment member paths through named-import namespace aliases are not resolved.
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

An unresolved explicit import does not fall through to a weaker snapshot-wide guess. Steps 6 and 7
require a unique candidate, so ambiguity stays unresolved rather than becoming a guessed fact.
Confidence reflects the step (0.9 exact same-module down to 0.5 unique-name): a receiver proven by
a construction resolves at 0.8 — a `new Type()` initializer at the binding, or the one type every
`return` of a factory's body constructs — one proven by an annotation at 0.7, one typed by a type the
source writes one derivation away — the element of a collection or the awaited value of a declared
promise — at 0.65, and a member the receiver's type inherits at 0.6 — the weaker tiers are one step
further from the call site than the evidence that seeds them, and still above the name guesses they
replace. Every relation is
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

Path aliases, workspace packages, `exports` subpaths, `rootDirs` roots, and pnpm workspace layouts
resolve only through the recorded configuration evidence described above; resolution that needs an
installed tree — package-manager installs, `node_modules` contents, and Yarn linker layouts — stays
outside the model. An `exports` subpath the map does not declare is unreachable even when the file
exists, a row guarded by an unevaluable condition is skipped, and two `rootDirs` roots holding the
same path are ambiguous. Node built-ins, bare specifiers that no configuration
proves, dynamic imports,
CommonJS `require` calls, default expressions that declare nothing callable or
constructible, computed member calls of an element (`handlers[key]()`), nested index access, and
multi-segment named-import namespace paths are not resolved. Receiver typing covers only a type the
source states: an inferred type, a type imported by another module but never named in an annotation,
and a member reached through a factory whose body names neither a construction nor one callee stay unresolved. An index access resolves only
when the collection's annotation or an array literal of constructions names one element type, and
does not fall through to a unique member name. A member inherited through a
declared `extends` chain does resolve, but only to a base the snapshot states unambiguously and only
within four hops. A receiver path may walk through an object literal, but only into a member the
literal constructs; a property whose value is a literal, a call, or a shorthand names nothing. A
receiver a factory produced is typed by the return type the factory declares or,
when it declares none, by the one type every `return` of its body constructs, or one hop through
the factory a wrapper returns a call to — two returns that disagree, a mix of construction and
call, two callees, a member callee, an unawaited `async` wrapper, a second hop, an inner factory
that yields a promise on an unawaited binding, and a `return` of a nested function name nothing. A receiver the
module awaits is typed by the promise the factory declares, but only when the source awaits it: an
unawaited promise holds no receiver type. A field is read from
the module that
declares its owner only when that owner is exported and unambiguous, and a cyclic or over-bound
path ends the walk. Callback bridging only fires for the extractor's combinator allowlist and
only for statically nameable arguments; callbacks passed as inline arrows, call results, or
computed expressions are never recorded, and a callback argument whose name does not resolve
uniquely stays unresolved like any other callee. A named import representing a namespace object can use one unique, direct,
source-backed `export * as name` mapping for a single member call; missing or ambiguous namespace
mappings stay unresolved. Named default functions, named default classes, statically named local default bindings, and
anonymous defaults recorded under the canonical `default` name resolve through relative imports, path
aliases, workspace packages, and explicit named re-export clauses; plain `export *` still does not
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
