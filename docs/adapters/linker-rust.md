---
id: doc://docs/adapters/linker-rust.md
kind: adapter
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Rust Linker Adapter

The Rust relation linker maps module containment, static calls, imports, test coverage, trait
implementation, and trait-supertype dependencies within Rust codebases.

Implements: `Linker`

## Emitted Relations

- **`RelationKind::Contains`**: Maps nested structures, submodules, and methods to their containing module or type parent.
- **`RelationKind::Imports`**: Resolves `use` declarations (taking into account parent/self modules) to target symbols inside the workspace.
- **`RelationKind::Calls`**: Resolves a bare path call through local paths and recorded imports at `0.8` (`rust_static_call`), and resolves a method call through the receiver's declared type: `self.m()` dispatches inside its own impl at `0.8` (`rust_self_method`), a receiver declared as a concrete type resolves to that type's method at `0.8` (`rust_typed_receiver_method`), and a `&dyn Trait` or `impl Trait` receiver resolves to the trait's own method declaration at `0.7` (`rust_dyn_dispatch_method`). A method never links to itself, and a receiver the source does not type — nested such as `self.reader`, unannotated, or named by a type declared twice — stays unresolved.
- **`RelationKind::TestedBy`**: Links tested functions/methods to the `TestCase`s that invoke them during static analysis.
- **`RelationKind::Implements`**: Resolves `trait_impl` facts from the Rust extractor into bridges from the implementing type to the trait. Both endpoints must resolve to exactly one `struct`, `enum`, `union`, `type_alias`, or `trait` symbol by name; ambiguous names stay unresolved and marker impls are included.
- **`RelationKind::Extends`**: Resolves a trait declaration's extracted `supertraits` paths through local paths and unambiguous recorded imports to in-snapshot trait declarations, emitting an edge from the child trait to each parent trait. External, ambiguous, and renamed-import aliases remain unresolved.

All relations merge evidence and ownership from both sides.

## Inputs

`LinkInput` from `athanor-extractor-rust` containing `Module`, `Function`, `TestCase`, and `Symbol`
entities. Calls are read from the declaration payload's `calls` objects and typed through the
declaration's own `typed_bindings`; the type index is built from the `struct`, `enum`, `union`,
`type_alias`, and `trait` symbols the snapshot declares.

## Configuration

Add `builtin.linker.rust` to the adapter plugin manifests or registry. It is enabled in Athanor by default.

## Side Effects

None.

## Test

```bash
cargo test -p athanor-linker-rust
```
