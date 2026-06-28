# athanor-extractor-js-ts

JavaScript and TypeScript source-code extractor adapter.

Implements: `Extractor`

## What It Emits

- `EntityKind::Module` for each supported source file
- `EntityKind::Function` for function, method, arrow-function, and variable-bound function declarations
- `EntityKind::Class` for JavaScript and TypeScript classes
- `EntityKind::Symbol` for TypeScript interfaces and type aliases
- `EntityKind::Package` and `EntityKind::Dependency` for `package.json` package and dependency declarations
- `FactKind::SymbolDefined` for emitted source declarations and package dependency declarations
- `DiagnosticKind::Other("js_ts_parse_error")` when tree-sitter reports parser errors, coalesced at the outer parser error node to avoid duplicate nested findings
- `DiagnosticKind::Other("js_ts_unsupported_syntax")` for unsupported top-level declaration shapes detected during extraction

All emitted objects include ownership metadata for the source file. Facts and diagnostics include source evidence.

## Inputs

`SourceFile` with:

```text
language_hint = javascript | javascriptreact | typescript | typescriptreact | json
content = UTF-8 source text
```

The adapter supports `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`, and project `package.json` files.

## Stable Keys

```text
module://js-ts:src/auth.ts
symbol://js-ts:src/auth.ts#login
symbol://js-ts:src/App.tsx#App
package://npm:example-app
dependency://npm:@scope/package
```

Source declarations are scoped to the project-relative file path. Package dependency stable keys are scoped by npm package name.

## Parser Backend

The adapter uses tree-sitter JavaScript, TypeScript, and TSX grammars. Parser AST nodes are normalized into Athanor canonical entities and payload fields; parser-specific node types do not cross the adapter boundary.

The parser input strips a leading UTF-8 BOM and accepts Node-style shebangs before handing source text to tree-sitter. Ambient TypeScript `declare module` blocks are accepted as known declaration syntax and are not reported as unsupported top-level declarations.

## Side Effects

None. The adapter does not run commands, use the network, or modify project files.

## Limitations

- Import/export data is stored in module payloads and definition facts. The separate
  `athanor-linker-js-ts` adapter materializes exact relative module imports.
- JSX and TSX component semantics are not inferred. Components are emitted as normal functions or classes.
- Framework-specific routes, controllers, pages, schemas, and conventions are intentionally out of scope.
- Parser errors are reported as coalesced diagnostics, and extraction continues with recoverable declarations.
- Top-level runtime statements in scripts and ambient module declarations are not treated as unsupported declaration diagnostics.
- Dynamic CommonJS exports and computed declaration names are not fully resolved.

## Test

```bash
cargo test -p athanor-extractor-js-ts
```
