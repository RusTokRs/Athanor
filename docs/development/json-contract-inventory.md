---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level schema id and its current payload shape is protected by a regression fixture.

Audit baseline: `main` at `fe4b22ef28ac7d0c17058eb05db556e467128264`.

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
| `athanor.project_resolution.v1` | `ProjectResolutionReport` | CLI project resolution | second-wave golden |

## Resolved migration decisions

### Context pack

`ContextPack` remains the internal domain value and still carries detailed evidence in its nested `payload`. Public JSON boundaries now serialize `ContextReport`, which adds the required top-level `schema` field and flattens the existing context-pack fields rather than introducing a new `pack` nesting level.

This preserves the established `id`, `task`, `scope`, `level`, `summary`, entity, file, diagnostic, confidence, and payload fields while making `athanor.context_pack.v1` valid under `VersionedJsonContract`.

The registered owner is `ContextReport`. A dedicated golden fixture protects both the new top-level schema and the unchanged nested payload schema. The migration allowlist no longer contains `athanor.context_pack.v1`.

The direct CLI JSON path, cached and operation-aware daemon context paths, and the active lifecycle-based MCP context path now serialize the wrapper. Internal context generation and daemon caching continue to use `ContextPack`.

## Discovered contracts requiring migration decisions

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

## Shared-constant migration

The registered Search, Impact, Diagnostic Check, Affected Check, Operations Docs Check, and ChangeMap builders now import their schema ids from `json_contract` instead of embedding quoted schema literals. Impact covers both the normal report path and the empty-diff early return.

`json_contract_inventory.rs` protects all six migrated schema ids with a source-level regression: each schema must remain registered and its quoted literal must not reappear in the owner source. Unit assertions for Check and ChangeMap also use the shared constants.

## Enforcement implementation

`crates/athanor-app/tests/json_contract_inventory.rs` scans the currently identified app-layer agent-facing owner modules. Every canonical schema literal found there must either be present in `VERSIONED_JSON_CONTRACTS` or in the explicit migration allowlist.

The allowlist now contains only the Project Registry collision and the specialized Rustok graph/audit family. The test also fails when an allowlisted schema disappears or becomes registered without removing the stale exception.

This is a bounded first enforcement slice. Daemon/MCP envelopes, process protocols, persistence documents, and newly discovered source modules must be added after their inventory classification is complete.

## Enforcement rules

- Every registered schema id is valid and unique.
- Every registered Rust owner is unique.
- The owner implements `VersionedJsonContract`.
- A golden regression exercises serialization and `validate_contract()`.
- Migrated builders must import their schema id from the shared registry module and must not embed the quoted literal.
- Local schema constants must equal the shared registry constant until literals are fully migrated.
- A schema id must never describe two materially different top-level shapes.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id.
- New agent-facing schema literals in inventoried modules must be registered or explicitly tracked by the migration allowlist.
