# Rust Extractor

Crate: `athanor-extractor-rust`

Port: `Extractor`

The Rust extractor parses source files with `syn` and emits canonical module, function, and symbol entities plus `symbol_defined` facts pointing to the canonical file entity.

Stable keys use the `symbol://rust:` namespace and a collision-resistant crate/module prefix derived from the project-relative source path. Workspace member directory names become crate names with hyphens normalized to underscores. Parser spans provide line evidence; every entity and fact is owned by the source file for incremental replacement.

The adapter runs in-process, has no network access, and has no side effects. Alternative `cfg` definitions with the same qualified name are coalesced with multiple evidence spans. It does not expand macros, emit trait methods, link imports/calls, or infer framework routes.

Test with:

```bash
cargo test -p athanor-extractor-rust
```
