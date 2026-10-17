---
id: doc://docs/adapters/extractor-js-ts.md
kind: adapter
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# JavaScript/TypeScript Extractor

Crate: `athanor-extractor-js-ts`

Port: `Extractor`

## Purpose

The JavaScript/TypeScript extractor indexes mixed JavaScript and TypeScript projects through one built-in language adapter. It normalizes tree-sitter JavaScript, TypeScript, and TSX parser output into Athanor canonical objects without exposing parser-specific AST structures outside the adapter. An optional precision build runs Oxc as a second verification backend.

The same crate also exposes separate `NextJsExtractor` and `ExpressExtractor` owners for bounded
framework route projections. They are documented separately and do not change the framework-neutral
semantics described on this page.

## Inputs

The adapter supports source files whose language hint is one of:

```text
javascript
javascriptreact
typescript
typescriptreact
json
```

It handles `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`, `package.json`, the
`tsconfig.json`/`jsconfig.json` configuration files plus their named `.json` variants such as
`tsconfig.base.json`, and the `pnpm-workspace.yaml`/`pnpm-workspace.yml` workspace manifest.

## Emitted Knowledge

Source files emit:

- `module` entities for files
- `function` entities for declarations, methods, arrow functions, and variable-bound functions
- `class` entities for classes
- `symbol` entities for TypeScript interfaces and type aliases
- `symbol_defined` facts with source evidence and ownership

`package.json` emits:

- `package` entities for package names
- `dependency` entities for dependency declarations
- `symbol_defined` facts with dependency kind, version requirement, source evidence, and ownership

The package entity payload also carries the bounded `main` entry point and the `workspaces` glob
patterns, in both the array form and the `{ "packages": [...] }` object form, with an
`omitted_workspaces` count. Those two fields are the evidence a linker needs to treat a bare
workspace-package specifier as a repository-internal import.

The payload also carries `exports`: one ordered row per declared target with its `subpath`, the
`conditions` guarding it, and its `target`, bounded to 64 rows and 4 condition keys each with an
`omitted_exports` count. String, array, and condition-object sugar forms are normalized to the `.`
subpath, wildcard subpaths and targets keep their `*`, and a pattern with more than one `*` is
skipped. Row order is the declaration order Node falls back through, so the linker proves rows in
the package's own preference order.

`pnpm-workspace.yaml` and `pnpm-workspace.yml` emit:

- one `feature` entity per manifest, stable key `config://<path>#js-ts-workspace-layout`
- `symbol_defined` facts with source evidence and ownership

The layout payload records `config_kind: "js_ts_workspace_layout"`, the `packages` glob patterns of
the flat block sequence under the top-level `packages:` key, and an `omitted_packages` count.
Flow sequences, anchors, and nested blocks are outside the projection, and a manifest without a
readable package list is not projected at all, so an unreadable layout never makes every package in
the snapshot a workspace member.

`tsconfig.json`, `jsconfig.json`, and their named `.json` variants emit:

- one `feature` entity per configuration file, stable key `config://<path>#js-ts-module-resolution`
- `symbol_defined` facts with source evidence and ownership

The configuration payload records only module-resolution evidence: `base_dir` (the `baseUrl` value
resolved against the directory of the declaring file, or that directory when `baseUrl` is absent),
each `paths` pattern with its substitution targets and declaration line, an inherited `extends`
specifier, and `omitted_paths`/`omitted_targets` counts. Bounds are 64 path mappings and 8 targets
per mapping. A pattern with more than one `*`, a non-string target, and an empty target list are
skipped and counted. An absolute, drive-qualified, or repository-escaping `baseUrl` is recorded
without a base directory, so its mappings stay unresolvable in the linker. A configuration file
without `compilerOptions` and without `extends` carries no resolution evidence and is not projected.
Configuration files are read as JSONC: comments outside string literals and trailing commas are
removed before parsing, preserving line structure, while `//` and `/* */` inside string values are
kept verbatim.

