# athanor-extractor-js-ts

JavaScript and TypeScript source-code extractor adapter.

Implements: `Extractor`

The crate exposes three extractor types. `JsTsExtractor` owns framework-neutral JavaScript/TypeScript,
`package.json`, and `tsconfig.json`/`jsconfig.json` parsing. `NextJsExtractor` is a separate bounded framework projection for standard
Next.js filesystem route conventions and is registered independently as `builtin.extractor.nextjs`.
`ExpressExtractor` is a separate bounded framework projection for static Express application/router
route declarations and is registered independently as `builtin.extractor.express`.

## What It Emits

`JsTsExtractor` emits:

- `EntityKind::Module` for each supported source file
- `EntityKind::Function` for function, method, arrow-function, and variable-bound function declarations
- `EntityKind::Class` for JavaScript and TypeScript classes
- `EntityKind::Symbol` for TypeScript interfaces and type aliases
- Class `extends`/`implements` and TypeScript interface `extends` names in normalized declaration
  payloads for the heritage linker
- Bounded `calls` arrays of statically named call sites per declaration, plus a bounded
  `callback_arguments` array on call sites whose callee is a known callback combinator
  (iteration methods, promise handlers, timers, event/observable registration) for the call
  linker's callback bridges
- `EntityKind::Package` and `EntityKind::Dependency` for `package.json` package and dependency declarations,
  with bounded `main` and `workspaces` evidence on the package entity
- `EntityKind::Feature` for the module-resolution options of `tsconfig.json`/`jsconfig.json`: the
  `baseUrl`-resolved base directory, each `paths` pattern with its substitution targets, and an
  inherited `extends` specifier
- `FactKind::SymbolDefined` for emitted source declarations and package dependency declarations
- `DiagnosticKind::Other("js_ts_parse_error")` when tree-sitter reports parser errors, coalesced at the outer parser error node to avoid duplicate nested findings
- `DiagnosticKind::Other("js_ts_typescript_config_parse_error")` when a `tsconfig.json`/`jsconfig.json`
  file cannot be read as JSONC after comments and trailing commas are removed
- `DiagnosticKind::Other("js_ts_unsupported_syntax")` for unsupported top-level declaration shapes detected during extraction
- feature-gated parser verification diagnostics for backend-only findings, source-range mismatches,
  and recovery differences between tree-sitter and Oxc

`NextJsExtractor` emits one source-backed `EntityKind::Other("nextjs_route")` and one
`FactKind::RouteDeclared` for recognized App Router `page`/`route` files and Pages Router route files.
The adapter-scoped entity kind prevents generic operations/API/onboarding profiles from treating a web
route as one of their own anchors. Dynamic segments and framework/router/route-kind metadata are
preserved without inferring HTTP methods or React component semantics.

`ExpressExtractor` emits source-backed `EntityKind::Other("express_route")` entities plus
`FactKind::RouteDeclared` for static literal `app|router.<method>("/path", handler)` calls after the
adapter has resolved the receiver from an explicit `express` import/require. It records receiver,
HTTP method, literal path, handler path, source path, evidence, and ownership without promoting the
finding to the shared `ApiEndpoint` taxonomy.

All emitted objects include ownership metadata for the source file. Facts and diagnostics include source evidence.

## Inputs

`JsTsExtractor` accepts `SourceFile` with:

```text
language_hint = javascript | javascriptreact | typescript | typescriptreact | json
content = UTF-8 source text
```

It supports `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`, project `package.json`
files, and the `tsconfig.json`/`jsconfig.json` configuration files plus their named `.json` variants
such as `tsconfig.base.json`.

`NextJsExtractor` recognizes standard `.js`, `.jsx`, `.ts`, and `.tsx` route files under `app/`,
`src/app/`, `pages/`, and `src/pages/`. Route inventory is path-derived and does not require source
content.

`ExpressExtractor` accepts the same JavaScript/TypeScript source-language hints as `JsTsExtractor`,
requires source content, and only runs its bounded route projection when the file references `express`.
Tree-sitter parsing must succeed before framework findings are emitted.

## Stable Keys

