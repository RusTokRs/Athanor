# Runtime composition migration

Athanor active entrypoints create `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, MCP, API, Docs, Repair, Search, Generation and indexing do
not install or read process-global runtime factories.

## Current state

`COMP-003A`, `COMP-003B1`, `COMP-003B2`, `COMP-003C1` and `COMP-003C2A` are implemented.

- the former Store task-local bridge and factory-introspection helpers are removed;
- `legacy-global-runtime` is removed from Cargo features and CI;
- adapter, projector, Store and Search `OnceLock` owners are removed;
- legacy factory error types are removed;
- every public `install_*` symbol is removed;
- `athanor_runtime_defaults::install()` is removed;
- unit tests use a fresh local `test_runtime::composition()` rather than installation;
- `RuntimeBuilder::new` has no production built-ins or hidden resolver fallback;
- parallel Store/Search/Wiki/HTML compositions have an isolation regression matrix.

There is no process-global runtime state or runtime installer API in the application and default
runtime crates.

## Remaining no-composition compatibility surface

`COMP-003C2B` remains active. Public no-composition service wrappers still exist in several
application families. Production variants fail explicitly instead of discovering dependencies,
while some unit-test paths create a local test composition.

This surface is state-free, but it is still misleading for embedders because the signature does
not expose the dependencies required for successful production execution. It must not be used by
new integrations.

The next breaking cleanup will:

1. remove public no-composition Store, Search, Projection and changed-validation wrappers;
2. require `RuntimeComposition` in Index, Search, Generation, Wiki, benchmark and remaining
   service APIs;
3. remove internal `Option<RuntimeComposition>` branching from composition-owned operations;
4. update stable re-export modules and migration examples to composition-only signatures;
5. execute the default/all-features test and Clippy matrix.

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

## Enforcement

`crates/athanor-app/tests/legacy_factory_migration.rs` verifies that Cargo, CI, Runtime,
Projection, Store, Search and runtime-default sources contain no process-global state, legacy
feature wiring or installer functions.

`crates/athanor-app/tests/composition_isolation.rs` exercises two distinct compositions in
parallel and checks independent Store, Search, Wiki and HTML factory routing.

Both tests are present in `main`, but execution evidence still requires the Rust test and Clippy
matrix recorded in `athanor_implementation_plan_ru.md`.
