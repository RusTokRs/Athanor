---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level schema id and its current payload shape is protected by a regression fixture.

Audit baseline: `main` at `243a7ec059f321f663877fb41e4246da46b5ebb7`.

## Registered contracts

| Schema | Rust owner | Boundary | Fixture status |
| --- | --- | --- | --- |
| `athanor.overview.v1` | `RepositoryOverview` | CLI/daemon/MCP read | dedicated golden |
| `athanor.search.v1` | `SearchReport` | CLI/daemon/MCP read | dedicated golden |
| `athanor.entity_explanation.v1` | `EntityExplanation` | CLI/daemon/MCP read | dedicated golden |
| `athanor.impact_analysis.v1` | `ImpactAnalysis` | CLI/daemon/MCP read | dedicated golden |
| `athanor.diagnostic_check.v1` | `DiagnosticCheckReport` | CLI/daemon/MCP read | dedicated golden |
| `athanor.affected_check.v1` | `AffectedCheckReport` | CLI/daemon/MCP read | dedicated golden |
| `athanor.operations_docs_check.v1` | `OperationsDocsCheckReport` | CLI/daemon/MCP read | dedicated golden |
| `athanor.coverage.v1` | `CoverageReport` | CLI/daemon/MCP read | dedicated golden |
| `athanor.capabilities.v1` | `CapabilitiesReport` | CLI/daemon/MCP read | dedicated golden |
| `athanor.change_map.v1` | `ChangeMapReport` | CLI/daemon/MCP read | second-wave golden |
| `athanor.context_pack.v1` | `ContextReport` | direct CLI/daemon/active MCP read | dedicated golden |
| `athanor.graph_export.v1` | `GraphExport` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_related.v1` | `GraphRelated` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_path.v1` | `GraphPath` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_hubs.v1` | `GraphHubs` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_pagerank.v1` | `GraphPageRank` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_cycles.v1` | `GraphCycles` | CLI/daemon/MCP read | second-wave golden |
| `athanor.rustok_ffa_audit.v1` | `RustokFfaAudit` | direct CLI Rustok audit | representative family golden |
| `athanor.rustok_ffa_surface_graph.v1` | `RustokFfaSurfaceGraphReport` | direct CLI FFA graph | representative family golden |
| `athanor.rustok_ffa_violations_graph.v1` | `RustokFfaViolationsGraphReport` | direct CLI FFA graph | representative family golden |
| `athanor.project_registry.v1` | `ProjectRegistryReport` | CLI project list/add/remove | dedicated golden |
| `athanor.project_resolution.v1` | `ProjectResolutionReport` | CLI/daemon project resolution | second-wave golden |

## Resolved migration decisions

### Context pack

`ContextPack` remains the internal domain value and still carries detailed evidence in its nested `payload`. Public JSON boundaries now serialize `ContextReport`, which adds the required top-level `schema` field and flattens the existing context-pack fields rather than introducing a new `pack` nesting level.

This preserves the established `id`, `task`, `scope`, `level`, `summary`, entity, file, diagnostic, confidence, and payload fields while making `athanor.context_pack.v1` valid under `VersionedJsonContract`.

The registered owner is `ContextReport`. A dedicated golden fixture protects both the new top-level schema and the unchanged nested payload schema. The migration allowlist no longer contains `athanor.context_pack.v1`.

The direct CLI JSON path, cached and operation-aware daemon context paths, and the active lifecycle-based MCP context path now serialize the wrapper. Internal context generation and daemon caching continue to use `ContextPack`.

### Project registry state and report

The public `ProjectRegistryReport` keeps the existing `athanor.project_registry.v1` identifier, so CLI consumers do not receive a schema-id change. The persisted `ProjectRegistry` document now emits the distinct internal identifier `athanor.project_registry_state.v1`.

Existing `projects.json` files that used `athanor.project_registry.v1` are accepted as legacy persisted input throughout the v1 compatibility window. Loading normalizes the in-memory state to `athanor.project_registry_state.v1`; the next add or remove operation atomically rewrites the file with the current state schema. Read-only list and resolve operations do not rewrite the file.

Removing legacy persisted-state acceptance requires a new major compatibility decision. `PROJECT_REGISTRY_SCHEMA` remains as a deprecated Rust alias for the public report schema, while new code must choose `PROJECT_REGISTRY_REPORT_SCHEMA` or `PROJECT_REGISTRY_STATE_SCHEMA` explicitly.

`ProjectRegistryReport` is the unique registered owner of `athanor.project_registry.v1` and is protected by a dedicated golden fixture. The persisted state schema is intentionally classified outside `VERSIONED_JSON_CONTRACTS`, and unit regressions cover current writes, legacy reads, migration-on-mutation, and unknown-schema rejection.

### RusTok FFA graph owners

The surface and violations commands share the internal calculation type `RustokFfaGraph`, but one Rust owner cannot implement two associated schema constants. Public operation-aware APIs now return transparent `RustokFfaSurfaceGraphReport` and `RustokFfaViolationsGraphReport` wrappers.

The wrappers reset the internal graph schema to the command-specific constant, serialize transparently without a new nesting level, and dereference to `RustokFfaGraph` for existing text renderers. `RustokFfaAudit` remains a unique direct owner. One representative fixture protects a non-empty audit surface plus non-empty surface and violations graph nodes and edges.

## Discovered contracts requiring migration decisions

### Remaining specialized graph and Rustok reports

FBA currently shares `RustokFbaGraph` across module, port, dependencies, and violations schema ids. Page Builder similarly shares `RustokPageBuilderGraph` across provider, consumer, and violations ids. They require the same unique transparent-owner treatment used for FFA before registry inclusion.

### Process-adapter protocols and persisted state

Extractor/linker/checker process payloads, daemon envelopes, MCP envelopes, index state, publication journals, generation pointers, and read-model manifests require a separate inventory pass. Internal persistence documents must not be mixed with public report schemas merely because both serialize as JSON.

`athanor.project_registry_state.v1` is the first explicitly classified persisted-state schema in the bounded source scanner. The broader persistence inventory remains pending.

## Shared-constant migration

The registered Search, Impact, Diagnostic Check, Affected Check, Operations Docs Check, and ChangeMap builders now import their schema ids from `json_contract` instead of embedding quoted schema literals. Impact covers both the normal report path and the empty-diff early return.

`json_contract_inventory.rs` protects all six migrated schema ids with a source-level regression: each schema must remain registered and its quoted literal must not reappear in the owner source. Unit assertions for Check and ChangeMap also use the shared constants.

## Enforcement implementation

`crates/athanor-app/tests/json_contract_inventory.rs` scans the currently identified app-layer owner modules. Every canonical schema literal found there must be registered, tracked as an unregistered public migration item, or classified as an internal persisted schema.

The public migration allowlist now contains only FBA and Page Builder graph/audit contracts. The persisted classification currently contains `athanor.project_registry_state.v1`. Tests reject stale migration exceptions, public registration of internal persisted ids, duplicate classifications, and classified ids that disappear from the inventoried source set.

This is a bounded first enforcement slice. Daemon/MCP envelopes, process protocols, persistence documents, and newly discovered source modules must be added after their inventory classification is complete.

## Enforcement rules

- Every registered schema id is valid and unique.
- Every registered Rust owner is unique.
- The owner implements `VersionedJsonContract`.
- A golden regression exercises serialization and `validate_contract()`.
- Shared internal calculation types with multiple public schema ids require distinct transparent public owner types.
- Migrated builders must import their schema id from the shared registry module and must not embed the quoted literal.
- Local schema constants must equal the shared registry constant until literals are fully migrated.
- A schema id must never describe two current emitted top-level shapes.
- Legacy input may be accepted only through a documented compatibility path that normalizes to a current schema before writing.
- Internal persisted schemas must be classified separately from public report contracts.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id.
- New schema literals in inventoried modules must be registered or explicitly classified.
