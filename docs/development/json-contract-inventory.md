---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, generated-artifact, interchange, adapter-plugin, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level current Athanor schema id and its current payload shape is protected by a regression fixture. Standard and schema-less protocols use separate registries instead of receiving synthetic Athanor schema ids.

Audit baseline: current `main`; Rust-enabled verification is pending for the current HEAD.

## Registered public contracts

The public registry contains 60 current Athanor-schema owners. The previously documented 59 owners remain unchanged, and the adapter trust status response now has a distinct owner:

| Schema | Rust owner | Boundary | Fixture status |
| --- | --- | --- | --- |
| `athanor.adapter_trust_report.v1` | `VersionedAdapterTrustReport` | direct `ath plugins list/trust/untrust --json` application output | adapter contract fixture plus executable CLI regression; execution pending |

The other registered families remain:

- overview, search, explain, impact, diagnostic/check, coverage, capabilities, change map, and context;
- index, benchmark, changed validation, generation, config, documentation, API, wiki, and HTML reports;
- graph and specialized RusTok report families;
- project registry/resolution reports;
- nine Repair owners;
- daemon request v3, response v3, and jobs v1.

`VERSIONED_JSON_CONTRACTS` validates schema and Rust-owner uniqueness across all 60 current public owners.

## Adapter plugin and trust contracts

Adapter boundaries use three distinct current owners:

| Schema | Rust owner | Class | Lifecycle |
| --- | --- | --- | --- |
| `athanor.adapter_manifest.v1` | `AdapterPluginManifest` | project-local interchange/configuration | current input |
| `athanor.adapter_trust_registry.v2` | `AdapterTrustRegistry` | user-level persisted state | current read/write |
| `athanor.adapter_trust_report.v1` | `VersionedAdapterTrustReport` | public CLI/application response | current output |

`ADAPTER_NON_PUBLIC_JSON_CONTRACTS` separately records the two non-public current owners and their compatibility inputs:

| Legacy schema | Current normalization | Lifecycle |
| --- | --- | --- |
| `athanor.adapter_manifest` | `athanor.adapter_manifest.v1` | accepted legacy input |
| `athanor.adapter_trust.v2` | `athanor.adapter_trust_registry.v2` | accepted legacy persisted input |

Legacy manifest and trust-registry documents are normalized in memory before runtime use. A subsequent trust mutation writes only `athanor.adapter_trust_registry.v2`. The focused `ath plugins` dispatcher emits only `athanor.adapter_trust_report.v1` while preserving the existing command grammar and human-readable rendering.

The older library functions in `runtime.rs` still return the legacy `AdapterTrustReport` structure for source compatibility. They are not current registry owners and remain a compatibility-cleanup item; callers requiring a current contract must use `list_adapter_plugin_trust_versioned`, `trust_adapter_plugin_versioned`, or `untrust_adapter_plugin_versioned`.

## Transport and process protocols

### Daemon

`DaemonRequest` owns current `athanor.daemon_request.v3`; `DaemonResponse` owns current `athanor.daemon_response.v3`; `DaemonJobsReport` owns `athanor.daemon_jobs.v1`. Request v1/v2 compatibility remains accepted input only. `DaemonError`, `DaemonCommand`, and `DaemonJob` are embedded. `DaemonEndpoint` is persisted runtime discovery state: v3 is current, v2 is accepted legacy input, and v1 is historical-only and rejected by the current reader.

### MCP

