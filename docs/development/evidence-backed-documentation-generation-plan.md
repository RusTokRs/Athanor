---
id: doc://docs/development/evidence-backed-documentation-generation-plan.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Evidence-Backed Documentation Generation Plan

## Status

In progress. Slices 0A–1C are execution-confirmed. The architecture exact-snapshot CLI and validated
inspection surface passed the full cross-platform matrix. The repaired bounded Rustok evaluation is
matrix-confirmed on `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e`; the human-facing unsupported-relation
disclosure is exact-evaluation-confirmed on `6862aee81dd0f53fa8372d1ce3fcb6e2ed198cca`.

Module expansion is source-implemented through Slice 2C, API through Slice 3C, operations through Slice
4C, and onboarding through Slice 5C. Focused module, API, operations, and onboarding format/test/Clippy
evidence remains pending and is not inferred from source presence.

The existing coordinated `ath generate` command is unchanged. No model provider, daemon, MCP, or new dependency is enabled.

```text
source files -> adapters -> exact committed snapshot -> bounded context -> cited document -> immutable generation
```

## Non-Negotiable Boundaries

- The canonical snapshot is the only source for factual claims.
- Every request names an exact committed snapshot and hard limits; there is no latest fallback.
- Claims and diagram edges carry in-scope stable keys/evidence or an explicit inference marker.
- Omitted counts are visible.
- Secrets/raw-file explorer access are forbidden; network/provider use is opt-in and currently absent.
- Invalid, tampered, or cancelled work cannot advance `current.json`.
- Inspection validates profile, path confinement, identities, exact artifact layout, and checksums.
- New profiles start as pure deterministic owners before publication or transport integration.

## Implemented Contracts

- `athanor.documentation_generation_request.v1`
- `athanor.documentation_generation_manifest.v1`
- `athanor.documentation_outline.v1`
- `athanor.documentation_context.v1`
- `athanor.documentation_citation.v1`
- `athanor.documentation_draft.v1`
- `athanor.documentation_validation_report.v1`
- `athanor.documentation_current.v1`

`DocumentationProfile` contains `architecture`, `module`, `api`, `operations`, and `onboarding`; the v1
schema names remain unchanged.

## Implemented Slices

### Slices 0A–0B — Contracts And Evidence Flow

Strict schemas, hard limits, omissions, portable paths, SHA-256, data policy, quality metrics, fixture
repository, Rustok evaluation corpus, and full-chain fail-closed alignment.

### Slice 1A — Deterministic Architecture Profile

`build_documentation_architecture_profile` consumes one explicit `CanonicalSnapshot`, applies stable
ordering and hard limits, and emits Overview, Components, Relationships, Diagnostics, cited claims,
Mermaid source, evidence footnotes, validation metrics, and Markdown SHA-256 without filesystem, Store,
network, or provider access.

### Slice 1B — Immutable Architecture Publication

```text
.athanor/generated/documentation/
  current.json
  generations/<8-digit-generation>/
    manifest.json
    architecture/index.md
    validation-report.json
```

Publication is staged and immutable. `UpToDate` requires exact artifact IDs, paths, media types,
identities, and deterministic hashes. Force, tamper recovery, history retention, and cancellation are
regression protected.

### Slice 1C — Exact Store Operation, CLI, And Inspection

The operation initializes the configured Store, loads exactly `SnapshotId(request.snapshot)` through
`CanonicalSnapshotStore`, verifies identity, checks cancellation, and delegates publication.

```bash
ath docs generate-architecture <PATH> --snapshot <EXACT-ID> \
  [--max-entities N] [--max-facts N] [--max-relations N] [--max-diagnostics N] [--force] [--json]
ath docs architecture current <PATH> [--json]
ath docs architecture manifest <PATH> [--json]
ath docs architecture validation <PATH> [--json]
```

Generation drains after Ctrl-C cancellation. Inspection rejects path escape, wrong profile, identity
or layout drift, invalid validation status, and checksum drift.

### Slices 2A–2C — Module Documentation Profile

- Slice 2A established the exact-snapshot evidence-backed module inventory and `modules/index.md`.
- Slice 2B added module-scoped facts, relations, open diagnostics, Mermaid edges, omissions, and the
  deterministic aggregate 256-item round-robin budget.
- Slice 2C added immutable publication, exact Store loading, `generate-module`, and validated
  `module current|manifest|validation` with profile-isolated shared `current.json`.