```text
module://js-ts:src/auth.ts
symbol://js-ts:src/auth.ts#login
symbol://js-ts:src/App.tsx#App
symbol://js-ts:src/anon.ts#default
package://npm:example-app
dependency://npm:@scope/package
config://tsconfig.json#js-ts-module-resolution
nextjs-route://src/app/products/[id]/page.tsx#route
express-route://src/http/server.ts#app:GET:/health:health
```

Source declarations are scoped to the project-relative file path. An anonymous `export default`
function, class, or arrow function is a real declaration with no source name, so it is recorded as
`default`: the one name a JavaScript or TypeScript binding cannot have, which keeps the synthesized
key collision-free. Package dependency stable keys are scoped by npm package name. Module-resolution configuration keys are scoped by the configuration file path, so a monorepo keeps one entity per `tsconfig.json`. Next.js and Express route entities are source-backed so conflicting route declarations remain distinct.

## Parser Backend

The base adapter uses tree-sitter JavaScript, TypeScript, and TSX grammars. Parser AST nodes are normalized into Athanor canonical entities and payload fields; parser-specific node types do not cross the adapter boundary. `ExpressExtractor` reuses the same in-process parser selection and normalized source input rather than adding another parser dependency.

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

- Import/export data is stored in module payloads and definition facts. Payloads preserve named
  import/export aliases, distinguish plain source-backed `export *` from `export * as name`, and mark
  named default-function, default-class, or local-identifier bindings; `athanor-linker-js-ts`
  follows bounded named, default-function and default-class, and multiple-source wildcard chains
  when they identify one target, plus direct namespace mappings for single-member calls.
- An anonymous `export default` function, class, or arrow function is recorded as a declaration
  named `default` with `anonymous_default: true`, so a default import or construction resolves to
  it. Default expressions that declare nothing callable or constructible — an object literal, a
  call result, or a primitive — stay unbound, and at most one anonymous default is recorded per
  file. Precision verification excludes synthesized `default` rows because the second backend
  names only declarations that carry a source identifier.
- Ambiguous or unresolved wildcard branches remain unresolved. Package, workspace, and
  path-alias specifiers are now resolution evidence for `athanor-linker-js-ts`; the extractor only
  records the configuration that makes them resolvable.
- Only the module-resolution options of a TypeScript/JavaScript configuration file are projected:
  `compilerOptions.baseUrl`, `compilerOptions.paths`, and `extends`. Other compiler options,
  `references`, `include`/`exclude`, and solution-style configurations are not extracted. A
  configuration file without `compilerOptions` and without `extends` is not projected at all.
- Path mappings are bounded to 64 entries and 8 substitution targets per file, with disclosed
  omission counts. A pattern with more than one `*`, a non-string target, and an empty target list
  are skipped rather than guessed. An absolute, drive-qualified, or repository-escaping `baseUrl`
  is reported without a base directory, which keeps its mappings unresolvable.
- Configuration files are read as JSONC: line and block comments outside string literals and
  trailing commas are removed before parsing, preserving line structure so evidence lines stay
  correct. Nested comment markers, `//` and `/* */` inside string values, are preserved verbatim.
- Callback evidence is recorded only for the combinator allowlist and only for statically
  nameable arguments (identifiers and identifier-rooted member chains); inline arrows, call
  results, literals, and computed arguments are never recorded, and at most four arguments are
  kept per call site.
- JSX and TSX component semantics are not inferred. Components are emitted as normal functions or classes.
- Framework-specific semantics remain outside `JsTsExtractor`; separate `NextJsExtractor` and
  `ExpressExtractor` owners cover only their bounded route projections.
- Next.js HTTP methods, route-handler schemas/auth, layouts, metadata, middleware, rewrites, redirects,
  server actions, custom `pageExtensions`, and interception-route semantics are not inferred.
- Express dynamic route expressions, mounted/nested prefixes, `app.use`, `app.all`, `router.route(...)`,
  chained route builders, extra middleware arguments, inline closures, computed members, schemas, auth,
  middleware semantics, and handler implementation linking are not inferred.
- Parser errors are reported by the base extractor as coalesced diagnostics; the Express projection
  fails closed for a source file whose tree-sitter parse contains errors.
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
