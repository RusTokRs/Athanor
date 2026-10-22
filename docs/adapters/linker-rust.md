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
- **`RelationKind::Calls`**: Resolves a bare path call through local paths and recorded imports at `0.8` (`rust_static_call`), and resolves a method call through the receiver's declared type: `self.m()` dispatches inside its own impl at `0.8` (`rust_self_method`), `self.reader.write()` walks the named field the owner struct declares — one hop, peeling wrapping parentheses and a leading `&`/`mut` — and resolves to that field's type at `0.8` (`rust_typed_receiver_method`) or `0.7` (`rust_dyn_dispatch_method`) for a `dyn`/`impl` field, a receiver declared as a concrete type resolves to that type's method at `0.8` (`rust_typed_receiver_method`), `create_repo().write()` takes the type that factory declares it returns at `0.8`, or at `0.7` when the return is `impl Trait` / `T: Trait` / `dyn Trait`, `svc.create().write()` and `self.create().write()` take the type that method declares it returns at the same tiers, `create_repo().get().write()` and `svc.create().get().write()` take one extra hop through that inner method's return at the same tiers, `Store::new().write()` takes the type that associated function declares it returns at the same tiers, including inherent `-> Self`, `Store::new().get().write()` takes one extra hop through that inner method's return at the same tiers, `crate::create_repo().write()` and `crate::app::Store::new().write()` take a `crate::` / `self::` / `super::` path to that factory, `crate::create_repo().get().write()` takes one extra hop through that inner method's return, `let store = create_repo(); store.write()`, `let store = Store::new(); store.write()`, `let store = svc.create(); store.write()`, `let store = create_repo().get(); store.write()`, `let store = crate::create_repo().get(); store.write()`, `let store = create_repo()?; store.write()`, `let store = create_repo()?.get(); store.write()`, `let store = create_repo().unwrap(); store.write()`, and `let store = create_repo().unwrap().get(); store.write()` take that factory return through the binding, `create_repo()?.write()`, `create_repo().get()?.write()`, `create_repo()?.get().write()`, `create_repo().unwrap().write()`, `create_repo().expect("failed").write()`, and `create_repo().unwrap().get().write()` take the Ok/Some type a `Result`/`Option` factory declares, a type alias unfolds one hop so `type Client = Store` types `item.write()` as `Store::write`, and a `&dyn Trait`, `impl Trait`, or `T: Trait` receiver resolves to the trait's own method declaration at `0.7` (`rust_dyn_dispatch_method`). A method never links to itself, and a receiver the source does not type — two hops such as `self.reader.inner` or `svc.create().inner`, a field the struct does not declare, an untyped factory, a third factory hop, a missing or untyped member factory, a `Result`/`Option` factory without `?`/`unwrap`/`expect`, a `?` or unwrap on a non-Result factory, `unwrap_or_default`, `store.unwrap()`, unannotated, or named by a type declared twice — stays unresolved. A `dyn`/`impl` trait call additionally records one candidate edge per declared implementation of the method at `0.7`'s weaker tier `0.5` (`rust_dyn_dispatch_candidate`): the trait proves the interface and the impl proves the body exists, not that this call reaches it. Candidates come from the same `trait_impl` facts that produce `Implements` bridges, are ordered by qualified name, are bounded to eight per call site with the recorded count in the payload, and never become `TestedBy` coverage.
- **`RelationKind::TestedBy`**: Links tested functions/methods to the `TestCase`s that invoke them during static analysis.
- **`RelationKind::Implements`**: Resolves `trait_impl` facts from the Rust extractor into bridges from the implementing type to the trait. Both endpoints must resolve to exactly one `struct`, `enum`, `union`, `type_alias`, or `trait` symbol by name; ambiguous names stay unresolved and marker impls are included.
- **`RelationKind::Extends`**: Resolves a trait declaration's extracted `supertraits` paths through local paths and unambiguous recorded imports to in-snapshot trait declarations, emitting an edge from the child trait to each parent trait. External, ambiguous, and renamed-import aliases remain unresolved.

All relations merge evidence and ownership from both sides.

## Inputs

`LinkInput` from `athanor-extractor-rust` containing `Module`, `Function`, `TestCase`, and `Symbol`
entities. Calls are read from the declaration payload's `calls` objects and typed through the
declaration's own `typed_bindings`, through a named field on the owner struct for `self.field`,
or through a factory's declared `return_type` for `create_repo().write()`, `svc.create().write()`, `self.create().write()`, one extra hop such as `create_repo().get().write()`, an associated function such as `Store::new().write()`, one extra hop such as `Store::new().get().write()`, a `crate::` path such as `crate::create_repo().write()`, one extra hop such as `crate::create_repo().get().write()`, an inner-try hop such as `create_repo()?.get().write()`, a trailing unwrap such as `create_repo().unwrap().write()`, an inner unwrap hop such as `create_repo().unwrap().get().write()`, and a `call_bindings` local such as `let store = create_repo(); store.write()`, `let store = svc.create(); store.write()`, `let store = create_repo().get(); store.write()`, `let store = create_repo()?; store.write()`, `let store = create_repo()?.get(); store.write()`, `let store = create_repo().unwrap(); store.write()`, or `let store = create_repo().unwrap().get(); store.write()`;
the type index is built from the `struct`, `enum`, `union`,
`type_alias`, and `trait` symbols the snapshot declares.

## Configuration

Add `builtin.linker.rust` to the adapter plugin manifests or registry. It is enabled in Athanor by default.

## Side Effects

None.

## Test

```bash
cargo test -p athanor-linker-rust
```