- Module publication/Store/CLI source regressions are present; focused execution evidence is pending.

### Slices 3A–3C — API Documentation Profile

- Slice 3A established exact endpoint/schema/example inventory, portable evidence, deterministic
  round-robin selection, omissions, citations, `api/index.md`, and checksum binding.
- Slice 3B added API-scoped facts, supported canonical relations, open diagnostics, aggregate 256-item
  budgeting, and cited Mermaid relationships.
- Slice 3C added immutable publication, exact Store loading, `generate-api`, and validated
  `api current|manifest|validation` with profile-isolated shared `current.json`.
- API source regressions are present; focused execution evidence remains pending.

### Slices 4A–4C — Operations Documentation Profile

- Slices 4A–4B establish the pure exact-snapshot operations profile over evidence-backed `EnvVar`,
  script/CI, deployment, database, configuration, runbook, and operation-step entities; general
  `Package`/`Dependency` entities remain outside the bounded inventory.
- Facts are scoped by selected operations anchors. Supported relations are `Defines`, `Contains`,
  `Documents`, `DocumentsOperation`, `UsesEnv`, and `QueriesTable`; open diagnostics require an anchor.
- Shared portable evidence normalization, per-kind limits, aggregate `DOCUMENTATION_REFERENCE_LIMIT = 256`,
  cited Mermaid, omissions, validation metrics, deterministic `operations/index.md`, and SHA-256 remain
  regression protected.
- Slice 4C publishes `operations/index.md` plus `validation-report.json` in the shared immutable generation
  root and advances the profile-aware `current.json` only after a complete staged publication.
- Exact `UpToDate` validates profile, snapshot, generation path, manifest layout, artifact checksums, and
  validation identity/status. Force, tamper recovery, immutable history, and cancellation are protected.
- `DocumentationOperationsOperationOptions` initializes the configured Store and loads only exact
  `SnapshotId(request.snapshot)`; missing snapshots and Store identity mismatch fail closed.
- `ath docs generate-operations <PATH> --snapshot <EXACT-ID>` supports hard limits, `--force`, `--json`,
  and Ctrl-C cancellation/drain. `ath docs operations current|manifest|validation` validates profile,
  confinement, layout, identities, and SHA-256. Existing `ath docs operations check` remains unchanged.
- Operations publication/Store/CLI source regressions are present; focused execution evidence is pending.

### Slices 5A–5C — Onboarding Documentation Profile

- `DocumentationProfile::Onboarding` serializes as `onboarding` without changing v1 schema names.
- `build_documentation_onboarding_profile` is pure over one exact `CanonicalSnapshot`; eligible evidence-backed
  anchors are `DocumentationPage`, `DocumentationSection`, `Package`, `ScriptCommand`, `EnvVar`, and `TestCase`/`CiJob`.
- Six category queues use stable-key/entity-id round-robin. Slice 5B adds facts when subject or object is a
  selected anchor, supported `Contains`, `Documents`, `UsesEnv`, and `TestedBy` relations touching an anchor,
  and open diagnostics referencing an anchor.
- Per-kind limits feed deterministic Entity/Fact/Relation/Diagnostic round-robin under the aggregate
  `DOCUMENTATION_REFERENCE_LIMIT = 256`; each selected item is cited and supported relations emit Mermaid edges.
- Omitted counts are scoped to eligible onboarding evidence; omitted supported relations are disclosed and
  feed `validation.metrics.unsupported_relations`. Shared portable evidence normalization remains the owner.
- Slice 5C publishes deterministic `onboarding/index.md` plus `validation-report.json` in the shared immutable
  generation root and advances profile-aware `current.json` only after complete staged publication.
- Exact `UpToDate` validates profile, snapshot, generation path, manifest layout, artifact checksums, and
  validation identity/status. Force, tamper recovery, immutable history, and cancellation are source protected.
- `DocumentationOnboardingOperationOptions` initializes the configured Store and loads only exact
  `SnapshotId(request.snapshot)`; missing snapshots and Store identity mismatch fail closed.
- `ath docs generate-onboarding <PATH> --snapshot <EXACT-ID>` supports hard limits, `--force`, `--json`, and
  Ctrl-C cancellation/drain. `ath docs onboarding current|manifest|validation` validates profile, confinement,
  layout, identities, and SHA-256.
