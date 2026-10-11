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

The crate owns two linkers over the same normalized extractor payloads: `JsTsImportLinker`
(`builtin.linker.js_ts_imports`) for module imports and `JsTsCallLinker`
(`builtin.linker.js_ts_calls`) for the static call graph. Both read canonical entities from the
current snapshot and never touch the filesystem.

## Import Linker

The linker converts normalized static import payloads from `athanor-extractor-js-ts` into canonical
`imports` relations between existing JavaScript and TypeScript module entities.

### Resolution

Only relative `./` and `../` specifiers are resolved. The linker supports exact module paths,
extensionless source paths, and directory `index` modules across the built-in JS/TS extensions.
Resolution is deterministic and uses canonical entities already present in the current snapshot; it
does not read the filesystem.

### Emitted Knowledge

Each exact match emits a verified `RelationKind::Imports` relation from the importing module to the
target module. Evidence includes the source import range and target module source. Ownership is the
union of both module source files so incremental removal prunes stale relations.

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
   (default, namespace, or named) of the calling module, and the remainder matches a declaration of
   the resolved relative target module.
4. `same_module_member_name` — a member call with an unknown receiver resolves only when the calling
   module contains exactly one member with that name.
5. `unique_exported_name` — a bare call with no local or imported target resolves only when exactly
   one exported declaration across the snapshot carries that name.

Steps 4 and 5 require a unique candidate, so ambiguity stays unresolved rather than becoming a
guessed fact. Confidence reflects the step (0.9 exact same-module down to 0.5 unique-name) and every
relation is `RelationStatus::Inferred`, because JavaScript dispatch is dynamic even when the call
shape is static.

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

## Boundaries

Package imports, Node built-ins, workspace aliases, TypeScript path aliases, dynamic imports,
CommonJS `require` calls, computed member calls, and receiver typing are not resolved in this slice.
Both linkers emit relations only when either endpoint is in the affected subset, which keeps
incremental updates proportional to the change.

## Verification

```bash
cargo test -p athanor-linker-js-ts
```
