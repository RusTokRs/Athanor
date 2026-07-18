# Legacy runtime compatibility boundary

Athanor active entrypoints build `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, MCP, API, Docs, Repair, Search, Generation and
indexing must not call `athanor_runtime_defaults::install()`.

## Current compatibility perimeter

`COMP-003B1` and `COMP-003B2` moved every remaining process-global factory behind the
opt-in `legacy-global-runtime` feature. Default production builds compile no runtime
factory `OnceLock` state.

The compatibility globals are confined to four feature/test-only owners:

- adapter registry and built-in resolver: `runtime/legacy_registry.rs`;
- wiki and HTML projectors: `projection_legacy_global.rs`;
- Store factory: `store_legacy_global.rs`;
- Search factories: `search_legacy_global.rs`.

The corresponding default-build owners contain only explicit disabled paths and point
callers to `RuntimeComposition`.

Store and Search implementation code is independent of compatibility bootstrap:

- `store.rs` owns the Store type and pending publication state;
- `store/knowledge.rs`, `store/publication.rs` and `store/canonical.rs` own trait impls;
- `search.rs` owns search use-case orchestration;
- `search/model.rs` owns contracts and factory types;
- `search/index.rs` owns index rebuild/open lifecycle.

The former task-local Store bridge and public factory-introspection helpers are gone.
Application report and Docs services initialize stores directly through the supplied
composition.

## Feature contract

- default builds use explicit `RuntimeComposition` and contain no process-global
  adapter, projector, Store or Search factory storage;
- `--features legacy-global-runtime` enables the limited compatibility bootstrap;
- `athanor-runtime-defaults/legacy-global-runtime` forwards the feature to
  `athanor-app`;
- `--all-features` exercises the compatibility window until `COMP-003C` removes it;
- unit tests retain controlled bootstrap through `cfg(test)`.

## Removal sequence

### COMP-003A — implemented, execution pending

- remove the unused task-local Store composition bridge;
- remove public factory-introspection helpers;
- enforce that active entrypoints use explicit composition;
- document the exact global-state perimeter.

### COMP-003B1 — implemented, execution pending

- add and forward the opt-in `legacy-global-runtime` feature;
- move adapter and projector globals into feature/test-only owners;
- make default no-composition adapter/projector paths fail fast;
- add an explicit legacy feature CI slice and source enforcement.

### COMP-003B2 — implemented, execution pending

- remove Store and Search factory storage from core implementation files and facades;
- create one feature/test-only global owner for Store and one for Search;
- split Store trait implementations and Search contracts/index lifecycle into bounded modules;
- make default no-composition Store/Search paths return actionable disabled errors;
- extend source enforcement and default/legacy/all-features verification commands.

### COMP-003C — active

- migrate or remove the remaining public no-composition wrappers;
- delete `athanor_runtime_defaults::install()`;
- delete legacy factory errors and all compatibility owner modules;
- add parallel composition isolation tests with different Store, Search and projector factories.

## Enforcement

The source inventory in `crates/athanor-app/tests/legacy_factory_migration.rs`
checks feature wiring, exact global owners, bounded modules and active composition-only
entrypoints. Execution evidence still requires the Rust test and Clippy matrix recorded
in `athanor_implementation_plan_ru.md`.
