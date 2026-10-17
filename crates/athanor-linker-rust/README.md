# athanor-linker-rust

Rust codebase relation linker adapter.

Implements: `Linker`

## What It Links

- **Module Containment (`Contains`)**: Connects parent module/symbol entities to child entities (e.g. `crate` contains `crate::auth`).
- **Imports (`Imports`)**: Resolves module use paths to target symbols in the codebase.
- **Call Graph (`Calls`)**: Resolves a bare path call through imports at `0.8` (`rust_static_call`), and resolves a method call through the receiver's declared type: `self.m()` inside its own impl at `0.8` (`rust_self_method`), a concrete receiver to that type's method at `0.8` (`rust_typed_receiver_method`), and a `&dyn Trait` or `impl Trait` receiver to the trait's own method declaration at `0.7` (`rust_dyn_dispatch_method`). A nested, unannotated, or ambiguously named receiver stays unresolved.
- **Tested By (`TestedBy`)**: Links functions/methods to the `TestCase`s that invoke them.
- **Trait inheritance (`Extends`)**: Links a trait to its resolvable supertraits; external, ambiguous, or renamed-import aliases remain unresolved.
- **Trait implementation (`Implements`)**: Links a type to the trait declared by each `impl Trait for Type` fact.

All emitted relations combine evidence and ownership from both sides.

## Inputs

`LinkInput` containing entities parsed from `athanor-extractor-rust`.

## Side Effects

None.

## Test

```bash
cargo test -p athanor-linker-rust
```
