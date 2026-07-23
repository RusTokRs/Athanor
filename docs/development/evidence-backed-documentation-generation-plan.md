---
id: doc://docs/development/evidence-backed-documentation-generation-plan.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Evidence-Backed Documentation Generation Plan

## Status

In progress. Slices 0A–0B define the strict evidence contracts and evaluation corpus. Slice 1A
implements the deterministic `architecture` profile; Slice 1B implements checksum-bound immutable
publication and an atomic current pointer. No CLI, daemon, MCP, model provider, store-loading entrypoint,
or new dependency is enabled yet. Slice 1C is the next bounded implementation.

The canonical snapshot remains the only truth source:

```text
source files -> adapters -> canonical snapshot -> bounded context -> cited document -> immutable generation
```

## Product Goal

Athanor should generate reviewable architecture documentation from verified repository knowledge. The
first profile contains an overview, components, canonical relationships, diagnostics, Mermaid source,
omission disclosure, and evidence footnotes. Module, API, operations, and onboarding profiles remain
later work.

Offline deterministic generation must remain useful. An optional future model may compose prose only
from `DocumentationContext`; it may not establish facts, explore arbitrary files, or directly rewrite
editable documentation.

## Non-Negotiable Boundaries

- The canonical snapshot is the only source for generated factual claims.
- `athanor-domain` and `athanor-core` remain free of presentation and provider types.
- Every request names an exact snapshot, supported profile, and hard input limits.
- Every factual claim and diagram edge is traceable to in-scope stable keys and evidence locations,
  unless explicitly marked as an inference with confidence and rationale.
- Context records effective limits and omitted counts instead of hiding truncation.
- Secrets and raw-file explorer access are forbidden; network use requires provider opt-in.
- Publication is immutable and atomic. Invalid or cancelled work must not advance `current.json`.
- Editable documentation changes remain explicit reviewable patch proposals.

## Reference Decisions

References supply bounded patterns, not dependencies by default.

| Reference | Retained idea | Excluded |
| --- | --- | --- |
| C4 / Structurizr | hierarchical architecture views | Structurizr runtime, DSL, and workspace format |
| arc42 | architecture coverage checklist | copied CC BY-SA template prose |
| Diataxis | separate reference and explanation intent | copied framework text or renderer assumptions |
| MADR | later reviewable decision records | inferring decisions from code without evidence |
| Mermaid | version-controlled diagram source | dependency in domain/core or only output format |
| SCIP | future imported code relations | replacing Athanor stable IDs, evidence, or snapshots |
| Backstage TechDocs | publication separation | portal/catalog/runtime adoption |
| Litho / deepwiki-rs | document-profile UX ideas | dependency, regex analysis, arbitrary file exploration |

No reference becomes a dependency without a separate adoption record covering license, maintenance,
MSRV, security, adapter boundary, fixture comparison, and replacement path.

## Implemented Contracts

### Slice 0A — Request And Manifest

- `athanor.documentation_generation_request.v1` binds an exact snapshot, `architecture` profile, and
  hard entity/fact/relation/diagnostic limits.
- `athanor.documentation_generation_manifest.v1` records request identity, generation, snapshot,
  effective limits, omissions, status, and checksum-bound artifacts.
- Unknown fields, schema drift, invalid identities, unsafe paths, duplicate IDs/paths, invalid SHA-256,
  and request/manifest mismatch fail closed.

### Slice 0B — Evidence Flow

- `athanor.documentation_outline.v1` — ordered section identity and selection reason;
- `athanor.documentation_context.v1` — bounded canonical items, stable keys, evidence, direction,
  omissions, and data policy;
- `athanor.documentation_citation.v1` — snapshot-bound stable keys and evidence ranges;
- `athanor.documentation_draft.v1` — cited claims, explicit inferences, and diagram edges;
- `athanor.documentation_validation_report.v1` — status, diagnostics, policy, quality, and optional
  provider-cost fields.

Outline, context, citation, and draft remain source-owned intermediate types. Validation reports are
now real generated boundaries because Slice 1B writes them into immutable generations.

