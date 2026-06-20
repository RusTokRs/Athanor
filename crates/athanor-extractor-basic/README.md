# athanor-extractor-basic

Universal extractor adapter for source files.

Implements: `Extractor`

## What It Emits

- `EntityKind::File`
- `FactKind::FileDiscovered`

Both emitted object types include ownership metadata for the discovered source file path.

## Inputs

`SourceFile` from any `SourceProvider`.

## Side Effects

None.

This adapter does not run commands, does not use the network, and does not modify project files.

## Test

```bash
cargo test -p athanor-extractor-basic
```
