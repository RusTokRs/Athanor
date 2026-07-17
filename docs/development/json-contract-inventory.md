---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, generated-artifact, interchange, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level schema id and its current payload shape is protected by a regression fixture.

Audit baseline: `main` at `92d15376b6e511433d512728720f180c936981f7`.

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
| `athanor.generation.v1` | `GenerationReport` | direct CLI generation and daemon generation job result | generation/docs golden plus daemon parity regression |
| `athanor.config_validate.v1` | `ConfigValidateReport` | direct CLI config validation | config golden plus executable CLI regression |
| `athanor.config_doctor.v1` | `ConfigDoctorReport` | direct CLI config diagnostics | config golden plus executable CLI regression |
| `athanor.docs_check.v1` | `DocsCheckReport` | direct CLI documentation policy check | generation/docs golden |
| `athanor.docs_drift.v1` | `DocsDriftReport` | direct CLI documentation drift report | generation/docs golden |
| `athanor.docs_apply_patch.v1` | `DocsApplyPatchReport` | direct CLI documentation patch application | generation/docs golden |
| `athanor.docs_propose_fix.v1` | `VersionedDocsProposeFixReport` | direct CLI documentation proposal summary | remaining-application golden plus direct help regression |
| `athanor.api_snapshot.v1` | `VersionedApiSnapshotReport` | direct CLI API snapshot summary | remaining-application golden plus direct help regression |
| `athanor.api_contract_diff.v2` | `ApiContractDiff` | direct CLI diff plus persisted diff artifact | API golden |
| `athanor.api_cleanup.v1` | `ApiCleanupReport` | direct CLI API retention cleanup | API golden |
| `athanor.wiki_report.v1` | `WikiReport` | application result and daemon Wiki job result | Wiki/HTML golden plus daemon parity regression |
| `athanor.html_report.v1` | `HtmlReport` | application result and daemon HTML job result | Wiki/HTML golden plus daemon parity regression |
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

### Context, indexing, generation, and registry

`ContextPack` remains the internal domain value and is exposed through flattened `ContextReport`. Direct CLI and daemon Index and Generation operations serialize the same typed reports. Public Project Registry reports and persisted registry state use distinct schema ids; legacy state normalizes on mutation.

### Config reports

`ConfigValidateReport` owns `athanor.config_validate.v1`. Its effective `ProjectConfig` remains flattened at the top level, while `schema` and `root` are additive fields. `ConfigDoctorReport` owns `athanor.config_doctor.v1` and preserves the existing `{ schema, root, config, checks }` shape with typed check entries.

The direct CLI dispatcher routes both Config commands through the typed application functions. Human-readable output remains compatible. `apps/ath/tests/direct_config_cli.rs` protects the executable JSON shapes, help surface, and strict unknown-field failure path.

### Documentation contracts

`DocsCheckReport`, `DocsDriftReport`, and `DocsApplyPatchReport` remain direct public report owners. `DocsPatchProposal` remains a versioned interchange file under `athanor.docs_patch.v1`.

`VersionedDocsProposeFixReport` now owns `athanor.docs_propose_fix.v1`. It adds only the top-level `schema` field and flattens the existing `{ proposal, path }` summary, so the proposal document and output path remain unchanged. The production entry dispatcher routes `ath docs propose-fix` through this wrapper.

### API contracts

`ApiContractDiff` owns `athanor.api_contract_diff.v2` for both the direct command result and persisted diff artifact. `ApiCleanupReport` owns `athanor.api_cleanup.v1`. Generated contract snapshots and latest pointers remain separate generated documents under `athanor.api_contract_snapshot.v2` and `athanor.api_contract_latest.v1`.

`VersionedApiSnapshotReport` now owns `athanor.api_snapshot.v1`. It adds only top-level `schema` and flattens the previous snapshot summary fields. `ath api snapshot` uses this wrapper while the generated immutable snapshot retains its independent v2 schema.

### Wiki and HTML reports

`WikiReport` and `HtmlReport` own distinct public application schemas. App constructors set the schema, and daemon jobs serialize the complete typed report. Projector input documents remain separate generated boundaries.

### Specialized RusTok owners

The architecture context has a dedicated owner. FFA, FBA, and Page Builder graph commands use transparent command-specific wrappers over shared internal calculation types.

## Classified non-public schemas

- Persisted: `athanor.project_registry_state.v1`.
- Generated: `athanor.validation_result.v1`, `athanor.generated_generation.v1`, `athanor.generated_current.v1`, `athanor.api_contract_snapshot.v2`, `athanor.api_contract_latest.v1`.
- Embedded: `athanor.index_metrics.v1`, `athanor.index_report_metrics.v1`, `athanor.generation_metrics.v1`.
- Interchange: `athanor.docs_patch.v1`.

The bounded public migration allowlist remains empty.

## Boundaries requiring the next inventory pass

### Remaining application outputs

The remaining application pass covers Repair JSON reports and state. API Snapshot and Docs Propose Fix wrappers are no longer blockers.

### Daemon and MCP envelopes

Daemon request/response/error documents and MCP JSON-RPC/tool-content envelopes must be classified separately from the application reports carried inside them.

### Process-adapter protocols

Extractor, linker, and checker process request/response documents require an explicit inventory of schema ids, Rust owners, framing, and compatibility behavior.

### Persisted and generated state

Index state, publication journals, projector payloads/manifests, read-model manifests, repair journals/guards, and remaining pointer documents require a repository-wide persisted/generated pass.

## Enforcement implementation

The legacy validation and owner implementations live in `json_contract_base.rs`; `json_contract.rs` is now the public facade and extends the canonical registry to 47 owners. `remaining_application_contracts.rs` and its golden fixture protect both additive wrappers. `remaining_application_contract_inventory.rs` verifies their registry ownership and schema literals. The production entry dispatcher routes only `api snapshot` and `docs propose-fix` through the new direct module, leaving unrelated API and Docs commands on the established dispatcher.

Public, migration, persisted, generated, embedded, and interchange sets remain mutually exclusive.

## Enforcement rules

- Every registered schema id and Rust owner is unique.
- The owner implements `VersionedJsonContract` and has a golden regression.
- Equivalent CLI/application and daemon results serialize the same typed document.
- Embedded schema-bearing fragments are not registered as top-level documents.
- Generated, persisted, and interchange schemas remain separate from public report contracts.
- A schema id must never describe two current emitted top-level shapes.
- Legacy input must normalize to a current schema before writing.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id.
- New schema literals must be registered or explicitly classified.
