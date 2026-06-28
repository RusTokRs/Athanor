# athanor-linker-js-ts

JavaScript and TypeScript import linker adapter.

Implements: `Linker`

## What It Emits

- `RelationKind::Imports` from a JS/TS module to an existing local JS/TS module when a static
  relative import resolves exactly.

Relations are verified, evidence-backed by the import source range and target module, and owned by
both source files.

## Resolution

The linker resolves `./` and `../` imports against canonical JS/TS module paths. It supports exact
paths, extensionless source paths, and directory `index` modules for `.js`, `.jsx`, `.mjs`, `.cjs`,
`.ts`, `.tsx`, `.mts`, and `.cts`.

## Side Effects

None. The adapter does not read files, run commands, use the network, or modify project files.

## Limitations

- Package, workspace alias, TypeScript path alias, and Node built-in imports are not linked.
- Dynamic `import()`, CommonJS `require()`, and symbol-level imported bindings are not linked.
- Export and re-export relations are deferred.

## Test

```bash
cargo test -p athanor-linker-js-ts
```
