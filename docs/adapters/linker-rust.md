---
id: doc://docs/adapters/linker-rust.md
kind: adapter
language: en
last_verified_snapshot: snap_jsonl_00000090
source_language: en
status: verified
---
# Rust Linker Adapter

The Rust relation linker maps module containment, static calls, imports, and test coverage dependencies within Rust codebases.

Implements: `Linker`

## Emitted Relations

- **`RelationKind::Contains`**: Maps nested structures, submodules, and methods to their containing module or type parent.
- **`RelationKind::Imports`**: Resolves `use` declarations (taking into account parent/self modules) to target symbols inside the workspace.
- **`RelationKind::Calls`**: Resolves local/imported path calls or referenced variables to target functions/methods.
- **`RelationKind::TestedBy`**: Links tested functions/methods to the `TestCase`s that invoke them during static analysis.

All relations merge evidence and ownership from both sides.

## Inputs

`LinkInput` from `athanor-extractor-rust` containing `Module`, `Function`, `TestCase`, and `Symbol` entities.

## Configuration

Add `builtin.linker.rust` to the adapter plugin manifests or registry. It is enabled in Athanor by default.

## Side Effects

None.

## Test

```bash
cargo test -p athanor-linker-rust
```
