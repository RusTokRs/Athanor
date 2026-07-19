# Runtime composition migration

Athanor active entrypoints create `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, daemon, MCP, API, Docs, Repair, Search, Generation and
indexing do not install or read process-global runtime factories.

## Current state

The following migration packages are implemented in `main`:

- `COMP-003A`, `COMP-003B1`, `COMP-003B2`;
- `COMP-003C1`, `COMP-003C2A`, `COMP-003C2B1`;
- `COMP-003C2B2A`, `COMP-003C2B2B1`, `COMP-003C2B2B2A`, `COMP-003C2B2B2B`;
- `COMP-003C2B2C1`, `COMP-003C2B2C2A`;
- composition-only slices of `COMP-003C2B2C2B` listed below.

Implemented invariants:

- the former Store task-local bridge and factory-introspection helpers are removed;
- `legacy-global-runtime` is removed from Cargo features and CI;
- adapter, projector, Store and Search `OnceLock` owners are removed;
- legacy factory error types and every runtime installer API are removed;
- unit tests use a fresh local `test_runtime::composition()` rather than installation;
- `RuntimeBuilder::new` has no production built-ins or hidden resolver fallback;
- parallel Store/Search/Wiki/HTML compositions have an isolation regression matrix;
- normal and operation-aware Context cores route Store/Search only through supplied composition;
- daemon host, queries, derived reads and write jobs require composition;
- Index, Generation, Wiki, HTML report and benchmark public APIs require composition;
- Search, Explain, ChangeMap, API Registry, Overview, Capabilities, Impact and Coverage require
  composition;
- standard operation-aware Graph reads require composition;
- Repair latest/recovery execution has one composition owner;
- Docs check, drift, proposal and patch execution routes only through composition owners.

There is no process-global runtime state or runtime installer API in the application and default
runtime crates.

## Context migration

The active `context_composition.rs` owner uses supplied Store/Search factories directly. Normal and
operation-aware cores accept mandatory `&RuntimeComposition`.

Removed compatibility surface:

- `context_project`;
- `context_project_with_operation_context`;
- no-composition ChangeMap and Search operation wrappers;
- the obsolete `context.rs` owner;
- duplicate `rustok_operation.rs` execution.

Context pack ranking, relation expansion, diagnostics and explicit limit behavior remain covered by
integration regressions.

## Daemon migration

Daemon construction and execution are composition-only:

- `DaemonState.composition` is mandatory;
- the public serve API is `serve_daemon_with_composition`;
- snapshot/search queries use Store and Search from daemon composition;
- derived Context and ChangeMap dispatch invokes composition-aware operations;
- Index, Generate, Wiki and HTML jobs use composition-aware APIs;
- command dispatch is separated from transport lifecycle;
- source enforcement prevents optional host state and fallback services from returning.

## Write-service migration

Index, Generation, Wiki, HTML report and benchmark expose only composition-aware execution.
Generation workers clone the supplied composition for parallel projection. Store, RuntimeBuilder and
projector fallback branches are removed.

The CLI, daemon and MCP were already using the retained composition-aware symbols, so removal of the
old entrypoints did not require replacement shims.

## Read-service migration

The following read owners are composition-only and source-enforced:

- Search and operation-aware snapshot Search;
- Explain;
- ChangeMap;
- API Registry;
- Overview;
- Capabilities;
- Impact;
- Coverage;
- standard operation-aware Graph reads.

ChangeMap, Overview, Capabilities, Impact and Coverage are split into conventional bounded modules
without `include!` or forwarding facades.

## Repair and Docs migration

Repair latest and publication recovery models remain in `repair_latest.rs` and
`repair_recovery.rs`. Their duplicate no-composition execution paths are removed. Execution is owned
by `repair_composition/direct.rs` and exposed through:

```rust,ignore
repair_canonical_latest_with_composition(options, &composition).await?;
recover_index_publication_with_composition(options, &composition).await?;
```

The old `docs/service.rs` owner is physically deleted. Documentation operations are exposed only
through the composition facade:

```rust,ignore
check_docs_with_composition(options, &composition).await?;
docs_drift_with_composition(options, &composition).await?;
docs_propose_fix_with_composition(options, &composition).await?;
docs_apply_patch_with_composition(options, &composition).await?;
```

## Remaining compatibility surface

`COMP-003C2B2C2B` remains active until these production owners are migrated:

1. `graph.rs` — no-composition standard and RusTok project loaders plus a 4.8k-line mixed owner;
2. `check.rs` — diagnostic, affected-file and operations-docs Store fallbacks;
3. `api.rs` — API contract snapshot Store fallback;
4. public `store::init_store` facade, removable after all callers are migrated.

`graph_operation.rs` is already composition-only. The remaining `graph.rs` work is a physical owner
split, not an operation-context migration.

New integrations must not call `crate::store::init_store` or introduce APIs that omit required
runtime dependencies.

## Breaking change for embedders

Replace installer/bootstrap code:

```rust,ignore
athanor_runtime_defaults::install();
```

with an owned or shared composition:

```rust,ignore
let composition = athanor_runtime_defaults::production();
let report = athanor_app::index_project_with_composition(options, &composition).await?;
```

Daemon hosts use:

```rust,ignore
athanor_app::serve_daemon_with_composition(options, composition).await?;
```

Read and write services follow the same pattern:

```rust,ignore
athanor_app::overview_project_with_composition(options, &composition).await?;
athanor_app::generate_project_with_composition(options, &composition).await?;
athanor_app::context_project_with_composition_and_operation_context(
    options,
    &composition,
    &operation,
).await?;
```

Removed functions do not have compatibility aliases, implicit test runtimes or replacement setters.

## Enforcement

The migration is source-enforced by:

- `legacy_factory_migration.rs` — Cargo, CI, Runtime, Projection, Store and Search global/runtime
  compatibility removal;
- `write_service_composition_inventory.rs` — Index, Generation, Wiki, HTML and benchmark;
- `context_composition_inventory.rs` — Context and RusTok composition routing;
- `daemon_composition_inventory.rs` — daemon host/query/read/write execution;
- `read_service_composition_inventory.rs` — Search, read owners, Graph operation, Repair and Docs;
- `composition_isolation.rs` — parallel independent Store/Search/Wiki/HTML compositions.

The regressions are present in `main`, but implementation status is not promoted to `verified` until
the Rust test and Clippy matrix in `athanor_implementation_plan_ru.md` runs on one commit.
