# Runtime composition migration

Athanor active entrypoints create `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, daemon, MCP, API, Docs, Repair, Search, Generation and
indexing do not install or read process-global runtime factories.

## Current state

`COMP-003A`, `COMP-003B1`, `COMP-003B2`, `COMP-003C1`, `COMP-003C2A`, `COMP-003C2B1`,
`COMP-003C2B2A`, `COMP-003C2B2B1`, `COMP-003C2B2B2A` and `COMP-003C2B2B2B` are implemented.

- the former Store task-local bridge and factory-introspection helpers are removed;
- `legacy-global-runtime` is removed from Cargo features and CI;
- adapter, projector, Store and Search `OnceLock` owners are removed;
- legacy factory error types and every runtime installer API are removed;
- unit tests use a fresh local `test_runtime::composition()` rather than installation;
- `RuntimeBuilder::new` has no production built-ins or hidden resolver fallback;
- parallel Store/Search/Wiki/HTML compositions have an isolation regression matrix;
- dead no-composition Validate and Search wrappers are removed;
- normal and operation-aware Context cores route Store/Search only through supplied composition;
- daemon snapshot/search queries, derived reads and write jobs are composition-only;
- `DaemonState.composition` is mandatory;
- the only daemon serve entrypoint is `serve_daemon_with_composition`;
- control/write command dispatch is owned by bounded `daemon_command_dispatch.rs` rather than a
  duplicate dispatcher inside the host lifecycle owner;
- cancellation/deadline and write-report contract tests live in separate bounded test owners;
- the previous 823-line `context.rs` owner remains quarantined outside the compiled module graph
  until final compatibility cleanup.

There is no process-global runtime state or runtime installer API in the application and default
runtime crates.

## Context migration status

`COMP-009` exposed a compile-level inconsistency after `COMP-003C2B1`: the old active Context owner
still called the removed `get_or_build_search_index` wrapper. The active module now uses
`get_or_build_search_index_with_factory` and `RuntimeComposition::build_search_index` directly.

The operation-aware core receives mandatory composition and uses
`RuntimeComposition::init_store` and `build_search_index_with_operation_context`. Imports of
no-composition Store/Search helpers and internal `Option<RuntimeComposition>` branches are gone.

The replacement preserves Context pack ranking, relation expansion, diagnostics and explicit limit
behavior through integration regressions. Temporary public Context compatibility edges remain only
for callers covered by the final service API cleanup; daemon execution no longer uses them.

## Daemon migration status

`COMP-003C2B2B2A/B` completes explicit composition through daemon execution and construction:

- `daemon_queries` creates Store and Search only from `DaemonState.composition`;
- derived Context and ChangeMap dispatch calls only composition-aware operations;
- Index, Generate, Wiki and HTML jobs call only composition-aware cancellable APIs;
- the daemon state cannot be constructed without a `RuntimeComposition`;
- legacy `serve_daemon` and optional `serve_daemon_inner` are removed;
- the former giant `daemon.rs::execute_request` owner is removed;
- read dispatch delegates control/write requests to the bounded command dispatcher;
- source enforcement prevents optional host state, fallback services and duplicate dispatch from
  returning.

`athd` already passed production composition explicitly, so the production command line remains on
the same supported path while external embedders must use the explicit serve API.

## Remaining no-composition compatibility surface

`COMP-003C2B2C` is the remaining composition cleanup package:

1. require composition in Index, Generation, Wiki and benchmark public APIs;
2. remove Generation/Wiki projector fallback branches;
3. remove remaining Store and snapshot Search compatibility edges;
4. delete obsolete public Context compatibility edges after caller migration;
5. physically delete the quarantined legacy `context.rs` owner;
6. update stable re-exports, tests and embedding examples.

This surface is state-free, but signatures that omit required dependencies remain misleading for
embedders and must not be used by new integrations.

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

Daemon hosts must use:

```rust,ignore
athanor_app::serve_daemon_with_composition(options, composition).await?;
```

The removed installer and daemon compatibility functions do not have replacement setters or
no-composition entrypoints. Custom embedders construct `RuntimeComposition::new(...)` and pass it to
composition-aware APIs.

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

`crates/athanor-app/tests/composition_isolation.rs` exercises two distinct compositions in parallel
and checks independent Store, Search, Wiki and HTML factory routing.

`crates/athanor-app/tests/context_composition_inventory.rs` verifies that compiled normal and
operation-aware Context modules contain no Store/Search fallback or optional-composition branching.

`crates/athanor-app/tests/context_pack_behavior.rs` preserves ranking, relation expansion,
diagnostic inclusion and limit behavior from the replaced Context owner.

`crates/athanor-app/tests/daemon_composition_inventory.rs` verifies mandatory daemon host state,
composition-only query/read/write execution, bounded dispatch ownership and bounded test owners.

The regressions are present in `main`, but execution evidence still requires the Rust test and
Clippy matrix recorded in `athanor_implementation_plan_ru.md`.
