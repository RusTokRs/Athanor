---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level schema id and its current payload shape is protected by a regression fixture.

Audit baseline: `main` at `9cb7dec05eb268f1113b38bd11da02389832ef0d`.

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
| `athanor.graph_export.v1` | `GraphExport` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_related.v1` | `GraphRelated` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_path.v1` | `GraphPath` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_hubs.v1` | `GraphHubs` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_pagerank.v1` | `GraphPageRank` | CLI/daemon/MCP read | second-wave golden |
| `athanor.graph_cycles.v1` | `GraphCycles` | CLI/daemon/MCP read | second-wave golden |
| `athanor.project_resolution.v1` | `ProjectResolutionReport` | CLI project resolution | second-wave golden |

## Discovered contracts requiring migration decisions

### Context pack

`ContextPack` has no top-level `schema` field. The identifier `athanor.context_pack.v1` is nested inside its arbitrary `payload` value. It cannot implement `VersionedJsonContract` without one of these explicit decisions:

1. introduce a typed top-level context response wrapper;
2. introduce a separate nested-payload contract abstraction;
3. promote the schema field into the domain type in a versioned compatibility change.

Do not register `ContextPack` by bypassing top-level validation.

### Project registry schema collision

`ProjectRegistry` and `ProjectRegistryReport` both emit `athanor.project_registry.v1`, but their shapes differ: the report adds `registry_path`. One schema id therefore describes two Rust document shapes.

Required follow-up:

- designate separate persisted-state and public-report schema ids;
- preserve a documented compatibility window for existing files and CLI consumers;
- add migration tests before changing either emitted identifier.

Until that decision is implemented, neither type is registered as the unique owner of `athanor.project_registry.v1`.

### Specialized graph and Rustok reports

`graph.rs` also exposes Rustok FFA, FBA, and page-builder audit/graph schemas. They remain outside this wave because the family is large and should be registered together with representative non-empty fixtures and transport parity coverage.

### Process-adapter protocols and persisted state

Extractor/linker/checker process payloads, daemon envelopes, MCP envelopes, index state, publication journals, generation pointers, and read-model manifests require a separate inventory pass. Internal persistence documents must not be mixed with public report schemas merely because both serialize as JSON.

## Enforcement implementation

`crates/athanor-app/tests/json_contract_inventory.rs` scans the currently identified app-layer agent-facing owner modules. Every canonical schema literal found there must either be present in `VERSIONED_JSON_CONTRACTS` or in the explicit migration allowlist.

The allowlist currently contains only the Context and Project Registry blockers plus the specialized Rustok graph/audit family. The test also fails when an allowlisted schema disappears or becomes registered without removing the stale exception.

This is a bounded first enforcement slice. Daemon/MCP envelopes, process protocols, persistence documents, and newly discovered source modules must be added after their inventory classification is complete.

## Enforcement rules

- Every registered schema id is valid and unique.
- Every registered Rust owner is unique.
- The owner implements `VersionedJsonContract`.
- A golden regression exercises serialization and `validate_contract()`.
- Local schema constants must equal the shared registry constant until literals are fully migrated.
- A schema id must never describe two materially different top-level shapes.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id.
- New agent-facing schema literals in inventoried modules must be registered or explicitly tracked by the migration allowlist.
