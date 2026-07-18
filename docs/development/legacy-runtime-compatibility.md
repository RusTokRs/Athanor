# Runtime composition migration

Athanor active entrypoints create `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, MCP, API, Docs, Repair, Search, Generation and indexing do
not install or read process-global runtime factories.

## Current state

`COMP-003A`, `COMP-003B1`, `COMP-003B2`, `COMP-003C1`, `COMP-003C2A`, `COMP-003C2B1`,
`COMP-003C2B2A`, `COMP-003C2B2B1` and `COMP-003C2B2B2A` are implemented.

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
- operation-aware Context internals also require `&RuntimeComposition` and contain no Store/Search
  fallback branches;
- daemon snapshot/search queries and derived Context/ChangeMap reads reject a missing composition
  instead of invoking compatibility Store/Search/application services;
- daemon Index/Generate/Wiki/HTML jobs validate composition before job creation and call only
  composition-aware execution APIs;
- the previous 823-line `context.rs` owner is quarantined outside the compiled module graph until
  physical deletion after remaining caller migration.

There is no process-global runtime state or runtime installer API in the application and default
runtime crates.

## Context migration status

`COMP-009` exposed a compile-level inconsistency after `COMP-003C2B1`: the old active Context owner
still called the removed `get_or_build_search_index` wrapper. The active module now uses
`get_or_build_search_index_with_factory` and `RuntimeComposition::build_search_index` directly.

The operation-aware core now also receives mandatory composition. It uses
`RuntimeComposition::init_store` and `build_search_index_with_operation_context`; imports of
no-composition Store/Search helpers and internal `Option<RuntimeComposition>` branching are gone.

The replacement preserves Context pack ranking, relation expansion, diagnostics and explicit limit
behavior through integration regressions. Temporary `context_project` and
`context_project_with_operation_context` compatibility edges remain only for callers that have not
yet migrated; production calls fail explicitly and test builds use a fresh local composition.

## Daemon migration status

`COMP-003C2B2B2A` removes fallback execution from the daemon read and write layers:

- `daemon_queries` creates Store and Search only from the daemon composition;
- derived Context and ChangeMap dispatch calls only composition-aware operations;
- Index, Generate, Wiki and HTML jobs call only composition-aware cancellable APIs;
- a missing daemon composition is rejected explicitly before write-job creation;
- source enforcement prevents no-composition imports and fallback branches from returning.

The daemon host itself is not yet fully migrated. `DaemonState.composition` remains optional and the
legacy `serve_daemon`/`serve_daemon_inner(..., Option<...>)` construction path still exists. Active
`athd` production startup already uses `serve_daemon_with_composition`, but host-state cleanup and
test constructor migration remain `COMP-003C2B2B2B`.

## Remaining no-composition compatibility surface

`COMP-003C2B2` remains active and is split into bounded follow-up packages:

- `COMP-003C2B2B2B`: make `DaemonState` composition mandatory, remove the legacy daemon serve
  entrypoint and migrate daemon test constructors;
- `COMP-003C2B2C`: require composition in Index, Generation, Wiki and benchmark public APIs and
  remove Generation/Wiki projector fallback branches;
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
let report = athanor_app::context_project_with_composition_and_operation_context(
    options,
    &composition,
    &operation,
).await?;
```

## Enforcement

`crates/athanor-app/tests/legacy_factory_migration.rs` verifies that Cargo, CI, Runtime,
Projection, Store, Search and runtime-default sources contain no process-global state, legacy
feature wiring, installer functions or removed dead no-composition wrappers.

`crates/athanor-app/tests/composition_isolation.rs` exercises two distinct compositions in
parallel and checks independent Store, Search, Wiki and HTML factory routing.

`crates/athanor-app/tests/context_composition_inventory.rs` verifies that compiled normal and
operation-aware Context modules contain no Store/Search fallback or optional-composition branching.

`crates/athanor-app/tests/context_pack_behavior.rs` preserves ranking, relation expansion,
diagnostic inclusion and limit behavior from the replaced Context owner.

`crates/athanor-app/tests/daemon_composition_inventory.rs` verifies composition-only daemon query,
derived-read and write-job execution while explicitly tracking the remaining optional host state.

The regressions are present in `main`, but execution evidence still requires the Rust test and
Clippy matrix recorded in `athanor_implementation_plan_ru.md`.
