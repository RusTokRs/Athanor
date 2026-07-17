---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, generated-artifact, interchange, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level Athanor schema id and its current payload shape is protected by a regression fixture. Standard and schema-less protocols use separate registries instead of receiving synthetic Athanor schema ids.

Audit baseline: `main` at `0c177cb43c43b0a575045decfb86932e53bc4d08`.

## Registered public contracts

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

The public registry contains 59 current Athanor-schema owners. Public reports, generated artifacts, persisted state, embedded fragments, interchange documents, and transport/process protocols are deliberately separate ownership classes.

## Transport and process protocols

### Daemon

`DaemonRequest` owns current `athanor.daemon_request.v3`; `DaemonResponse` owns current `athanor.daemon_response.v3`; `DaemonJobsReport` owns `athanor.daemon_jobs.v1`. Request v1/v2 compatibility remains accepted input only. `DaemonError`, `DaemonCommand`, and `DaemonJob` are embedded. `DaemonEndpoint` is persisted runtime discovery state: v3 is current, v2 is accepted legacy input, and v1 is historical-only and rejected by the current reader.

### MCP

MCP uses JSON-RPC `2.0` and MCP protocol `2024-11-05`. `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, and text tool-call results remain native standard-protocol boundaries in `MCP_TRANSPORT_CONTRACTS`; they do not receive synthetic `athanor.*` schemas. Text content carries serialized Athanor reports with their own inner schema ids.

### External processes

`PROCESS_PROTOCOL_CONTRACTS` records four intentionally schema-less protocols:

| Protocol | Request | Response | Framing |
| --- | --- | --- | --- |
| `source-discover` | `SourceDiscoverInput` object | `Vec<SourceFile>` array | newline-terminated JSON stdin, one JSON stdout document |
| `extractor` | `ExtractInput` object | `ExtractOutput` object | newline-terminated JSON stdin, one JSON stdout document |
| `linker` | `LinkInput` object | `Vec<Relation>` array | newline-terminated JSON stdin, one JSON stdout document |
| `checker` | `CheckInput` object | `Vec<Diagnostic>` array | newline-terminated JSON stdin, one JSON stdout document |

These wire shapes are the serde representation of core port types. Adding a top-level schema now would change existing external adapters; future incompatible changes require an explicit versioned process envelope rather than silent field mutation.

## Non-public Athanor-schema registry

`NON_PUBLIC_JSON_CONTRACTS` contains 30 descriptors: 24 current documents, five accepted legacy inputs, and one historical-only schema.

### Persisted current

- `athanor.project_registry_state.v1`
- `athanor.daemon_endpoint.v3`
- `athanor.index_current.v2`
- `athanor.index_state.v46` or feature-specific `athanor.index_state.v46-js-ts-precision-v1`
- `athanor.index_publication.v3`
- `athanor.index_current_publication.v1`
- `athanor.canonical_snapshot.v1`
- `athanor.canonical_latest.v1`
- `athanor.canonical_commit.v2`

`IndexState.generation` is derived and required when `snapshot` is present. An empty current state has `snapshot: null` and may omit `generation`; both feature-specific schema alternatives use this conditional rule.

### Generated current

- `athanor.validation_result.v1`
- `athanor.generated_generation.v1`
- `athanor.generated_current.v1`
- `athanor.api_contract_snapshot.v2`
- `athanor.api_contract_latest.v1`
- `athanor.jsonl_manifest.v1`
- `athanor.wiki_manifest.v1`
- `athanor.html_report_manifest.v1`

### Interchange and embedded current

- Interchange: `athanor.docs_patch.v1`, `athanor.wiki_projection.v1`, `athanor.html_report_projection.v1`.
- Embedded: `athanor.index_metrics.v1`, `athanor.index_report_metrics.v1`, `athanor.generation_metrics.v1`, plus schema-less Repair and Daemon fragments.

### Compatibility lifecycle

Accepted legacy input schemas are `athanor.daemon_endpoint.v2`, `athanor.index_current.v1`, `athanor.index_publication.v1`, `athanor.index_publication.v2`, and `athanor.canonical_commit.v1`. They normalize to current state before a current write. `athanor.daemon_endpoint.v1` is retained only as explicit history and is rejected by the current endpoint reader.

Cleanup tombstones, staging directories, backups, locks, and repair guards are filesystem recovery protocols, not JSON documents. Schema-less canonical latest documents from older stores are compatibility input but cannot enter an Athanor schema registry because they have no schema id.

## Enforcement implementation

- `VERSIONED_JSON_CONTRACTS` protects 59 current public/transport Athanor owners.
- `MCP_TRANSPORT_CONTRACTS` protects four native JSON-RPC/MCP boundaries.
- `NON_PUBLIC_JSON_CONTRACTS` protects current, legacy-input, and historical persisted/generated/interchange/embedded schemas.
- `PROCESS_PROTOCOL_CONTRACTS` protects the four schema-less external-process shapes and framing.
- `boundary_contracts.v1.json` provides representative current documents and process request/response fixtures.
- `process_persistence_contract_inventory.rs` verifies disjoint classifications, current fixture coverage, conditional empty index state, endpoint lifecycle, runtime source observability, process type usage, and newline/single-document framing.
- Existing application, daemon, MCP, Repair, publication, checksum, recovery, and projector tests continue to protect semantic validation and atomicity.

The bounded public migration allowlist remains empty. No known implementation inventory gap remains in the audited JSON scope; executable parity and workspace verification remain outstanding.

## Enforcement rules

- Every current public schema has exactly one Rust owner and a regression fixture.
- Non-public and public schema sets are disjoint; all schema ids are canonical and unique within their registry.
- Standard and schema-less protocols retain native framing/version rules instead of receiving synthetic Athanor schema ids.
- Current persisted/generated/interchange documents have representative required-field fixtures.
- Legacy input is never emitted as a current fixture; historical-only schemas are not presented as accepted compatibility.
- Equivalent CLI/application and daemon/MCP inner results serialize the same typed Athanor document.
- A schema id must never describe two current emitted top-level shapes.
- Legacy input normalizes to a current schema before writing or responding.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id or protocol version.
- New schema literals and protocol envelopes must be registered or explicitly classified in the same change.
