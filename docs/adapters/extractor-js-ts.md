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

It handles `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`, and `package.json`.

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

Module payloads include normalized `imports` and `exports` arrays. Each import entry records the
specifier, its source range, and the local bindings it introduces: `default_name`, `namespace_name`,
and the sorted local names of `named` specifiers (`import { fetchUser as fetch }` records `fetch`).
The built-in JS/TS import linker uses those payloads to materialize exact relative module imports
without exposing parser AST nodes.

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

## Class Heritage

Class declarations additionally record their heritage as bare type names. `extends` holds the base
class name and `implements` the interface names, in declaration order and deduplicated:
`class Service extends api.Base implements Store<K>, Closeable {}` records `extends: "Base"` and
`implements: ["Store", "Closeable"]`. Generic arguments and qualified paths are reduced to the final
segment, because resolution to a canonical symbol is the linker's job; classes without heritage carry
neither field, so payloads stay unchanged for the common case. Plain JavaScript has no `implements`
clause, so only `extends` appears there.

## Diagnostics

Recoverable parser errors produce `js_ts_parse_error` diagnostics coalesced at the outer parser error node, so one parser failure does not produce duplicate nested findings. Parser input strips a leading UTF-8 BOM and accepts Node-style shebangs before tree-sitter parsing. Unsupported top-level declaration shapes detected during extraction produce `js_ts_unsupported_syntax` diagnostics. Ordinary top-level runtime statements in scripts and ambient TypeScript `declare module` blocks are not treated as unsupported declaration diagnostics. Diagnostics include source evidence, ownership, and the originating parser/language metadata.

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
[Express Route Extractor](extractor-express.md). Package and alias import resolution remain outside
the base language slice.

## Verification

```bash
cargo test -p athanor-extractor-js-ts
cargo test -p athanor-extractor-js-ts --features precision-parser
```
