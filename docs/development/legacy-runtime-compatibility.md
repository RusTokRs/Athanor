# Runtime composition migration

Athanor active entrypoints create `athanor_runtime_defaults::production()` and pass a
`RuntimeComposition` explicitly. CLI, MCP, API, Docs, Repair, Search, Generation and indexing do
not install or read process-global runtime factories.

## Current state

`COMP-003A`, `COMP-003B1`, `COMP-003B2` and `COMP-003C1` are implemented.

- the former Store task-local bridge and factory-introspection helpers are removed;
- `legacy-global-runtime` is removed from Cargo features and CI;
- adapter, projector, Store and Search `OnceLock` owners are removed;
- legacy factory error types are removed;
- unit tests use a fresh local `test_runtime::composition()` rather than installation;
- production no-composition Store/Search/Projection and changed-validation paths fail explicitly;
- `RuntimeBuilder::new` has no production built-ins or hidden resolver fallback.

There is no process-global runtime state in the application crate.

## Temporary API shims

A small source-compatibility surface remains until `COMP-003C2`:

- non-try `install_*` functions exist only as state-free panic shims;
- `athanor_runtime_defaults::install()` remains deprecated and reaches those shims;
- public no-composition service wrappers remain as deterministic fail-fast shims in production;
- unit-test builds route those wrappers through a local test composition.

These shims do not install, cache or discover dependencies. Applications must not call them.

## COMP-003C2

The final breaking cleanup will:

1. remove `athanor_runtime_defaults::install()`;
2. remove all `install_*` symbols;
3. remove public no-composition service wrappers after the external API change is recorded;
4. require `RuntimeComposition` in Index, Search, Generation, Wiki and remaining service APIs;
5. add full parallel Store/Search/Projector composition-isolation regressions.

## Enforcement

`crates/athanor-app/tests/legacy_factory_migration.rs` verifies that Cargo, CI, Runtime,
Projection, Store, Search and test bootstrap contain no process-global state or legacy feature
wiring. Execution evidence still requires the Rust test and Clippy matrix recorded in
`athanor_implementation_plan_ru.md`.
