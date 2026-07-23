---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents that cross CLI, daemon, MCP, persisted-state,
generated-artifact, interchange, adapter-plugin, repository-automation, or process-adapter
boundaries. Public Rust reports, general Rust-owned boundaries, adapter boundaries, and
workflow-owned automation documents use separate checked registries. Standard and schema-less
protocols retain their native version/framing contracts.

Audit baseline: current `main`. Source inventories and fixtures are implemented; execution evidence
for the current HEAD remains pending.

## Schema id grammar

The ordinary form is `athanor.<name>.v<major>`. A feature-qualified family may use
`athanor.<name>.v<major>-<qualifier>-v<revision>`. The current
`athanor.index_state.v46-js-ts-precision-v1` therefore has compatibility major `46`. Empty qualifiers,
zero major/revision values, and missing qualifier revisions fail closed.

## Registered public contracts

`VERSIONED_JSON_CONTRACTS` contains 62 current Rust-owned public application and daemon contracts:

- overview, search, explain, impact, check, coverage, capabilities, change map, and context;
- index, benchmark, changed validation, coordinated generation, config, documentation, API, wiki,
  and HTML;
- evidence-backed documentation generation request and manifest contracts;
- standard Graph and specialized RusTok reports;
- project registry/resolution reports;
- Repair reports;
- daemon request, response, and jobs reports;
- the adapter trust status report.

`DocumentationGenerationRequest` binds one exact snapshot and the supported `architecture` profile to
explicit non-zero limits. `DocumentationGenerationManifest` records request-schema identity,
effective limits, omitted counts, normalized relative output paths, and lowercase SHA-256 checksums.
Both reject unknown fields and schema drift through fixture-backed regressions. Each public schema has
one current Rust owner.

## General non-public Rust contracts

`NON_PUBLIC_JSON_CONTRACTS` retains 30 descriptors: 24 current documents, five accepted legacy
inputs, and one historical-only schema.

Current persisted families include project registry state, daemon endpoint, Index current/state and
publication journals, canonical snapshot/latest/commit documents. Current generated families include
validation result, coordinated generation/current pointers, API contract snapshot/latest, JSONL
manifest, wiki manifest, and HTML manifest. Interchange families include docs patches and projector
payloads. Embedded families include Index and generation metrics.

Legacy input is never emitted as a current fixture. Historical-only schemas are not presented as
accepted compatibility.

## Adapter plugin and trust contracts

`ADAPTER_NON_PUBLIC_JSON_CONTRACTS` contains two current and two legacy-input descriptors:

| Schema | Owner | Lifecycle |
| --- | --- | --- |
| `athanor.adapter_manifest.v1` | `AdapterPluginManifest` | current interchange input |
| `athanor.adapter_manifest` | `AdapterPluginManifest` | accepted legacy input |
| `athanor.adapter_trust_registry.v2` | `AdapterTrustRegistry` | current persisted read/write |
| `athanor.adapter_trust.v2` | `AdapterTrustRegistry` | accepted legacy input |

Legacy documents normalize in memory before runtime use. Current writes use only current schemas. The
public trust status response remains `athanor.adapter_trust_report.v1`.

## Automation-owned contracts

`AUTOMATION_JSON_CONTRACTS` contains documents emitted by repository automation rather than Rust
runtime owners.

| Schema | Owner | Class | Required identity |
| --- | --- | --- | --- |
| `athanor.verification_evidence.v1` | `.github/workflows/verification-evidence.yml` | persisted current | workflow, exact head SHA, run id/URL, conclusion, completion time, matrix |

The verification evidence document is published only after a successful push-CI run on `main`. It is
not a public application report and does not belong in a registry whose descriptor requires a
`rust_owner`. The automation registry is validated for canonical schema ids, ownership metadata,
required fields, duplicate ids, and disjointness from public, general, and adapter registries.

## Transport and process protocols

### Daemon

`DaemonRequest` owns current `athanor.daemon_request.v3`; `DaemonResponse` owns
`athanor.daemon_response.v3`; `DaemonJobsReport` owns `athanor.daemon_jobs.v1`. Earlier request and
endpoint versions follow their declared legacy/historical lifecycles.

### MCP

MCP uses JSON-RPC `2.0` and MCP protocol `2024-11-05`. Request, response, error, and text tool-call
results remain native standard-protocol boundaries and do not receive synthetic `athanor.*` schemas.
CLI, daemon, and MCP Index paths serialize the same public `IndexReport`.

### External processes

`PROCESS_PROTOCOL_CONTRACTS` records four intentionally schema-less protocols:

| Protocol | Request | Response | Framing |
| --- | --- | --- | --- |
| `source-discover` | `SourceDiscoverInput` object | `Vec<SourceFile>` array | newline JSON stdin / one JSON stdout |
| `extractor` | `ExtractInput` object | `ExtractOutput` object | newline JSON stdin / one JSON stdout |
| `linker` | `LinkInput` object | `Vec<Relation>` array | newline JSON stdin / one JSON stdout |
| `checker` | `CheckInput` object | `Vec<Diagnostic>` array | newline JSON stdin / one JSON stdout |

Adding a synthetic top-level schema would change existing external adapters; an incompatible future
change requires an explicit versioned process envelope.

## Source-literal classifications

Sixteen canonical `athanor.*` literals are explicitly classified with a Rust owner and
lifecycle even though they are not members of the stable public/general registries. They
cover composite CLI and daemon envelopes, embedded GraphQL/JS-TS metadata, JSONL indexes,
and accepted legacy daemon/Repair inputs. The source scanner validates their canonical ids,
uniqueness, ownership, lifecycle, and observability.

Non-canonical Athanor-prefixed strings such as the `athanor.index_state.v` validation prefix
and the `athanor.toml` filename are not schema ids and are excluded only when they are not explicitly
classified and `validate_schema_id` rejects them. Registered legacy ids remain observable.

## Enforcement implementation

- `VERSIONED_JSON_CONTRACTS` protects 62 current public Rust owners.
- `NON_PUBLIC_JSON_CONTRACTS` protects 30 general Rust-owned boundary descriptors.
- `ADAPTER_NON_PUBLIC_JSON_CONTRACTS` protects two current and two legacy adapter documents.
- `AUTOMATION_JSON_CONTRACTS` protects the current workflow-owned verification evidence document.
- `PROCESS_PROTOCOL_CONTRACTS` protects four schema-less process shapes and framing.
- `json_contract_inventory.rs` scans production Rust sources recursively and requires every quoted
  `athanor.*` literal to be classified.
- `documentation_generation_contract_inventory.rs` protects request/manifest round trips, strict
  fields, schema identity, normalized output paths, checksums, and request/manifest alignment.
- Public, general, adapter, and automation schema sets are mutually disjoint.
- Current ids validate under ordinary or qualified grammar.
- Current boundary fixtures protect required fields and lifecycle rules.
- Daemon/MCP regressions protect typed transport payload parity.

## Enforcement rules

- Every current public schema has exactly one Rust owner.
- Every automation schema has one explicit workflow owner.
- Every production Athanor schema literal is classified by a checked registry.
- Current persisted/generated/interchange documents have required-field coverage.
- Legacy input normalizes before a current write or response.
- A schema id never describes two current emitted top-level shapes.
- Removing, renaming, retyping, or semantically changing a field requires a new major schema id or
  protocol version.

## Verification

```bash
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test adapter_contract_inventory --locked
cargo test -p athanor-app --test public_report_transport_inventory --locked
cargo test -p athanor-app --test verification_evidence_inventory --locked
cargo check --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

The inventory is implemented, not verified, until this matrix is executed on one commit.