## Implemented Runtime Slices

### Slice 1A — Deterministic Architecture Profile

`build_documentation_architecture_profile` consumes one explicit `CanonicalSnapshot` and request. It:

- rejects absent or mismatched snapshot identity;
- sorts canonical entities, facts, relations, and diagnostics deterministically;
- applies hard per-kind limits and discloses omissions;
- builds Overview, Components, Relationships, and Diagnostics sections;
- emits cited claims, relation-backed Mermaid source, evidence footnotes, and lowercase SHA-256;
- returns a valid `DocumentationValidationReport` without network, provider, store, or filesystem use.

The output is invariant to canonical input order and is fixture protected.

### Slice 1B — Immutable App-Layer Publication

`publish_documentation_architecture_generation` publishes under:

```text
.athanor/generated/documentation/
  current.json
  generations/<8-digit-generation>/
    manifest.json
    architecture/index.md
    validation-report.json
```

The publisher:

- stages a new immutable directory before publication;
- writes the exact Markdown and validation-report artifact set;
- validates paths, manifest identity, media types, and expected deterministic checksums;
- returns `UpToDate` only when the existing current generation exactly matches the rebuilt output;
- creates a new generation after pointer, manifest, document, report, or checksum drift;
- preserves immutable history and supports explicit `force`;
- preserves the previous pointer and generation set when cancelled before publication.

`athanor.documentation_current.v1` and `athanor.documentation_validation_report.v1` are registered as
current generated boundaries. The existing coordinated `ath generate` command is unchanged.

## Execution Evidence

Slices 1A–1B are confirmed on exact source commit
`0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`:

- Verification Matrix run `30013208011` — success;
- AppSec run `30013208197` — success;
- Store Conformance run `30013208312` — success.

The matrix covered Rust 1.95 formatting, workspace tests, Clippy, Linux/macOS/Windows smoke, installers,
default/store-surreal/js-ts-precision/all-features, cargo-deny, documentation check, and source coverage.
This evidence confirms the source slices; it does not claim a user-facing CLI that is not implemented.

## Delivery Slices

### Slices 0A–0B — Implemented And Execution-Confirmed

Strict contracts, fixtures, evaluation corpus, data policy, alignment, and fail-closed regressions.

### Slice 1A — Implemented And Execution-Confirmed

Pure deterministic architecture composition from an explicit canonical snapshot.

### Slice 1B — Implemented And Execution-Confirmed

Atomic immutable application-layer publication, exact artifact reuse, tamper recovery, force, and
cancellation regressions.

### Slice 1C — Next

- Load one exact committed canonical snapshot through explicit `RuntimeComposition`.
- Add a bounded application operation that composes and publishes without bypassing store visibility.
- Add an explicit CLI command for architecture generation rather than silently changing `ath generate`.
- Add bounded current/manifest/validation inspection commands.
- Preserve typed cancellation and exact snapshot identity across the CLI/application boundary.
- Add CLI help, JSON/text output, project-resolution, missing-snapshot, and cancellation regressions.

### Later Slices

- Slice 2: optional provider-neutral structured composer with recorded-response tests.
- Slice 3: replaceable diagram syntax/render validation and quality gates.
- Slice 4: module, API, operations, onboarding, and editable-document proposals.
- Slice 5: incremental recomposition and explicit external-knowledge adapters.

## Acceptance Criteria

- Architecture documents can be generated from a committed snapshot with no model or network.
- Every published document declares snapshot, profile, contract versions, limits, omissions, and hash.
- Every substantive claim and diagram relation is traceable to stable keys and evidence.
- Invalid or tampered output cannot be treated as up to date or advance the current pointer.
- Model output cannot introduce canonical facts or overwrite editable documentation.
- The Rustok evaluation demonstrates useful signal before profile expansion.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
cargo test -p athanor-app --test documentation_architecture_publication_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Slice 1C must additionally prove exact committed-snapshot loading and CLI behavior before the package
can be considered user-facing complete.