The same entity records `root_dirs`: the `compilerOptions.rootDirs` entries TypeScript presents as
one merged virtual directory, each resolved against the declaring file's directory and bounded to
eight roots with an `omitted_root_dirs` count; an absolute, drive-qualified, or repository-escaping
root is reported as omitted instead of being merged.

Module payloads include normalized `imports` and `exports` arrays. Each import entry records the
specifier, its source range, and the local bindings it introduces: `default_name`, `namespace_name`,
the sorted local names in `named`, and `named_bindings` pairs (`imported`, `local`) so
`import { fetchUser as fetch }` preserves both names. Each export entry records its optional source,
source range, a `star` marker, optional `namespace_name`, `default_export`/`default_name` metadata,
and explicit `bindings` pairs (`imported`, `exported`) for named export clauses. A source-backed
clause such as `export { login as signIn } from './auth'` preserves the original/exported spelling; a
source-less clause can represent a local alias. Plain `export * from './auth'` sets `star: true` and
leaves `namespace_name` null; `export * as ns from './auth'` records `namespace_name: "ns"` and is
not marked as a plain star. Named default functions, named default classes, and statically named
local identifier defaults carry their local binding. An anonymous default function, default class,
or default arrow function has no source name, so it is bound to the canonical name `default` — the
one name a JavaScript or TypeScript binding cannot have — and is recorded as a real declaration with
stable key `symbol://js-ts:<path>#default` and payload `anonymous_default: true`, including its call
sites and, for a class, its heritage and members. Default expressions that declare nothing callable
or constructible (an object literal, a call result, or a primitive) remain unbound, and at most one
anonymous default is recorded per file. The built-in JS/TS import
linker uses relative source-backed imports and re-exports to materialize module dependencies without
exposing parser AST nodes, while the call linker follows explicit named and default-function aliases,
bounded wildcard chains that resolve uniquely across sources, and direct namespace exports for
single-member calls. Absent names are skipped; ambiguous, unresolved, cyclic, and over-limit branches
remain unresolved.

Module payloads also carry bounded `typed_bindings`: one row per receiver whose declared type the
module states, with the binding `name`, the `type`, the `kind` (`construction` for a `new Type()`
initializer, `annotation` for a TypeScript annotation), and the declaration `line`. Rows come from
variable declarators, class fields (recorded as `Class.field`, so two classes never collide), and
TypeScript parameter properties, where `constructor(private repo: Repo)` also yields `Class.repo`.
Only a type the source names is recorded: a call result, a union, an array type, a destructuring
pattern, an aliased initializer, a function type, and a generic instantiation that is not a bare
identifier are skipped, and a qualified or generic name is reduced to its terminal identifier, so
`Promise<Repo>` and `ns.Repo` both record `Repo`. Rows are bounded to 64 per module with an
`omitted_typed_bindings` count.

## Call Sites

Function, method, and variable-bound function payloads include a bounded `calls` array of the
statically named call sites in the declaration subtree. Each entry records `callee` (a dotted path
such as `login`, `api.send`, or `this.refresh`), the terminal `name`, the optional `receiver`, the
`kind` (`call` or `new`), the callee `line` and `column`, and the argument `arg_count`.

Only shapes whose target can be named without running the program are recorded: identifiers, member
chains rooted in an identifier or `this`/`super`, and constructor invocations. Computed calls
(`handlers[key]()`), calls through call results or parenthesized expressions (`factory()()`), and JSX
elements are omitted rather than guessed. Call sites are attributed to the declaration that contains
them, nested class bodies are skipped because their members are separate declarations, and the array
is capped at 256 entries per declaration so payloads stay bounded. Call sites are raw syntax
evidence: resolving them to declarations is the job of the JS/TS call linker.

