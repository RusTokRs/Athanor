# Runtime composition migration

Athanor active entrypoints create `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, daemon, MCP, API, Docs, Repair, Search, Generation and
indexing do not install or read process-global runtime factories.

## Current state

`COMP-003A`, `COMP-003B1`, `COMP-003B2`, `COMP-003C1`, `COMP-003C2A`, `COMP-003C2B1`,
`COMP-003C2B2A`, `COMP-003C2B2B1`, `COMP-003C2B2B2A`, `COMP-003C2B2B2B`,
`COMP-003C2B2C1` and `COMP-003C2B2C2A` are implemented.

- the former Store task-local bridge and factory-introspection helpers are removed;
- `legacy-global-runtime` is removed from Cargo features and CI;
- adapter, projector, Store and Search `OnceLock` owners are removed;
- legacy factory error types and every runtime installer API are removed;
- unit tests use a fresh local `test_runtime::composition()` rather than installation;
- `RuntimeBuilder::new` has no production built-ins or hidden resolver fallback;
- parallel Store/Search/Wiki/HTML compositions have an isolation regression matrix;
- dead no-composition Validate and Search project wrappers are removed;
- normal and operation-aware Context cores route Store/Search only through supplied composition;
- public no-composition Context and derived-read operation wrappers are removed;
- the obsolete `context.rs` and `rustok_operation.rs` owners are physically deleted;
- RusTok Context execution is owned by `rustok_composition_operation.rs`;
- daemon snapshot/search queries, derived reads and write jobs are composition-only;
- `DaemonState.composition` is mandatory and the only daemon serve entrypoint is
  `serve_daemon_with_composition`;
- Index, Generation, Wiki, HTML report and benchmark public APIs require composition;
- write-service Store, RuntimeBuilder and projector fallback branches are removed;
- stable indexing re-exports expose only composition-aware entrypoints.

There is no process-global runtime state or runtime installer API in the application and default
runtime crates.

## Context migration status

`COMP-009` exposed a compile-level inconsistency after `COMP-003C2B1`: the old active Context owner
still called a removed Search wrapper. The active `context_composition.rs` owner uses
`get_or_build_search_index_with_factory` and `RuntimeComposition::build_search_index` directly.

The operation-aware core receives mandatory composition and uses
`RuntimeComposition::init_store` and `build_search_index_with_operation_context`. Imports of
no-composition Store/Search helpers and internal `Option<RuntimeComposition>` branches are gone.

`COMP-003C2B2C2A` completes the Context compatibility cleanup:

- `context_project` is removed;
- `context_project_with_operation_context` is removed;
- no-composition ChangeMap and Search operation wrappers are removed;
- the old 823-line `context.rs` owner is deleted rather than retained as quarantine;
- duplicate no-composition RusTok operation execution is deleted;
- the RusTok architecture model contains only contracts and pure snapshot transformation;
- source enforcement checks physical owner absence and composition-only routing.

Context pack ranking, relation expansion, diagnostics and explicit limit behavior remain covered by
integration regressions.

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

## Write-service migration status

`COMP-003C2B2C1` removes dependency-free signatures from every write-service family:

- Index exposes normal and cancellable composition-aware calls, with optional operation metadata;
- Generation exposes normal and cancellable composition-aware calls, with optional operation
  metadata;
- Wiki and HTML report expose normal and cancellable composition-aware calls, with optional
  operation metadata;
- benchmark indexing requires composition and invokes the same composition-aware Index path;
- Generation workers clone the supplied composition for parallel Wiki and HTML projection;
- no write-service core imports `crate::store::init_store`;
- no write-service core accepts `Option<RuntimeComposition>`;
- `projection.rs` contains only projection schema constants and the projector factory contract.

The CLI, daemon and MCP were already using the retained composition-aware symbols, so this package
removes misleading embedding APIs without adding replacement shims.

## Remaining no-composition compatibility surface

`COMP-003C2B2C2B` is the remaining composition cleanup package:

1. remove the Store facade `init_store` compatibility edge;
2. migrate read-service owners that still import `crate::store::init_store` or accept optional
   composition;
3. make ChangeMap composition-only and route task search through supplied composition;
4. remove snapshot Search and operation-aware Search-index compatibility edges;
5. update stable re-exports, tests, examples and architecture documentation.

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

Removed write-service entrypoints map directly to composition-aware variants:

```rust,ignore
// before
athanor_app::index_project(options).await?;
athanor_app::generate_project(options).await?;
athanor_app::project_wiki(options).await?;
athanor_app::project_html_report(options).await?;
athanor_app::benchmark_index(options).await?;

// after
athanor_app::index_project_with_composition(options, &composition).await?;
athanor_app::generate_project_with_composition(options, &composition).await?;
athanor_app::project_wiki_with_composition(options, &composition).await?;
athanor_app::project_html_report_with_composition(options, &composition).await?;
athanor_app::benchmark_index_with_composition(options, &composition).await?;
```

Context and operation-aware callers migrate directly:

```rust,ignore
let pack = athanor_app::context_project_with_composition(options, &composition).await?;
let report = athanor_app::context_project_with_composition_and_operation_context(
    options,
    &composition,
    &operation,
).await?;
```

The removed functions do not have compatibility aliases, implicit test runtimes or replacement
setters.

## Enforcement

`crates/athanor-app/tests/legacy_factory_migration.rs` verifies that Cargo, CI, Runtime,
Projection, Store, Search and runtime-default sources contain no process-global state, legacy
feature wiring, installer functions or removed no-composition wrappers.

`crates/athanor-app/tests/write_service_composition_inventory.rs` verifies composition-only Index,
Generation, Wiki, HTML and benchmark signatures, mandatory Store/projector routing, stable
re-exports, active CLI callers and bounded owners.

`crates/athanor-app/tests/context_composition_inventory.rs` verifies mandatory normal and
operation-aware Context routing, composition-owned RusTok execution, removal of no-composition
operation APIs, and physical deletion of obsolete Context/RusTok owners.

`crates/athanor-app/tests/context_pack_behavior.rs` preserves ranking, relation expansion,
diagnostic inclusion and limit behavior from the replaced Context owner.

`crates/athanor-app/tests/composition_isolation.rs` exercises two distinct compositions in parallel
and checks independent Store, Search, Wiki and HTML factory routing.

`crates/athanor-app/tests/daemon_composition_inventory.rs` verifies mandatory daemon host state,
composition-only query/read/write execution, bounded dispatch ownership and bounded test owners.

The regressions are present in `main`, but execution evidence still requires the Rust test and
Clippy matrix recorded in `athanor_implementation_plan_ru.md`.
