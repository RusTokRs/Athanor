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

Module payloads include normalized `imports` and `exports` arrays. The built-in JS/TS import linker
uses those payloads to materialize exact relative module imports without exposing parser AST nodes.

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

The adapter deliberately avoids framework-specific semantics such as React components, Next.js pages, NestJS controllers, Express routes, Vue components, and project convention inference. Package and alias import resolution also remain outside the base language slice.

## Verification

```bash
cargo test -p athanor-extractor-js-ts
cargo test -p athanor-extractor-js-ts --features precision-parser
```
