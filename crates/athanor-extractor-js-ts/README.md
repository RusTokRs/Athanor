# athanor-extractor-js-ts

JavaScript and TypeScript source-code extractor adapter.

Implements: `Extractor`

The crate exposes two extractor types. `JsTsExtractor` owns framework-neutral JavaScript/TypeScript
and `package.json` parsing. `NextJsExtractor` is a separate bounded framework projection for standard
Next.js filesystem route conventions and is registered independently as `builtin.extractor.nextjs`.

## What It Emits

`JsTsExtractor` emits:

- `EntityKind::Module` for each supported source file
- `EntityKind::Function` for function, method, arrow-function, and variable-bound function declarations
- `EntityKind::Class` for JavaScript and TypeScript classes
- `EntityKind::Symbol` for TypeScript interfaces and type aliases
- `EntityKind::Package` and `EntityKind::Dependency` for `package.json` package and dependency declarations
- `FactKind::SymbolDefined` for emitted source declarations and package dependency declarations
- `DiagnosticKind::Other("js_ts_parse_error")` when tree-sitter reports parser errors, coalesced at the outer parser error node to avoid duplicate nested findings
- `DiagnosticKind::Other("js_ts_unsupported_syntax")` for unsupported top-level declaration shapes detected during extraction
- feature-gated parser verification diagnostics for backend-only findings, source-range mismatches,
  and recovery differences between tree-sitter and Oxc

`NextJsExtractor` emits one source-backed `EntityKind::Other("nextjs_route")` and one
`FactKind::RouteDeclared` for recognized App Router `page`/`route` files and Pages Router route files.
The adapter-scoped entity kind prevents generic operations/API/onboarding profiles from treating a web
route as one of their own anchors. Dynamic segments and framework/router/route-kind metadata are
preserved without inferring HTTP methods or React component semantics.

All emitted objects include ownership metadata for the source file. Facts and diagnostics include source evidence.

## Inputs

`JsTsExtractor` accepts `SourceFile` with:

```text
language_hint = javascript | javascriptreact | typescript | typescriptreact | json
content = UTF-8 source text
```

It supports `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`, and project `package.json` files.

`NextJsExtractor` recognizes standard `.js`, `.jsx`, `.ts`, and `.tsx` route files under `app/`,
`src/app/`, `pages/`, and `src/pages/`. Route inventory is path-derived and does not require source
content.

## Stable Keys

```text
module://js-ts:src/auth.ts
symbol://js-ts:src/auth.ts#login
symbol://js-ts:src/App.tsx#App
package://npm:example-app
dependency://npm:@scope/package
nextjs-route://src/app/products/[id]/page.tsx#route
```

Source declarations are scoped to the project-relative file path. Package dependency stable keys are scoped by npm package name. Next.js route entities are source-backed so conflicting route files remain distinct.

## Parser Backend

The base adapter uses tree-sitter JavaScript, TypeScript, and TSX grammars. Parser AST nodes are normalized into Athanor canonical entities and payload fields; parser-specific node types do not cross the adapter boundary.

The parser input strips a leading UTF-8 BOM and accepts Node-style shebangs before handing source text to tree-sitter. Ambient TypeScript `declare module` blocks are accepted as known declaration syntax and are not reported as unsupported top-level declarations.

Build `ath` or `athd` with `--features js-ts-precision` to run Oxc as a second parser for every
affected JS/TS source file. The adapter compares normalized declaration, import, re-export, source
range, and recovery rows rather than raw ASTs. Tree-sitter remains the canonical-output backend;
Oxc disagreements produce evidence-backed diagnostics instead of silently merging results.

Precision reports are stored in the module payload as bounded `athanor.js_ts_precision.v1`
metrics. At most 64 disagreement diagnostics are emitted per file, with explicit reported and
omitted counts. Switching between normal and precision builds changes the persisted index-state
schema so unchanged JS/TS files are safely rebuilt once.

## Side Effects

None. The extractors do not run commands, use the network, or modify project files.

## Limitations

- Import/export data is stored in module payloads and definition facts. The separate
  `athanor-linker-js-ts` adapter materializes exact relative module imports.
- JSX and TSX component semantics are not inferred. Components are emitted as normal functions or classes.
- Framework-specific semantics remain outside `JsTsExtractor`; the separate `NextJsExtractor` currently
  covers only bounded filesystem route conventions.
- Next.js HTTP methods, route-handler schemas/auth, layouts, metadata, middleware, rewrites, redirects,
  server actions, custom `pageExtensions`, and interception-route semantics are not inferred.
- Parser errors are reported as coalesced diagnostics, and extraction continues with recoverable declarations.
- Top-level runtime statements in scripts and ambient module declarations are not treated as unsupported declaration diagnostics.
- Dynamic CommonJS exports and computed declaration names are not fully resolved.
- Precision mode increases JS/TS parsing cost because affected files are parsed twice. It compares
  named functions, classes, interfaces, type aliases, static imports, and source-backed re-exports;
  method and variable-bound function comparison remains intentionally excluded until both backends
  expose an equally stable adapter-local representation.

## Test

```bash
cargo test -p athanor-extractor-js-ts
cargo test -p athanor-extractor-js-ts --features precision-parser
```
