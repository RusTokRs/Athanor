---
id: doc://docs/development/json-contract-inventory.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Versioned JSON Contract Inventory

This inventory records JSON documents crossing CLI, daemon, MCP, persisted-state, generated-artifact,
interchange, adapter-plugin, automation, or process-adapter boundaries. Public reports, general
Rust-owned boundaries, adapter documents, and workflow-owned documents use separate checked registries.
Standard and schema-less protocols retain their native version/framing contracts.

Audit baseline: source commit `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`. Verification Matrix
`30013208011`, AppSec `30013208197`, and Store Conformance `30013208312` succeeded.

## Schema Id Grammar

Ordinary form: `athanor.<name>.v<major>`. A qualified family may use
`athanor.<name>.v<major>-<qualifier>-v<revision>`. Empty qualifiers, zero major/revision values, and
missing qualifier revisions fail closed.

## Registered Public Contracts

`VERSIONED_JSON_CONTRACTS` contains 62 current Rust-owned public application/daemon reports:

- overview, search, explain, impact, check, coverage, capabilities, change map, and context;
- index, benchmark, changed validation, coordinated generation, config, docs, API, wiki, and HTML;
- documentation-generation request and manifest;
- Graph and Rustok reports;
- project, Repair, daemon, and adapter-trust reports.

`DocumentationGenerationRequest` binds an exact snapshot, `architecture` profile, and bounded limits.
`DocumentationGenerationManifest` records request identity, effective limits, omissions, portable
artifact paths, media types, and lowercase SHA-256. Both reject unknown fields and schema drift.

## General Non-Public Rust Contracts

`NON_PUBLIC_JSON_CONTRACTS` contains 32 descriptors: 26 current documents, five accepted legacy
inputs, and one historical-only schema.

Current persisted families include project registry state, daemon endpoint, Index current/state and
publication journals, and canonical snapshot/latest/commit documents.

Current generated families include:

- validation result;
- coordinated generation manifest/current pointer;
- `athanor.documentation_current.v1` owned by `CurrentDocumentationGeneration`;
- `athanor.documentation_validation_report.v1` owned by `DocumentationValidationReport`;
- API contract snapshot/latest;
- JSONL, wiki, and HTML manifests.

Interchange families include documentation patches and projector payloads. Embedded families include
Index and generation metrics. Legacy input is never emitted as a current fixture; historical schemas
are not accepted compatibility.

The documentation current pointer requires schema, generation, snapshot, profile, relative generation
path, and manifest path. The validation report requires draft schema, snapshot/profile identity,
validation status, data policy, diagnostics, and quality metrics. Publication tests additionally require
the exact deterministic Markdown/report artifact set and checksum agreement.

## Adapter And Automation Contracts

`ADAPTER_NON_PUBLIC_JSON_CONTRACTS` contains two current and two legacy-input descriptors:

| Schema | Owner | Lifecycle |
| --- | --- | --- |
| `athanor.adapter_manifest.v1` | `AdapterPluginManifest` | current interchange input |
| `athanor.adapter_manifest` | `AdapterPluginManifest` | accepted legacy input |
| `athanor.adapter_trust_registry.v2` | `AdapterTrustRegistry` | current persisted read/write |
| `athanor.adapter_trust.v2` | `AdapterTrustRegistry` | accepted legacy input |

`AUTOMATION_JSON_CONTRACTS` contains `athanor.verification_evidence.v1`, owned by
`.github/workflows/verification-evidence.yml`. It records workflow identity, exact source SHA, run
identity, conclusion, completion time, and matrix after successful main-branch CI.

## Transport And Process Protocols

`DaemonRequest` owns current `athanor.daemon_request.v3`; `DaemonResponse` owns
`athanor.daemon_response.v3`; `DaemonJobsReport` owns `athanor.daemon_jobs.v1`. Earlier versions retain
their declared legacy/historical lifecycles.

MCP uses JSON-RPC `2.0` and protocol `2024-11-05`; native envelopes do not receive synthetic Athanor
schemas. CLI, daemon, and MCP Index serialize the same public `IndexReport`.

`PROCESS_PROTOCOL_CONTRACTS` records four schema-less protocols:

| Protocol | Request | Response | Framing |
| --- | --- | --- | --- |
| `source-discover` | `SourceDiscoverInput` object | `Vec<SourceFile>` array | newline JSON stdin / one JSON stdout |
| `extractor` | `ExtractInput` object | `ExtractOutput` object | newline JSON stdin / one JSON stdout |
| `linker` | `LinkInput` object | `Vec<Relation>` array | newline JSON stdin / one JSON stdout |
| `checker` | `CheckInput` object | `Vec<Diagnostic>` array | newline JSON stdin / one JSON stdout |

An incompatible process change requires an explicit versioned envelope.

## Source-Literal Classifications

Twenty canonical literals are explicitly source-classified outside stable public/general registries.
They cover CLI/daemon composite envelopes, GraphQL/JS-TS metadata, JSONL indexes, accepted legacy
inputs, and four intermediate documentation types:

- `athanor.documentation_outline.v1` — `DocumentationOutline`;
- `athanor.documentation_context.v1` — `DocumentationContext`;
- `athanor.documentation_citation.v1` — `DocumentationCitation`;
- `athanor.documentation_draft.v1` — `DocumentationDraft`.

These four are strict in-memory application contracts. The validation report is no longer in this
category because Slice 1B publishes it as a generated artifact.

## Enforcement

- `VERSIONED_JSON_CONTRACTS` protects 62 public owners.
- `NON_PUBLIC_JSON_CONTRACTS` protects 32 general boundaries.
- Adapter and automation registries remain disjoint from public/general contracts.
- `PROCESS_PROTOCOL_CONTRACTS` protects four schema-less shapes and framing.
- `json_contract_inventory.rs` recursively classifies production schema literals.
- `process_persistence_contract_inventory.rs` protects lifecycle counts and current fixtures.
- documentation contract tests protect strict fields, alignment, data policy, evidence, and limits.
- architecture profile/publication tests protect deterministic output, exact artifacts, checksums,
  immutable history, tamper recovery, force, and cancellation.

## Rules

- Every current public or non-public schema has exactly one checked owner.
- Every production Athanor schema literal is classified.
- Current persisted/generated/interchange documents have required-field fixtures.
- Legacy input normalizes before a current write or response.
- One schema never describes two current top-level shapes.
- Semantic field changes require a new major schema or protocol version.

## Verification

```bash
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
cargo test -p athanor-app --test documentation_architecture_publication_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test adapter_contract_inventory --locked
cargo test -p athanor-app --test public_report_transport_inventory --locked
cargo test -p athanor-app --test verification_evidence_inventory --locked
cargo check --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```
