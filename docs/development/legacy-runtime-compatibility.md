# Legacy runtime compatibility boundary

Athanor active entrypoints build `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, MCP, API, Docs, Repair, Search, Generation and
indexing must not call `athanor_runtime_defaults::install()`.

## Current compatibility perimeter

The remaining process-global state is confined to four owner groups:

- adapter registry and built-in adapter resolver in `runtime/legacy_registry.rs`;
- wiki and HTML projector factories in `projection.rs`;
- store factory guards in `store_facade.rs` and the legacy implementation module;
- search-index factory guards in `search_facade.rs` and the legacy implementation module.

These owners support public no-composition wrappers during the compatibility window.
They are not application composition roots.

The former task-local Store bridge has been removed. Application report and Docs
services initialize stores directly through the supplied composition. Factory lookup
helpers are private to their compatibility facades.

## Removal sequence

### COMP-003A — completed

- remove the unused task-local Store composition bridge;
- remove public factory-introspection helpers;
- enforce that active entrypoints use explicit composition;
- document the exact global-state perimeter.

### COMP-003B — next

- add an opt-in `legacy-global-runtime` feature;
- exclude adapter, projector, Store and Search globals from default production builds;
- keep compatibility wrappers only behind that feature;
- add default-build and all-features compile checks.

### COMP-003C — final

- migrate or remove the remaining no-composition public wrappers;
- delete `athanor_runtime_defaults::install()`;
- delete legacy factory errors and duplicated implementation-level `OnceLock`s;
- verify parallel compositions with different Store, Search and projector factories.

## Enforcement

The source inventory in `crates/athanor-app/tests/legacy_factory_migration.rs`
checks the compatibility perimeter. Execution evidence still requires the Rust test
and Clippy matrix recorded in `athanor_implementation_plan_ru.md`.
