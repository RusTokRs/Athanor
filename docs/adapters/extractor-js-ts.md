---
id: doc://docs/adapters/extractor-js-ts.md
kind: adapter
language: en
source_language: en
status: draft
---

# JavaScript/TypeScript Extractor

Crate: `athanor-extractor-js-ts`

Port: `Extractor`

## Purpose

The JavaScript/TypeScript extractor indexes mixed JavaScript and TypeScript projects through one built-in language adapter. It normalizes tree-sitter JavaScript, TypeScript, and TSX parser output into Athanor canonical objects without exposing parser-specific AST structures outside the adapter.

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

Module payloads include normalized `imports` and `exports` arrays. Import relation materialization is deferred to a later linker slice.

## Diagnostics

Recoverable parser errors produce `js_ts_parse_error` diagnostics. Unsupported declaration shapes detected during extraction produce `js_ts_unsupported_syntax` diagnostics. Diagnostics include source evidence, ownership, and the originating parser/language metadata.

## Boundaries

The adapter deliberately avoids framework-specific semantics such as React components, Next.js pages, NestJS controllers, Express routes, Vue components, and project convention inference. Those belong in later adapter slices.

## Verification

```bash
cargo test -p athanor-extractor-js-ts
```
