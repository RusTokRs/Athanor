---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, generated-artifact, interchange, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level schema id and its current payload shape is protected by a regression fixture.

Audit baseline: `main` at `bd740f52f128b1f7ca3fc98b7ae348f98826bab0`.

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
| `athanor.repair_inspect.v2` | `RepairInspectReport` | direct CLI repair inspection | Repair family golden |
| `athanor.repair_cleanup.v2` | `RepairCleanupReport` | direct CLI legacy artifact cleanup | Repair family golden |
| `athanor.repair_regenerate.v1` | `RepairRegenerateReport` | direct CLI generated-output repair | Repair family golden |
| `athanor.repair_recover_canonical.v1` | `RepairRecoverCanonicalReport` | direct CLI canonical pointer recovery | Repair family golden |
| `athanor.repair_apply.v2` | `RepairApplyReport` | direct CLI coordinated repair | Repair family golden |
| `athanor.index_generation_cleanup.v1` | `IndexGenerationCleanupReport` | direct CLI transactional index retention | Repair family golden plus executable CLI regression |
| `athanor.repair_recover_index.v1` | `RepairRecoverIndexReport` | direct CLI publication recovery | Repair family golden plus executable CLI regression |
| `athanor.repair_recover_index_cleanup.v1` | `RepairRecoverIndexCleanupReport` | direct CLI cleanup-tombstone recovery | Repair family golden plus executable CLI regression |
| `athanor.repair_canonical_latest.v1` | `RepairCanonicalLatestReport` | direct CLI backend latest repair | Repair family golden plus CLI help regression |
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

`ConfigValidateReport` owns `athanor.config_validate.v1`; its effective config remains flattened. `ConfigDoctorReport` owns `athanor.config_doctor.v1` and preserves the existing `{ schema, root, config, checks }` shape. Direct CLI dispatch uses both typed owners.

### Documentation contracts

`DocsCheckReport`, `DocsDriftReport`, and `DocsApplyPatchReport` remain direct public owners. `DocsPatchProposal` remains a versioned interchange file under `athanor.docs_patch.v1`. `VersionedDocsProposeFixReport` owns `athanor.docs_propose_fix.v1`, adding only top-level `schema` around the existing flattened summary.

### API contracts

`VersionedApiSnapshotReport` owns `athanor.api_snapshot.v1`, while generated immutable snapshots and latest pointers remain independent generated v2/v1 documents. `ApiContractDiff` and `ApiCleanupReport` retain their established public ownership.

### Repair reports and state

Nine Repair report types are public top-level contracts. Existing CLI payloads already carried schemas, so registration adds ownership and validation without changing their shapes. Current public Inspect, Cleanup, and Apply reports use v2; Regenerate, canonical recovery, index recovery, cleanup recovery, index retention, and backend latest repair retain v1.

`CanonicalRepairState`, `GeneratedRepairState`, `RepairIssue`, cleanup removal/retained rows, index cleanup rows, and cleanup tombstones are embedded fragments. They must not become independent registry owners.

The source chain still contains historical internal v1 Inspect/Cleanup/Apply constructors. Public facade layers normalize those reports to the current v2 schema before returning them; the historical intermediate implementations are not external contracts.

`athanor.index_current_publication.v1` is a persisted recovery journal, not a public report. Index-current pointers, immutable index state, read-model manifests, generated pointers, canonical manifests, and backend latest documents remain in the persisted/generated inventory package. Cleanup tombstones are a filesystem recovery protocol rather than JSON documents.

### Wiki and HTML reports

`WikiReport` and `HtmlReport` own distinct public application schemas. Daemon jobs serialize the complete typed reports. Projector input documents remain separate generated boundaries.

### Specialized RusTok owners

The architecture context has a dedicated owner. FFA, FBA, and Page Builder graph commands use transparent command-specific wrappers over shared internal calculation types.

## Classified non-public schemas

- Persisted: `athanor.project_registry_state.v1`, `athanor.index_current_publication.v1`.
- Generated: `athanor.validation_result.v1`, `athanor.generated_generation.v1`, `athanor.generated_current.v1`, `athanor.api_contract_snapshot.v2`, `athanor.api_contract_latest.v1`.
- Embedded: `athanor.index_metrics.v1`, `athanor.index_report_metrics.v1`, `athanor.generation_metrics.v1`, plus schema-less Repair state/row/tombstone fragments.
- Interchange: `athanor.docs_patch.v1`.

The bounded public migration allowlist remains empty.

## Boundaries requiring the next inventory pass

### Remaining application outputs

No known application-output blocker remains in the audited scope. New application reports must enter the shared registry with a fixture in the same change.

### Daemon and MCP envelopes

Daemon request/response/error documents and MCP JSON-RPC/tool-content envelopes must be classified separately from the application reports carried inside them.

### Process-adapter protocols

Extractor, linker, and checker process request/response documents require an explicit inventory of schema ids, Rust owners, framing, and compatibility behavior.

### Persisted and generated state

Index-current pointers, index state, publication journals, projector payloads/manifests, read-model manifests, repair guards, generated/canonical pointers, and remaining persisted documents require repository-wide enforcement.

## Enforcement implementation

The public facade registry now contains 56 owners. `repair_contract.rs` implements shared ownership for all nine Repair report types. `repair_contracts.rs` and its golden fixture protect the complete serialized family. `repair_contract_inventory.rs` verifies all public owners and fails if embedded repair types enter the registry. Existing executable Repair regressions protect transactional command schemas and failure behavior.

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
