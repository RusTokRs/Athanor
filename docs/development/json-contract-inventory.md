---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, generated-artifact, interchange, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level schema id and its current payload shape is protected by a regression fixture.

Audit baseline: `main` at `fb2f30e2004765ba01bd28ea5c32860e192915fb`.

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

The direct CLI dispatcher now routes both `ath config validate` and `ath config doctor` through the typed application functions. Human-readable output remains compatible: validation still prints the raw effective configuration after the success line, and doctor still prints the three established check statuses. `apps/ath/tests/direct_config_cli.rs` protects the executable JSON shapes, help surface, and strict unknown-field failure path. Execution of that regression remains part of the pending workspace verification pass.

### Documentation contracts

`DocsCheckReport`, `DocsDriftReport`, and `DocsApplyPatchReport` are public command-response contracts. `DocsPatchProposal` is a versioned file exchanged between propose and apply, so `athanor.docs_patch.v1` is classified as interchange instead of public report ownership.

`DocsProposeFixReport` remains a structural blocker because its top-level `{ proposal, path }` wrapper has no schema while the nested proposal is already versioned.

### API contracts

`ApiContractDiff` owns `athanor.api_contract_diff.v2` for both the direct command result and persisted diff artifact. `ApiCleanupReport` owns `athanor.api_cleanup.v1`. Generated snapshots and latest pointers remain outside the public registry but are protected by the API golden fixture.

`ApiSnapshotReport` remains a migration blocker: the command returns an unversioned summary wrapper rather than the generated snapshot document itself.

### Wiki and HTML reports

`WikiReport` and `HtmlReport` now own distinct public application schemas. Their app-layer constructors set the schema, and daemon jobs serialize the complete typed report instead of assembling reduced objects. Existing daemon fields remain present; `schema` and `root` are additive.

The projector input documents remain separate generated boundaries: `athanor.wiki_projection.v1` and `athanor.html_report_projection.v1` describe projection payloads, not operation reports. They will be classified with projector manifests during the persisted/generated pass.

### Specialized RusTok owners

The architecture context has a dedicated owner. FFA, FBA, and Page Builder graph commands use transparent command-specific wrappers over shared internal calculation types.

## Classified non-public schemas

- Persisted: `athanor.project_registry_state.v1`.
- Generated: `athanor.validation_result.v1`, `athanor.generated_generation.v1`, `athanor.generated_current.v1`, `athanor.api_contract_snapshot.v2`, `athanor.api_contract_latest.v1`.
- Embedded: `athanor.index_metrics.v1`, `athanor.index_report_metrics.v1`, `athanor.generation_metrics.v1`.
- Interchange: `athanor.docs_patch.v1`.

The bounded public migration allowlist remains empty because known unversioned wrappers are tracked by type and boundary rather than by a schema id that does not yet exist.

## Boundaries requiring the next inventory pass

### Remaining application outputs

The remaining application pass covers versioned wrappers for `ApiSnapshotReport` and `DocsProposeFixReport`, plus repair JSON reports/state.

### Daemon and MCP envelopes

Daemon request/response/error documents and MCP JSON-RPC/tool-content envelopes must be classified separately from the application reports carried inside them.

### Process-adapter protocols

Extractor, linker, and checker process request/response documents require an explicit inventory of schema ids, Rust owners, framing, and compatibility behavior.

### Persisted and generated state

Index state, publication journals, projector payloads/manifests, read-model manifests, repair journals/guards, and remaining pointer documents require a repository-wide persisted/generated pass.

## Enforcement implementation

`crates/athanor-app/tests/json_contract_inventory.rs` scans the identified public application owner modules, including `config.rs`, `wiki.rs`, and `report.rs`. Registered Config, Wiki, and HTML ids are therefore enforced from their source owner modules. The executable Config CLI regression additionally verifies that the production entry dispatcher reaches the registered owners instead of the legacy ad-hoc branch.

Public, migration, persisted, generated, embedded, and interchange sets are mutually exclusive. Every classified id must remain observable in the bounded source set and cannot simultaneously become a public registry owner.

## Enforcement rules

- Every registered schema id and Rust owner is unique.
- The owner implements `VersionedJsonContract` and has a golden regression.
- Equivalent CLI/application and daemon results serialize the same typed document.
- Embedded schema-bearing fragments are not registered as top-level documents.
- Generated, persisted, and interchange schemas remain separate from public report contracts.
- One type may serve a public response and generated artifact only when both boundaries emit the same top-level shape.
- A schema id must never describe two current emitted top-level shapes.
- Legacy input must normalize to a current schema before writing.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id.
- New schema literals in inventoried modules must be registered or explicitly classified.
