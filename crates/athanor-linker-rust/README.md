# athanor-linker-rust

Rust codebase relation linker adapter.

Implements: `Linker`

## What It Links

- **Module Containment (`Contains`)**: Connects parent module/symbol entities to child entities (e.g. `crate` contains `crate::auth`).
- **Imports (`Imports`)**: Resolves module use paths to target symbols in the codebase.
- **Call Graph (`Calls`)**: Resolves function call expressions and path references to target functions/methods.
- **Tested By (`TestedBy`)**: Links functions/methods to the `TestCase`s that invoke them.

All emitted relations combine evidence and ownership from both sides.

## Inputs

`LinkInput` containing entities parsed from `athanor-extractor-rust`.

## Side Effects

None.

## Test

```bash
cargo test -p athanor-linker-rust
```
