---
id: doc://docs/adapters/extractor-rust.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000272
source_language: en
status: verified
---
# Rust Extractor

Crate: `athanor-extractor-rust`

Port: `Extractor`

`RustExtractor` parses source files with `syn` and emits canonical module, function, symbol, and
environment-variable entities plus `symbol_defined` and `env_var_used` facts pointing to canonical
evidence. Framework-specific semantics remain outside this base extractor.

The same crate now also exposes a separate bounded `AxumExtractor`, registered independently as
`builtin.extractor.axum`. It projects only high-confidence static Axum `.route(...)` declarations;
see [Axum Route Extractor](extractor-axum.md). Its source presence is not execution evidence for the
previously verified framework-neutral Rust extractor surface.

Stable keys use the `symbol://rust:` namespace and a collision-resistant crate/module prefix derived from the project-relative source path. Workspace member directory names become crate names with hyphens normalized to underscores. Parser spans provide line evidence; every entity and fact is owned by the source file for incremental replacement.

Environment variables are detected from common Rust calls such as `std::env::var`,
`std::env::var_os`, and `option_env!`, then emitted as `EntityKind::EnvVar` with `env://<NAME>`
stable keys. `EnvDocsChecker` can later report undocumented variables through `ath check env`.

The base adapter runs in-process, has no network access, and has no side effects. Alternative `cfg` definitions with the same qualified name are coalesced with multiple evidence spans. `RustExtractor` does not expand macros, emit trait methods, link imports/calls, or infer framework routes.

Test the framework-neutral owner with:

```bash
cargo test -p athanor-extractor-rust
```
