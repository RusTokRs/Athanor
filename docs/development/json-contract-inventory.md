---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, generated-artifact, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level schema id and its current payload shape is protected by a regression fixture.

Audit baseline: `main` at `1fa64eafe6b609c7655c7a6c08622be5ae40a1eb`.

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
| `athanor.index_report.v1` | `IndexReport` | direct CLI index/update and daemon index job result | application-output golden plus daemon parity regression |
| `athanor.index_benchmark.v1` | `BenchmarkReport` | direct CLI benchmark output | application-output golden |
| `athanor.changed_validation.v1` | `ChangedValidationReport` | direct CLI changed-file validation | application-output golden |
| `athanor.graph_export.v1` | `GraphExport` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_related.v1` | `GraphRelated` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_path.v1` | `GraphPath` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_hubs.v1` | `GraphHubs` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_pagerank.v1` | `GraphPageRank` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_cycles.v1` | `GraphCycles` | CLI/daemon/MCP read | second-wave golden |
| `athanor.rustok_architecture_context.v1` | `RustokArchitectureContext` | direct CLI/active MCP read | dedicated golden |
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

`ContextPack` remains the internal domain value. Public JSON boundaries serialize `ContextReport`, which adds the required top-level `schema` field and flattens the existing fields without a new `pack` nesting level.

### Project registry state and report

The public `ProjectRegistryReport` keeps `athanor.project_registry.v1`. Persisted state emits `athanor.project_registry_state.v1`; legacy persisted v1 input remains accepted during the compatibility window and migrates on the next mutation.

### Specialized RusTok owners

The architecture context has a dedicated owner and non-empty fixture. FFA, FBA, and Page Builder graph commands use transparent command-specific wrappers over shared internal calculation types. The wrappers preserve JSON shape and text-renderer access while ensuring one Rust owner per schema id.

### Index result parity

`IndexReport` owns `athanor.index_report.v1`. A custom serializer prepends the top-level `schema` field while preserving every established direct-CLI field: root, snapshot, file counts, output and validation paths, validate-only state, and nested metrics.

Direct `ath index --json` and `ath update --json` already serialize `IndexReport`, so they receive the versioned shape without call-site wrappers. Daemon index jobs now serialize the same `IndexReport` instead of constructing a reduced object. Existing daemon fields remain present and the previously omitted CLI fields are additive.

The application-output golden fixture validates `IndexReport` directly and protects its nested metrics. A daemon unit regression asserts that the stored job result equals direct `IndexReport` serialization.

### Application output classification

`BenchmarkReport` and `ChangedValidationReport` carry valid top-level schema ids and are registered as public CLI documents. The benchmark fixture contains the versioned nested index report.

`IndexPipelineMetrics` (`athanor.index_metrics.v1`) and `IndexReportMetrics` (`athanor.index_report_metrics.v1`) are embedded fragments rather than independent top-level documents. They are explicitly classified outside `VERSIONED_JSON_CONTRACTS`.

`athanor.validation_result.v1` is a generated validation artifact written by validate-only indexing. It is classified as generated state, not as a public report contract.

The bounded public migration allowlist is empty.

## Boundaries requiring the next inventory pass

### Remaining application outputs

The next application pass inventories config doctor/validation, API, docs, generation, wiki, HTML report, repair, and other CLI JSON documents outside the current scanner.

### Daemon and MCP envelopes

Daemon request/response/error documents and MCP JSON-RPC/tool-content envelopes must be classified separately from the application reports carried inside them.

### Process-adapter protocols

Extractor, linker, and checker process request/response documents require an explicit inventory of schema ids, Rust owners, framing, and compatibility behavior.

### Persisted and generated state

Index state, publication journals, generation pointers, read-model manifests, and other internal JSON documents require separate persisted/generated classifications. Current explicit classifications are `athanor.project_registry_state.v1` as persisted state and `athanor.validation_result.v1` as generated state.

## Enforcement implementation

`crates/athanor-app/tests/json_contract_inventory.rs` scans identified public read, project, Rustok, benchmark, index, pipeline-metrics, and changed-validation owner modules. Every canonical schema literal found there must be registered or classified as public migration, persisted, generated, or embedded.

The classifications are mutually exclusive and checked against observed source literals. The public migration set is empty; persisted contains `project_registry_state.v1`; generated contains `validation_result.v1`; embedded contains `index_metrics.v1` and `index_report_metrics.v1`.

This remains a bounded enforcement slice until the remaining application, transport, process, persistence, and generated boundaries are classified.

## Enforcement rules

- Every registered schema id and Rust owner is unique.
- The owner implements `VersionedJsonContract` and has a golden regression.
- Equivalent CLI and daemon application results serialize the same typed document.
- Embedded schema-bearing fragments are not registered as top-level documents.
- Generated and persisted schemas remain separate from public report contracts.
- Shared calculation types with multiple public schema ids require distinct transparent owner types.
- A schema id must never describe two current emitted top-level shapes.
- Legacy input must normalize to a current schema before writing.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id.
- New schema literals in inventoried modules must be registered or explicitly classified.
