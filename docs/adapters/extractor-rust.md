---
id: doc://docs/adapters/extractor-rust.md
kind: module_documentation
language: en
last_verified_snapshot: snap_jsonl_00000251
source_language: en
status: verified
---
# Rust Extractor

Crate: `athanor-extractor-rust`

Port: `Extractor`

The Rust extractor parses source files with `syn` and emits canonical module, function, symbol, and
environment-variable entities plus `symbol_defined` and `env_var_used` facts pointing to canonical
evidence.

Stable keys use the `symbol://rust:` namespace and a collision-resistant crate/module prefix derived from the project-relative source path. Workspace member directory names become crate names with hyphens normalized to underscores. Parser spans provide line evidence; every entity and fact is owned by the source file for incremental replacement.

Environment variables are detected from common Rust calls such as `std::env::var`,
`std::env::var_os`, and `option_env!`, then emitted as `EntityKind::EnvVar` with `env://<NAME>`
stable keys. `EnvDocsChecker` can later report undocumented variables through `ath check env`.

The adapter runs in-process, has no network access, and has no side effects. Alternative `cfg` definitions with the same qualified name are coalesced with multiple evidence spans. It does not expand macros, emit trait methods, link imports/calls, or infer framework routes.

Test with:

```bash
cargo test -p athanor-extractor-rust
```
