---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, generated-artifact, interchange, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level schema id and its current payload shape is protected by a regression fixture.

Audit baseline: `main` at `e2a9bf5ba7742b6d81f087ee5ef0a711a8d1f74f`.

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
| `athanor.docs_check.v1` | `DocsCheckReport` | direct CLI documentation policy check | generation/docs golden |
| `athanor.docs_drift.v1` | `DocsDriftReport` | direct CLI documentation drift report | generation/docs golden |
| `athanor.docs_apply_patch.v1` | `DocsApplyPatchReport` | direct CLI documentation patch application | generation/docs golden |
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

### Context and project registry

`ContextPack` remains the internal domain value. Public JSON boundaries serialize flattened `ContextReport`. Public Project Registry reports and persisted registry state use distinct schema ids; legacy persisted input normalizes on the next mutation.

### Index and generation parity

`IndexReport` owns `athanor.index_report.v1`; direct index/update JSON and daemon index jobs serialize the same document. `GenerationReport` similarly owns `athanor.generation.v1`; daemon generation jobs now serialize the complete report rather than a reduced hand-built object.

The generation parity change preserves the previous daemon generation fields and adds `schema`, `status`, `root`, and `metrics` additively. Golden fixtures and unit regressions compare daemon payloads with direct typed serialization.

### Documentation contracts

`DocsCheckReport`, `DocsDriftReport`, and `DocsApplyPatchReport` are public command-response contracts. `DocsPatchProposal` is different: it is a versioned file exchanged between propose and apply operations, so `athanor.docs_patch.v1` is classified as an interchange artifact instead of a public report owner.

`DocsProposeFixReport` remains a structural blocker because its top-level response contains `{ proposal, path }` without its own schema while the nested proposal is already versioned.

### Specialized RusTok owners

The architecture context has a dedicated owner. FFA, FBA, and Page Builder graph commands use transparent command-specific wrappers over shared internal calculation types.

## Classified non-public schemas

- Persisted: `athanor.project_registry_state.v1`.
- Generated: `athanor.validation_result.v1`, `athanor.generated_generation.v1`, `athanor.generated_current.v1`.
- Embedded: `athanor.index_metrics.v1`, `athanor.index_report_metrics.v1`, `athanor.generation_metrics.v1`.
- Interchange: `athanor.docs_patch.v1`.

The bounded public migration allowlist remains empty.

## Boundaries requiring the next inventory pass

### Remaining application outputs

The next pass covers config doctor/validation, API snapshot/diff/cleanup, wiki, HTML report, repair, and other CLI JSON documents. Known structural issues include unversioned `ApiSnapshotReport`, `WikiReport`, `HtmlReport`, and `DocsProposeFixReport` response wrappers.

### Daemon and MCP envelopes

Daemon request/response/error documents and MCP JSON-RPC/tool-content envelopes must be classified separately from the application reports carried inside them.

### Process-adapter protocols

Extractor, linker, and checker process request/response documents require an explicit inventory of schema ids, Rust owners, framing, and compatibility behavior.

### Persisted and generated state

Index state, publication journals, API snapshots/pointers/diffs, projector manifests, generation pointers, read-model manifests, and repair journals/guards require a repository-wide persisted/generated pass.

## Enforcement implementation

`crates/athanor-app/tests/json_contract_inventory.rs` now scans the identified public read, project, Rustok, benchmark, index, pipeline, changed-validation, generation, and docs modules.

Public, migration, persisted, generated, embedded, and interchange sets are mutually exclusive. Every classified id must remain observable in the bounded source set and cannot simultaneously become a public registry owner.

This remains a bounded enforcement slice until the remaining application, transport, process, persistence, and generated boundaries are classified.

## Enforcement rules

- Every registered schema id and Rust owner is unique.
- The owner implements `VersionedJsonContract` and has a golden regression.
- Equivalent CLI and daemon application results serialize the same typed document.
- Embedded schema-bearing fragments are not registered as top-level documents.
- Generated, persisted, and interchange schemas remain separate from public report contracts.
- Shared calculation types with multiple public schema ids require distinct transparent owner types.
- A schema id must never describe two current emitted top-level shapes.
- Legacy input must normalize to a current schema before writing.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id.
- New schema literals in inventoried modules must be registered or explicitly classified.
