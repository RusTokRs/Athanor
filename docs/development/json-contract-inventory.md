---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state, generated-artifact, interchange, adapter-plugin, or process-adapter boundaries. A document may enter `VERSIONED_JSON_CONTRACTS` only when one Rust type owns one top-level current Athanor schema id. Standard and schema-less protocols use separate registries instead of receiving synthetic Athanor schema ids.

Audit baseline: current `main`. The source inventories and regression fixtures described below are implemented; Rust formatting, tests, Clippy, and executable interoperability remain pending for the current HEAD.

## Schema id grammar

The ordinary form is:

```text
athanor.<name>.v<major>
```

A feature-qualified wire family may use the stricter form:

```text
athanor.<name>.v<major>-<qualifier>-v<revision>
```

The qualified form preserves the base compatibility major while versioning the qualifier. The current JS/TS precision index-state schema, `athanor.index_state.v46-js-ts-precision-v1`, therefore validates with compatibility major `46`. Empty qualifiers, zero major/revision values, and missing qualifier revisions fail closed.

## Registered public contracts

The public registry contains 60 current Athanor-schema owners. The adapter trust status response has a distinct public owner:

| Schema | Rust owner | Boundary | Fixture status |
| --- | --- | --- | --- |
| `athanor.adapter_trust_report.v1` | `VersionedAdapterTrustReport` | direct `ath plugins list/trust/untrust --json` application output | adapter contract fixture and CLI regression implemented; execution pending |

The other registered families are:

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

## Transport and process protocols

### Daemon

`DaemonRequest` owns current `athanor.daemon_request.v3`; `DaemonResponse` owns current `athanor.daemon_response.v3`; `DaemonJobsReport` owns `athanor.daemon_jobs.v1`. Request v1/v2 compatibility remains accepted input only. `DaemonError`, `DaemonCommand`, and `DaemonJob` are embedded. `DaemonEndpoint` is persisted runtime discovery state: v3 is current, v2 is accepted legacy input, and v1 is historical-only and rejected by the current reader.

### MCP

MCP uses JSON-RPC `2.0` and MCP protocol `2024-11-05`. `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, and text tool-call results remain native standard-protocol boundaries in `MCP_TRANSPORT_CONTRACTS`; they do not receive synthetic `athanor.*` schemas.

CLI, daemon, and MCP Index paths serialize the same public `IndexReport` type. `public_report_transport_inventory.rs`, the daemon typed parity regressions, and the MCP Index inventory prohibit transport-specific replacement payloads.

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
- `athanor.index_state.v46` or qualified `athanor.index_state.v46-js-ts-precision-v1`
- `athanor.index_publication.v3`
- `athanor.index_current_publication.v1`
- `athanor.canonical_snapshot.v1`
- `athanor.canonical_latest.v1`
- `athanor.canonical_commit.v2`
- `athanor.adapter_trust_registry.v2` in the adapter-specific registry

`IndexState.generation` is derived and required when `snapshot` is present. An empty current state has `snapshot: null` and may omit `generation`; both feature alternatives use this conditional rule.

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
- `NON_PUBLIC_JSON_CONTRACTS` protects 30 general persisted/generated/interchange/embedded descriptors.
- `ADAPTER_NON_PUBLIC_JSON_CONTRACTS` protects two current adapter documents and two accepted legacy inputs.
- `PROCESS_PROTOCOL_CONTRACTS` protects four schema-less external-process shapes and framing.
- `json_contract_inventory.rs` recursively scans production Rust sources under workspace `crates/*/src` and `apps/*/src`; any new quoted `athanor.*` schema literal must be registered or classified in the same change.
- Unit-test source files and terminal `#[cfg(test)] mod tests` bodies are excluded from emitter discovery, so fixture-only example ids do not masquerade as production contracts.
- `process_persistence_contract_inventory.rs` uses recursive source observability instead of a stale list of monolithic paths.
- `boundary_contracts.v1.json` and `adapter_contracts.v1.json` protect current required-field fixtures and legacy lifecycle rules.
- `public_report_transport_inventory.rs` and focused daemon/MCP regressions protect typed payload parity.

## Enforcement rules

- Every current public schema has exactly one Rust owner.
- Public, general non-public, and adapter non-public schema sets are disjoint.
- Every production Athanor schema literal is exhaustively classified by a checked-in registry.
- Current schema ids are canonical under the ordinary or qualified grammar.
- Standard and schema-less protocols retain native framing/version rules instead of receiving synthetic Athanor schema ids.
- Current persisted/generated/interchange documents have representative required-field fixtures.
- Legacy input is never emitted as a current fixture; historical-only schemas are not presented as accepted compatibility.
- Equivalent CLI/application and daemon/MCP inner results serialize the same typed Athanor document.
- A schema id must never describe two current emitted top-level shapes.
- Legacy input normalizes to a current schema before a current write or current response.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id or protocol version.

## Verification

```bash
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test adapter_contract_inventory --locked
cargo test -p athanor-app --test public_report_transport_inventory --locked
cargo test -p athanor-transport-mcp --test index_publication_cancellation_inventory --locked
cargo check --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

The inventory is implemented, not verified, until this matrix is executed on one commit.
