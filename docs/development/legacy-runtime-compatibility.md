---
id: doc://docs/development/legacy-runtime-compatibility.md
kind: developer_guide
language: en
source_language: en
status: implemented
---
# Runtime composition migration

Athanor active entrypoints create `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, daemon, MCP, API, Docs, Repair, Search, Generation, Graph and
indexing do not install or read process-global runtime factories.

## Current state

`COMP-003` and its migration packages through `COMP-003C2B2C2B` are implemented in `main`.
Execution verification remains separate: implementation is not promoted to `verified` until the Rust
fmt/check/test/Clippy matrix runs on one commit.

Implemented invariants:

- the former Store task-local bridge and factory-introspection helpers are removed;
- `legacy-global-runtime` is removed from Cargo features and CI;
- adapter, projector, Store and Search `OnceLock` owners are removed;
- legacy factory error types and every runtime installer API are removed;
- unit tests use a fresh local `test_runtime::composition()` rather than installation;
- `RuntimeBuilder::new` has no production built-ins or hidden resolver fallback;
- parallel Store/Search/Wiki/HTML compositions have an isolation regression matrix;
- Context, daemon, write services and read services route dependencies through supplied composition;
- API contract and Graph project snapshot loading require composition;
- the public `store::init_store` compatibility API is physically removed;
- Store construction is owned only by `RuntimeComposition::init_store`.

There is no process-global runtime state, runtime installer API or dependency-hidden Store initializer
in the application and default runtime crates.

## Context and daemon migration

The active `context_composition.rs` owner uses supplied Store/Search factories directly. Normal and
operation-aware cores accept mandatory `&RuntimeComposition`.

Removed compatibility surface includes:

- `context_project`;
- `context_project_with_operation_context`;
- no-composition ChangeMap and Search operation wrappers;
- the obsolete `context.rs` owner;
- duplicate `rustok_operation.rs` execution.

Daemon construction and execution are composition-only:

- `DaemonState.composition` is mandatory;
- the public serve API is `serve_daemon_with_composition`;
- snapshot/search queries use Store and Search from daemon composition;
- derived Context and ChangeMap dispatch invokes composition-aware operations;
- Index, Generate, Wiki and HTML jobs use composition-aware APIs;
- source enforcement prevents optional host state and fallback services from returning.

## Write-service migration

Index, Generation, Wiki, HTML report and benchmark expose only composition-aware execution.
Generation workers clone the supplied composition for parallel projection. Store, RuntimeBuilder and
projector fallback branches are removed.

The CLI, daemon and MCP were already using the retained composition-aware symbols, so removal of old
entrypoints did not require replacement shims.

## Read-service migration

The following read owners are composition-only and source-enforced:

- Search and operation-aware snapshot Search;
- Explain and ChangeMap;
- API Registry and API contract snapshot;
- Overview, Capabilities, Impact and Coverage;
- Check, affected Check and operations-docs Check;
- standard and RusTok operation-aware Graph reads;
- Repair latest and publication recovery;
- Docs check, drift, proposal and patch application.

ChangeMap, Overview, Capabilities, Impact, Coverage, Check, API contracts and Graph are split into
conventional bounded modules without `include!` or forwarding compatibility facades.

API contract snapshot Store loading is owned by `application_report_composition/api_direct.rs`.
Immutable snapshot publication, diff analysis and retention cleanup are pure bounded owners under
`api/`.

## Graph migration

The former 4.8k-line `crates/athanor-app/src/graph.rs` owner is physically deleted. Graph contracts and
pure snapshot operations now live under:

- `graph/model.rs` — schemas, options and public report models;
- `graph/standard.rs` — export, GraphML, hubs and pure standard Graph adapters;
- `graph/rustok.rs` — pure RusTok audit and graph adapters;
- `graph/tests.rs` — bounded contract regressions.

Traversal-heavy standard operations use the single cooperative algorithm owner in
`graph_cooperative.rs`. RusTok audit and graph adapters use `rustok_audit_cooperative.rs` and
`rustok_graph_cooperative.rs`. Project snapshot loading remains in `graph_operation.rs` and
`rustok_composition_operation.rs`, where mandatory composition and operation context are explicit.

Removed Graph compatibility surface includes all no-composition standard and RusTok async project
loaders, `load_latest_graph_snapshot` and the final production call to `crate::store::init_store`.

## Repair and Docs migration

Repair latest and publication recovery models remain in `repair_latest.rs` and
`repair_recovery.rs`. Their duplicate no-composition execution paths are removed. Execution is owned
by `repair_composition/direct.rs` and exposed through:

```rust,ignore
repair_canonical_latest_with_composition(options, &composition).await?;
recover_index_publication_with_composition(options, &composition).await?;
```

The old `docs/service.rs` owner is physically deleted. Documentation operations are exposed only
through composition owners:

```rust,ignore
check_docs_with_composition(options, &composition).await?;
docs_drift_with_composition(options, &composition).await?;
docs_propose_fix_with_composition(options, &composition).await?;
docs_apply_patch_with_composition(options, &composition).await?;
```

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

Store construction also requires composition:

```rust,ignore
let store = composition.init_store(&root, &config).await?;
```

Read and write services follow the same pattern:

```rust,ignore
athanor_app::overview_project_with_composition(options, &composition).await?;
athanor_app::snapshot_api_contract_with_composition(options, &composition).await?;
athanor_app::generate_project_with_composition(options, &composition).await?;
athanor_app::export_graph_with_composition_and_operation_context(
    options,
    &composition,
    &operation,
).await?;
```

Removed functions do not have compatibility aliases, implicit test runtimes or replacement setters.
New integrations must not add APIs that omit required runtime dependencies.

## Enforcement

The migration is source-enforced by:

- `legacy_factory_migration.rs` — Cargo, CI, Runtime, Projection, Store and Search compatibility
  removal;
- `write_service_composition_inventory.rs` — Index, Generation, Wiki, HTML and benchmark;
- `context_composition_inventory.rs` — Context and RusTok composition routing;
- `daemon_composition_inventory.rs` — daemon host/query/read/write execution;
- `read_service_composition_inventory.rs` — Search and bounded read owners;
- `check_composition_inventory.rs` — Check routing, physical decomposition and line budgets;
- `api_composition_inventory.rs` — API snapshot routing, bounded contracts and active CLI usage;
- `graph_composition_inventory.rs` — physical Graph decomposition, composition-only project loading,
  cooperative algorithm ownership, Store facade removal and line budgets;
- `composition_isolation.rs` — parallel independent Store/Search/Wiki/HTML compositions.

The regressions are present in `main`, but implementation status is not promoted to `verified` until
the Rust test and Clippy matrix in `athanor_implementation_plan_ru.md` runs on one commit.
