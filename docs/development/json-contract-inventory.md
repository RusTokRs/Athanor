---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, generated-artifact, interchange, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level Athanor schema id and its current payload shape is protected by a regression fixture. Standard protocol envelopes use a separate protocol registry instead of receiving synthetic Athanor schema ids.

Audit baseline: `main` at `216aac85487abb22c8df1dd4a0e1a822d02f8307`.

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
| `athanor.daemon_request.v3` | `DaemonRequest` | local daemon transport request | daemon transport golden |
| `athanor.daemon_response.v3` | `DaemonResponse` | local daemon transport success/error envelope | daemon transport golden |
| `athanor.daemon_jobs.v1` | `DaemonJobsReport` | daemon jobs result payload | daemon transport golden |
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

### Application and Repair reports

Application reports, generated artifacts, persisted state, embedded fragments, and interchange files remain distinct ownership classes. Additive wrappers preserve legacy fields where a top-level schema was missing. Nine Repair reports retain their existing payloads and current schemas; their state, issue, row, and tombstone components remain embedded or filesystem protocol fragments.

### Daemon transport

`DaemonRequest` owns current `athanor.daemon_request.v3`; `DaemonResponse` owns current `athanor.daemon_response.v3`; `DaemonJobsReport` owns `athanor.daemon_jobs.v1`. Request v1/v2 compatibility remains accepted input only and is not registered as current ownership. Historical response v2 is likewise not current.

`DaemonError`, `DaemonCommand`, and `DaemonJob` are embedded parts of the current envelopes. `DaemonEndpoint` is a persisted runtime-discovery descriptor under `athanor.daemon_endpoint.v3`, not a public response owner. Endpoint/request compatibility constants for earlier protocol generations remain migration history.

### MCP transport

MCP uses JSON-RPC `2.0` and MCP protocol `2024-11-05`. `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, and the text tool-call result are standard-protocol boundaries, not Athanor-schema documents. They are recorded in `MCP_TRANSPORT_CONTRACTS`, validated by fail-closed helpers, and protected by `mcp_transport_contracts.v1.json` plus the existing runtime unit tests around `handle_line`.

The content text contains serialized Athanor application reports. Those inner reports retain their own registered `athanor.*` schema ids; the outer JSON-RPC and tool-content envelopes must not acquire synthetic Athanor schema ids.

### Generated, persisted, and interchange documents

`DocsPatchProposal` is interchange. API snapshots/latest pointers and generation manifests/current pointers are generated. Project Registry state, index-current publication journal, and daemon endpoint are persisted. The remaining pointer/manifest inventory is the next implementation package.

## Classified non-public schemas and protocols

- Persisted: `athanor.project_registry_state.v1`, `athanor.index_current_publication.v1`, `athanor.daemon_endpoint.v3`.
- Generated: `athanor.validation_result.v1`, `athanor.generated_generation.v1`, `athanor.generated_current.v1`, `athanor.api_contract_snapshot.v2`, `athanor.api_contract_latest.v1`.
- Embedded: `athanor.index_metrics.v1`, `athanor.index_report_metrics.v1`, `athanor.generation_metrics.v1`, plus schema-less Repair and Daemon fragments.
- Interchange: `athanor.docs_patch.v1`.
- Standard protocol: JSON-RPC `2.0` and MCP `2024-11-05` envelopes registered separately in `MCP_TRANSPORT_CONTRACTS`.
- Legacy daemon input: request v1/v2; historical response/endpoint schemas are compatibility history, not current owners.

The bounded public migration allowlist remains empty.

## Boundaries requiring the next inventory pass

### Process-adapter protocols

Extractor, linker, and checker process request/response documents require an explicit inventory of schema ids, Rust owners, framing, and compatibility behavior.

### Persisted and generated state

Index-current pointers, index state, publication journals, projector payloads/manifests, read-model manifests, repair guards, generated/canonical pointers, and remaining persisted documents require repository-wide enforcement.

## Enforcement implementation

The shared Athanor registry now contains 59 current owners. `daemon_contract.rs` implements ownership for current daemon request, response, and jobs documents. `daemon_transport_contracts.rs` and its fixture protect current shapes and reject legacy or embedded ownership.

The MCP crate exposes a separate four-entry standard-protocol registry and validators. Its integration fixture covers request, success response, error response, initialize result, and success/error tool content. Source assertions ensure the actual legacy runtime still emits the inventoried JSON-RPC/MCP versions and does not introduce an `athanor.mcp_*` schema.

Public, migration, persisted, generated, embedded, interchange, and standard-protocol sets remain mutually exclusive.

## Enforcement rules

- Every registered schema id and Rust owner is unique.
- Current Athanor-schema owners implement `VersionedJsonContract` and have a golden regression.
- Standard protocol envelopes retain their native version field instead of receiving synthetic Athanor schema ids.
- Equivalent CLI/application and daemon/MCP inner results serialize the same typed Athanor document.
- Embedded schema-bearing fragments are not registered as top-level documents.
- Generated, persisted, and interchange schemas remain separate from public report contracts.
- A schema id must never describe two current emitted top-level shapes.
- Legacy input must normalize to a current schema before writing or responding.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id or protocol version.
- New schema literals and protocol envelopes must be registered or explicitly classified.