MCP uses JSON-RPC `2.0` and MCP protocol `2024-11-05`. `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, and text tool-call results remain native standard-protocol boundaries in `MCP_TRANSPORT_CONTRACTS`; they do not receive synthetic `athanor.*` schemas. The active server validates protocol/session state and uses bounded request and response resources. Execution of its current regressions remains pending.

### External processes

`PROCESS_PROTOCOL_CONTRACTS` records four intentionally schema-less protocols:

| Protocol | Request | Response | Framing |
| --- | --- | --- | --- |
| `source-discover` | `SourceDiscoverInput` object | `Vec<SourceFile>` array | newline-terminated JSON stdin, one JSON stdout document |
| `extractor` | `ExtractInput` object | `ExtractOutput` object | newline-terminated JSON stdin, one JSON stdout document |
| `linker` | `LinkInput` object | `Vec<Relation>` array | newline-terminated JSON stdin, one JSON stdout document |
| `checker` | `CheckInput` object | `Vec<Diagnostic>` array | newline-terminated JSON stdin, one JSON stdout document |

These wire shapes are the serde representation of core port types. Adding a top-level schema now would change existing external adapters; future incompatible changes require an explicit versioned process envelope rather than silent field mutation.

## General non-public Athanor-schema registry

`NON_PUBLIC_JSON_CONTRACTS` retains 30 descriptors: 24 current documents, five accepted legacy inputs, and one historical-only schema. The adapter-specific registry adds four more descriptors: two current and two accepted legacy inputs.

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
- `athanor.adapter_trust_registry.v2` in the adapter-specific registry

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

- Interchange: `athanor.docs_patch.v1`, `athanor.wiki_projection.v1`, `athanor.html_report_projection.v1`, and adapter manifest v1 in the adapter-specific registry.
- Embedded: `athanor.index_metrics.v1`, `athanor.index_report_metrics.v1`, `athanor.generation_metrics.v1`, plus schema-less Repair and Daemon fragments.

### Compatibility lifecycle

The general registry accepts legacy daemon endpoint, index current/publication, and canonical commit inputs as documented by their readers. The adapter-specific registry additionally accepts legacy unversioned adapter manifests and legacy shared trust-registry v2 documents. Legacy input is never emitted as a current fixture.

Cleanup tombstones, staging directories, backups, locks, and repair guards are filesystem recovery protocols, not JSON documents. Schema-less canonical latest documents from older stores are compatibility input but cannot enter an Athanor schema registry because they have no schema id.

## Enforcement implementation

- `VERSIONED_JSON_CONTRACTS` protects 60 current public/transport Athanor owners.
- `MCP_TRANSPORT_CONTRACTS` protects four native JSON-RPC/MCP boundaries.
- `NON_PUBLIC_JSON_CONTRACTS` protects 30 general persisted/generated/interchange/embedded descriptors.
- `ADAPTER_NON_PUBLIC_JSON_CONTRACTS` protects two current adapter documents and two accepted legacy inputs.
- `PROCESS_PROTOCOL_CONTRACTS` protects the four schema-less external-process shapes and framing.
- `boundary_contracts.v1.json` and `process_persistence_contract_inventory.rs` retain general process/persistence coverage.
- `adapter_contracts.v1.json` and `adapter_contract_inventory.rs` protect adapter ownership, normalization, current writes, and source observability.
- `direct_plugin_cli.rs` and `direct_plugin_cli` executable tests protect current public CLI output while preserving command grammar and human rendering.
- MCP unit/in-memory stdio and source guards protect bounded resources and JSON-RPC categories.

Actual test, formatting, and Clippy execution remain outstanding because the current environment has no Rust toolchain and the connector does not expose a hosted status for the latest direct-to-main commits.

## Enforcement rules

- Every current public schema has exactly one Rust owner and a regression fixture.
- Non-public and public schema sets are disjoint within each registry; current schema ids are canonical and unique.
- Standard and schema-less protocols retain native framing/version rules instead of receiving synthetic Athanor schema ids.
- Current persisted/generated/interchange documents have representative required-field fixtures.
- Legacy input is never emitted as a current fixture; historical-only schemas are not presented as accepted compatibility.
- Equivalent CLI/application and daemon/MCP inner results serialize the same typed Athanor document.
- A schema id must never describe two current emitted top-level shapes.
- Legacy input normalizes to a current schema before a current write or current response.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id or protocol version.
- New schema literals and protocol envelopes must be registered or explicitly classified in the same change.
