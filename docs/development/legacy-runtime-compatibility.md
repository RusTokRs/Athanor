# Runtime composition migration

Athanor active entrypoints create `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, MCP, API, Docs, Repair, Search, Generation and indexing do
not install or read process-global runtime factories.

## Current state

`COMP-003A`, `COMP-003B1`, `COMP-003B2`, `COMP-003C1`, `COMP-003C2A`, `COMP-003C2B1` and
`COMP-003C2B2A` are implemented.

- the former Store task-local bridge and factory-introspection helpers are removed;
- `legacy-global-runtime` is removed from Cargo features and CI;
- adapter, projector, Store and Search `OnceLock` owners are removed;
- legacy factory error types are removed;
- every public `install_*` symbol is removed;
- `athanor_runtime_defaults::install()` is removed;
- unit tests use a fresh local `test_runtime::composition()` rather than installation;
- `RuntimeBuilder::new` has no production built-ins or hidden resolver fallback;
- parallel Store/Search/Wiki/HTML compositions have an isolation regression matrix;
- dead no-composition `validate_changed`, `search_project`, `get_or_build_search_index` and
  `get_or_build_search_index_sync` wrappers are removed;
- the active Context owner is `context_composition.rs` and routes Store/Search only through the
  supplied `RuntimeComposition`;
- the previous 823-line `context.rs` owner is quarantined outside the compiled module graph until
  physical deletion after remaining caller migration.

There is no process-global runtime state or runtime installer API in the application and default
runtime crates.

## Context migration status

`COMP-009` exposed a compile-level inconsistency after `COMP-003C2B1`: the old active Context owner
still called the removed `get_or_build_search_index` wrapper. The active module now uses
`get_or_build_search_index_with_factory` and `RuntimeComposition::build_search_index` directly.

The replacement preserves Context pack ranking, relation expansion, diagnostics and explicit limit
behavior through integration regressions. A temporary `context_project` compatibility edge remains
only for callers that have not yet migrated; production calls fail explicitly and test builds use a
fresh local composition.

## Remaining no-composition compatibility surface

`COMP-003C2B2` remains active and is split into bounded follow-up packages:

- `COMP-003C2B2B`: require composition through operation-aware Context, daemon queries and daemon
  host state; remove `Option<RuntimeComposition>` branches;
- `COMP-003C2B2C`: require composition in Index, Generation, Wiki and benchmark APIs and remove
  Generation/Wiki projector fallback branches;
- final cleanup: remove Store/snapshot Search compatibility edges and physically delete the
  quarantined legacy Context owner.

This surface is state-free, but it is still misleading for embedders because some signatures do
not expose the dependencies required for successful production execution. It must not be used by
new integrations.

## Breaking change for embedders

Replace installer/bootstrap code:

```rust,ignore
athanor_runtime_defaults::install();
```

with an owned or shared composition passed explicitly:

```rust,ignore
let composition = athanor_runtime_defaults::production();
let report = athanor_app::index_project_with_composition(options, &composition).await?;
```

The removed installer functions do not have replacement setters. Custom embedders construct
`RuntimeComposition::new(...)` and pass it to composition-aware APIs.

Removed dead wrapper migrations:

```rust,ignore
// before
let report = athanor_app::validate_changed(options).await?;
let report = athanor_app::search_project(options).await?;

// after
let report = athanor_app::validate_changed_with_composition(options, &composition).await?;
let report = athanor_app::search_project_with_composition(options, &composition).await?;
```

Context callers should migrate to:

```rust,ignore
let pack = athanor_app::context_project_with_composition(options, &composition).await?;
```

## Enforcement

`crates/athanor-app/tests/legacy_factory_migration.rs` verifies that Cargo, CI, Runtime,
Projection, Store, Search and runtime-default sources contain no process-global state, legacy
feature wiring, installer functions or removed dead no-composition wrappers.

`crates/athanor-app/tests/composition_isolation.rs` exercises two distinct compositions in
parallel and checks independent Store, Search, Wiki and HTML factory routing.

`crates/athanor-app/tests/context_composition_inventory.rs` verifies that the compiled Context
module points to the composition-first owner and contains no Store/Search fallback branching.

`crates/athanor-app/tests/context_pack_behavior.rs` preserves ranking, relation expansion,
diagnostic inclusion and limit behavior from the replaced Context owner.

The regressions are present in `main`, but execution evidence still requires the Rust test and
Clippy matrix recorded in `athanor_implementation_plan_ru.md`.