A call site whose terminal callee name is a known callback combinator (array iteration methods,
promise handlers `then`/`catch`/`finally`, timers, and event/observable registration) additionally
records a bounded `callback_arguments` array of its statically nameable arguments — plain
identifiers and identifier-rooted member chains such as `this.onChange`. Inline arrows, call
results, literals, and computed expressions are skipped, and at most four arguments are recorded.
The allowlist keeps the evidence bounded to positions that really invoke a function; the call
linker turns each recorded argument into an indirect `Calls` edge when it resolves to a unique
declaration.

## Type Heritage

Class declarations record heritage names in declaration order: `extends` is a one-element array for
the base class and `implements` is an array of interface names. For example,
`class Service extends api.Base implements Store<K>, Closeable {}` records
`extends: ["Base"]` and `implements: ["Store", "Closeable"]`. TypeScript interfaces also record
all parent interfaces in an `extends` array; JavaScript has no `implements` clause. Generic arguments
and qualified paths are reduced to the final type-name segment because resolution to canonical
symbols belongs to the linker. Declarations without heritage omit both fields.

## Diagnostics

A `tsconfig.json`/`jsconfig.json` file that cannot be parsed after JSONC normalization produces a
`js_ts_typescript_config_parse_error` diagnostic. Recoverable parser errors produce `js_ts_parse_error` diagnostics coalesced at the outer parser error node, so one parser failure does not produce duplicate nested findings. Parser input strips a leading UTF-8 BOM and accepts Node-style shebangs before tree-sitter parsing. Unsupported top-level declaration shapes detected during extraction produce `js_ts_unsupported_syntax` diagnostics. Ordinary top-level runtime statements in scripts and ambient TypeScript `declare module` blocks are not treated as unsupported declaration diagnostics. Diagnostics include source evidence, ownership, and the originating parser/language metadata.

## Precision Verification

Enable the feature-gated dual-parser mode when building the CLI or daemon:

```bash
cargo run -p ath --features js-ts-precision -- index .
cargo build -p athd --features js-ts-precision
```

Precision mode parses each affected JS/TS file through tree-sitter and Oxc, normalizes named
declarations, static imports, source-backed re-exports, source ranges, and parser recovery state,
then compares those rows deterministically. Tree-sitter remains the canonical-output backend, so
agreed findings retain the same stable keys and ids as normal mode. Contradictions are not merged.

Disagreements emit evidence-backed `js_ts_parser_backend_only_finding`,
`js_ts_parser_source_range_mismatch`, or `js_ts_parser_recovery_difference` diagnostics. Module
payloads include bounded `athanor.js_ts_precision.v1` counts with a per-file limit of 64 reported
differences and explicit omitted counts. `ath coverage` continues to use its stable schema; these
diagnostics appear in its diagnostic-kind and file-level counts, while detailed bounded metrics
remain on the canonical module entity.

Precision builds use a distinct index-state schema, forcing a safe one-time rebuild when the mode
is enabled or disabled. The tradeoff is a second parse for each affected JS/TS file. Oxc is an
in-process Rust dependency; the mode does not run commands or use the network.

## Boundaries

`JsTsExtractor` deliberately avoids framework-specific semantics such as React components, Next.js
routes/pages, NestJS controllers, Express routes, Vue components, and project convention inference.
Call sites are recorded as syntax, not as resolved targets: overload resolution, receiver types,
prototype mutation, `require`-based CommonJS calls, mixin patterns, and reflective dispatch remain outside the base
language slice and never become facts. Parse order is deterministic, so a declaration's call array is
stable across runs of the same source.
The separate built-in `NextJsExtractor` owns only the bounded Next.js filesystem-route projection;
see [Next.js Route Extractor](extractor-nextjs.md). `ExpressExtractor` owns only static Express
application/router route declarations with explicit framework bindings; see
[Express Route Extractor](extractor-express.md). Package and alias import resolution itself remains
outside the base extractor: it records the `tsconfig.json`/`jsconfig.json` and `package.json` evidence,
and `athanor-linker-js-ts` decides what it proves.

## Verification

```bash
cargo test -p athanor-extractor-js-ts
cargo test -p athanor-extractor-js-ts --features precision-parser
```
