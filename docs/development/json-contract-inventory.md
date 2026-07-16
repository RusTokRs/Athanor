---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level schema id and its current payload shape is protected by a regression fixture.

Audit baseline: `main` at `cf8cb1f481997bde71a892d2ffefdc820daa252b`.

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
| `athanor.rustok_fba_audit.v1` | `RustokFbaAudit` | direct CLI Rustok audit | representative family golden |
| `athanor.rustok_fba_module_graph.v1` | `RustokFbaModuleGraphReport` | direct CLI FBA graph | representative family golden |
| `athanor.rustok_fba_port_graph.v1` | `RustokFbaPortGraphReport` | direct CLI FBA graph | representative family golden |
| `athanor.rustok_fba_dependencies_graph.v1` | `RustokFbaDependenciesGraphReport` | direct CLI FBA graph | representative family golden |
| `athanor.rustok_fba_violations_graph.v1` | `RustokFbaViolationsGraphReport` | direct CLI FBA graph | representative family golden |
| `athanor.rustok_page_builder_audit.v1` | `RustokPageBuilderAudit` | direct CLI Rustok audit | representative family golden |
| `athanor.rustok_page_builder_provider_graph.v1` | `RustokPageBuilderProviderGraphReport` | direct CLI Page Builder graph | representative family golden |
| `athanor.rustok_page_builder_consumer_graph.v1` | `RustokPageBuilderConsumerGraphReport` | direct CLI Page Builder graph | representative family golden |
| `athanor.rustok_page_builder_violations_graph.v1` | `RustokPageBuilderViolationsGraphReport` | direct CLI Page Builder graph | representative family golden |
| `athanor.project_registry.v1` | `ProjectRegistryReport` | CLI project list/add/remove | dedicated golden |
| `athanor.project_resolution.v1` | `ProjectResolutionReport` | CLI/daemon project resolution | second-wave golden |

## Resolved migration decisions

### Context pack

`ContextPack` remains the internal domain value and still carries detailed evidence in its nested `payload`. Public JSON boundaries serialize `ContextReport`, which adds the required top-level `schema` field and flattens the existing context-pack fields rather than introducing a new `pack` nesting level.

The direct CLI JSON path, cached and operation-aware daemon context paths, and the active lifecycle-based MCP context path serialize the wrapper. Internal context generation and daemon caching continue to use `ContextPack`.

### Project registry state and report

The public `ProjectRegistryReport` keeps the existing `athanor.project_registry.v1` identifier. The persisted `ProjectRegistry` document emits the distinct internal identifier `athanor.project_registry_state.v1`.

Existing `projects.json` files that used `athanor.project_registry.v1` are accepted as legacy persisted input throughout the v1 compatibility window. Loading normalizes the in-memory state to `athanor.project_registry_state.v1`; the next add or remove operation atomically rewrites the file with the current state schema. Read-only list and resolve operations do not rewrite the file.

### Specialized RusTok graph owners

FFA, FBA, and Page Builder each use shared internal graph calculation types across multiple command-specific schema ids. A single Rust type cannot own several associated schema constants, so every command-specific graph now has a transparent public wrapper.

The wrappers reset the internal graph schema to the command-specific constant, serialize without a new nesting level, and implement `Deref`/`AsRef` for existing text renderers. Operation-aware APIs wrap the graph only after the cooperative worker completes, leaving cancellation and deadline behavior unchanged.

Representative non-empty fixtures cover:

- FFA audit plus surface and violations graph nodes and edges;
- FBA audit plus module, port, dependencies, and violations graphs;
- Page Builder audit plus provider, consumer, and violations graphs.

The app-layer public migration allowlist is now empty.

## Boundaries requiring the next inventory pass

### Daemon and MCP envelopes

Daemon request/response/error documents and MCP JSON-RPC/tool-content envelopes must be classified separately from the public application reports carried inside their payload fields. Transport envelopes may have independent versioning and compatibility rules.

### Process-adapter protocols

Extractor, linker, and checker process request/response documents require an explicit inventory of their schema ids, Rust owners, subprocess framing, and compatibility behavior.

### Persisted and generated state

Index state, publication journals, generation pointers, read-model manifests, and other internal JSON documents require separate persisted/generated classifications. Internal persistence documents must not be mixed into the public report registry merely because both serialize as JSON.

`athanor.project_registry_state.v1` is the first explicitly classified persisted-state schema in the bounded source scanner.

## Enforcement implementation

`crates/athanor-app/tests/json_contract_inventory.rs` scans the currently identified app-layer owner modules. Every canonical schema literal found there must be registered, tracked as an unregistered public migration item, or classified as an internal persisted schema.

The public migration allowlist is empty. The persisted classification currently contains `athanor.project_registry_state.v1`. Tests reject stale migration exceptions, public registration of internal persisted ids, duplicate classifications, and classified ids that disappear from the inventoried source set.

This is a bounded first enforcement slice. Daemon/MCP envelopes, process protocols, persistence documents, and newly discovered source modules must be added after their inventory classification is complete.

## Enforcement rules

- Every registered schema id is valid and unique.
- Every registered Rust owner is unique.
- The owner implements `VersionedJsonContract`.
- A golden regression exercises serialization and `validate_contract()`.
- Shared internal calculation types with multiple public schema ids require distinct transparent public owner types.
- Migrated builders must import their schema id from the shared registry module and must not embed the quoted literal.
- A schema id must never describe two current emitted top-level shapes.
- Legacy input may be accepted only through a documented compatibility path that normalizes to a current schema before writing.
- Internal persisted schemas must be classified separately from public report contracts.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id.
- New schema literals in inventoried modules must be registered or explicitly classified.
