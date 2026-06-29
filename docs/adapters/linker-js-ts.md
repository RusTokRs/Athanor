---
id: doc://docs/adapters/linker-js-ts.md
kind: adapter
language: en
last_verified_snapshot: snap_jsonl_00000251
source_language: en
status: verified
---
# JavaScript/TypeScript Import Linker

Crate: `athanor-linker-js-ts`

Port: `Linker`

## Purpose

The linker converts normalized static import payloads from `athanor-extractor-js-ts` into canonical
`imports` relations between existing JavaScript and TypeScript module entities.

## Resolution

Only relative `./` and `../` specifiers are resolved. The linker supports exact module paths,
extensionless source paths, and directory `index` modules across the built-in JS/TS extensions.
Resolution is deterministic and uses canonical entities already present in the current snapshot; it
does not read the filesystem.

## Emitted Knowledge

Each exact match emits a verified `RelationKind::Imports` relation from the importing module to the
target module. Evidence includes the source import range and target module source. Ownership is the
union of both module source files so incremental removal prunes stale relations.

## Boundaries

Package imports, Node built-ins, workspace aliases, TypeScript path aliases, dynamic imports,
CommonJS calls, symbol-level bindings, exports, and re-exports are not resolved in this slice.

## Verification

```bash
cargo test -p athanor-linker-js-ts
```