- Shared `current.json` is profile-isolated across architecture, module, API, operations, and onboarding.
- Onboarding profile/publication/Store/CLI source regressions are present; focused execution evidence remains pending.

## Execution Evidence

- Slices 0A–0B: source `2a049303e797f00ac53f1e91fc010f284993926d`; CI `30005828864`,
  AppSec `30005828850`, Store `30005828956`.
- Slices 1A–1B: source `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`; CI `30013208011`,
  AppSec `30013208197`, Store `30013208312`.
- Slice 1C1: source `4f567271ed6d38d30b3c15dc6999aa33152a9312`; CI `30015689753`, AppSec
  `30015691399`, Store `30015689363`.
- Slice 1C2: source `042d02ac6b4c89d90a5b76c818098eb0c6b41920`; CI `30025932615`, AppSec
  `30025931953`, Store `30025932704`.
- First Rustok attempt: source `5e0b28099c48e22bdc172fa57b6d51db9e6efb7b`, workflow run
  `30029451096`; generation failed with
  `documentation draft citations must contain between 1 and 256 entries` after indexing completed.
- Repair probe: source `12a8687c5d098ab05a5988508816aad5f0dc3e23`, run `30030131126`.
- Repaired evaluation: source `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e`; evaluation `31625608720`,
  probe `31625608721`, CI `31625608729`, AppSec `31625608723`, Store `31625608739`.
- Relation disclosure: source `6862aee81dd0f53fa8372d1ce3fcb6e2ed198cca`; evaluation `32712992516`,
  probe `32712992421`; artifact records `unsupported_relations = 6962` and
  `unsupported_relation_disclosed = true`.
- Slice 2A landed on `658d53fb03dd47a971beb6cf67b46cfe1f20b3fe`; post-merge contexts are green,
  but focused module evidence is not claimed.
- Slice 2C landed on `b9e0eadc46175e15ec62f915a4729287f3884cd2`; Store Conformance
  `32718598218`, Rustok evaluation `32718598212`, and Rustok probe `32718598232` are green.
- Slice 3A landed on `0a4c0f78ef05b6c2ba9480b770c1ebf72038e049`; Rustok evaluation
  `32719989413` and Rustok probe `32719989450` are green. These are not focused API profile evidence.
- Slices 2B–2C, API Slices 3A–3C, operations Slices 4A–4C, and onboarding Slices 5A–5C remain
  execution-pending for focused gates.

The repaired bounded Rustok architecture-generation evaluation retains `DOCUMENTATION_REFERENCE_LIMIT`,
`workflow_dispatch` support, and diagnostic evidence checks as explicit regression boundaries.

## Next Bounded Step

1. Record formatting/build/test/Clippy evidence for module 2B–2C, API 3A–3C, operations 4A–4C, and onboarding 5A–5C.
2. Take the next deterministic documentation package from broader framework adapters/completeness reporting.
3. Keep provider, daemon, MCP, and coordinated `ath generate` changes out until separate deterministic
   quality gates justify them.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test documentation_onboarding_profile_inventory --locked
cargo test -p athanor-app --test documentation_onboarding_publication_inventory --locked
cargo test -p athanor-app --test documentation_onboarding_operation_inventory --locked
cargo test -p athanor-app --test documentation_onboarding_inspection_inventory --locked
cargo test -p athanor-app --test documentation_operations_profile_inventory --locked
cargo test -p athanor-app --test documentation_operations_publication_inventory --locked
cargo test -p athanor-app --test documentation_operations_operation_inventory --locked
cargo test -p athanor-app --test documentation_operations_inspection_inventory --locked
cargo test -p athanor-app --test documentation_api_profile_inventory --locked
cargo test -p athanor-app --test documentation_api_publication_inventory --locked
cargo test -p athanor-app --test documentation_api_operation_inventory --locked
cargo test -p athanor-app --test documentation_api_inspection_inventory --locked
cargo test -p athanor-app --test documentation_module_profile_inventory --locked
cargo test -p athanor-app --test documentation_module_publication_inventory --locked
cargo test -p athanor-app --test documentation_module_inspection_inventory --locked
cargo test -p athanor-app --test documentation_architecture_inspection_inventory --locked
cargo test -p ath --test documentation_onboarding_cli --locked
cargo test -p ath --test documentation_operations_cli --locked
cargo test -p ath --test documentation_api_cli --locked
cargo test -p ath --test documentation_module_cli --locked
cargo test -p ath --test documentation_architecture_cli --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```
