# athanor-extractor-rust

Rust source-code extractor adapter.

Implements: `Extractor`

## What It Emits

- `EntityKind::Module` for inline module declarations
- `EntityKind::Function` for free functions and implementation methods
- `EntityKind::Symbol` for structs, enums, traits, unions, type aliases, constants, and statics
- `FactKind::SymbolDefined` from every emitted symbol to its canonical file entity

All emitted objects include ownership metadata for the Rust source file. Facts include exact parser-derived line evidence.

## Inputs

`SourceFile` with:

```text
language_hint = rust
content = UTF-8 Rust source
```

## Stable Keys

```text
symbol://rust:auth_service::auth::login
symbol://rust:auth_service::auth::Session
symbol://rust:auth_service::auth::Session::refresh
```

The module prefix is derived from the project-relative file path. A root `src/lib.rs` maps to `crate`; `crates/auth-service/src/auth.rs` maps to `auth_service::auth`. Hyphens in workspace member directory names are normalized to Rust underscores.

## Side Effects

None. The adapter does not run commands, use the network, or modify project files.

## Limitations

- Macro-generated declarations are not expanded.
- Out-of-line module declarations are emitted as modules but resolved by their own source files rather than linked in this adapter.
- Functions declared inside traits are not emitted yet.
- Rust syntax errors fail extraction for the affected file.
- Alternative definitions with the same qualified name, such as platform-specific `cfg` functions, are coalesced and retain evidence for every definition.

## Test

```bash
cargo test -p athanor-extractor-rust
```
