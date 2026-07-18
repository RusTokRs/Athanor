# Legacy runtime compatibility boundary

Athanor active entrypoints build `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, MCP, API, Docs, Repair, Search, Generation and
indexing must not call `athanor_runtime_defaults::install()`.

## Current compatibility perimeter

`COMP-003B1` moved adapter-registry and projector globals behind the opt-in
`legacy-global-runtime` feature. In a default production build those owners compile
only fail-fast compatibility stubs and contain no `OnceLock` state. Unit tests retain
the controlled bootstrap through `cfg(test)`.

The remaining process-global state is confined to two owner groups scheduled for
`COMP-003B2`:

- store factory guards in `store_facade.rs` and the legacy implementation module;
- search-index factory guards in `search_facade.rs` and the legacy implementation module.

These owners support public no-composition wrappers during the compatibility window.
They are not application composition roots.

The former task-local Store bridge has been removed. Application report and Docs
services initialize stores directly through the supplied composition. Factory lookup
helpers are private to their compatibility facades.

## Feature contract

- default builds use explicit `RuntimeComposition` and do not compile adapter/projector
  process-global storage;
- `--features legacy-global-runtime` enables the limited compatibility bootstrap;
- `athanor-runtime-defaults/legacy-global-runtime` forwards the feature to
  `athanor-app`;
- `--all-features` exercises the compatibility window until `COMP-003C` removes it.

## Removal sequence

### COMP-003A — completed

- remove the unused task-local Store composition bridge;
- remove public factory-introspection helpers;
- enforce that active entrypoints use explicit composition;
- document the exact global-state perimeter.

### COMP-003B1 — implemented, execution pending

- add and forward the opt-in `legacy-global-runtime` feature;
- move adapter and projector globals into feature/test-only owners;
- make default no-composition adapter/projector paths fail fast;
- add an explicit legacy feature CI slice and source enforcement.

### COMP-003B2 — active

- extract Store and Search factory storage from the large implementation modules;
- compile those globals only for `legacy-global-runtime` or unit tests;
- keep default no-composition wrappers as explicit disabled errors;
- verify default, legacy-feature and all-features builds.

### COMP-003C — final

- migrate or remove the remaining no-composition public wrappers;
- delete `athanor_runtime_defaults::install()`;
- delete legacy factory errors and compatibility owner modules;
- verify parallel compositions with different Store, Search and projector factories.

## Enforcement

The source inventory in `crates/athanor-app/tests/legacy_factory_migration.rs`
checks feature wiring, the compatibility perimeter and active composition-only
entrypoints. Execution evidence still requires the Rust test and Clippy matrix
recorded in `athanor_implementation_plan_ru.md`.
