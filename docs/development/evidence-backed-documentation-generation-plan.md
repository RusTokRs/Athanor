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

Module expansion is source-implemented through Slice 2C. Slice 2A established a pure module inventory,
Slice 2B added module-scoped facts/relations/open diagnostics, and Slice 2C adds immutable publication,
exact Store loading, CLI generation, and validated inspection. Focused module format/test/Clippy evidence
remains pending and is not inferred from source presence.

The existing coordinated `ath generate` command is unchanged. No model provider, daemon, MCP, or new
dependency is enabled.

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
- Editable documentation remains an explicit patch workflow.
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

`DocumentationProfile` contains `architecture` and `module`; the v1 schemas remain unchanged.

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

The operation resolves the project, initializes the configured Store, loads exactly
`SnapshotId(request.snapshot)` through `CanonicalSnapshotStore`, verifies identity, checks cancellation,
and delegates publication. Supported architecture commands are:

```bash
ath docs generate-architecture <PATH> --snapshot <EXACT-ID> \
  [--max-entities N] [--max-facts N] [--max-relations N] [--max-diagnostics N] \
  [--force] [--json]
ath docs architecture current <PATH> [--json]
ath docs architecture manifest <PATH> [--json]
ath docs architecture validation <PATH> [--json]
```

Generation drains after Ctrl-C cancellation. Inspection rejects non-normalized pointers, path escape,
wrong profile, unsupported artifact layouts, snapshot drift, invalid validation status, and checksum drift.

### Slice 2A — Deterministic Module Inventory Profile

`build_documentation_module_profile` is a pure owner for `DocumentationProfile::Module`:

- exact snapshot/profile identity is mandatory;
- only evidence-backed `EntityKind::Module` candidates are selected;
- stable key + entity id ordering makes canonical input ordering irrelevant;
- `max_entities` and `DOCUMENTATION_REFERENCE_LIMIT = 256` bound context/citations;
- each selected module owns a citation;
- deterministic `modules/index.md` is SHA-256 bound;
- wrong profile/snapshot or missing evidence-backed modules fails closed.

### Slice 2B — Module-Scoped Evidence Enrichment

- Fact scope: subject or object is a selected module.
- Relation scope: source or target is a selected module.
- Diagnostic scope: status is open and it references a selected module.
- Evidence/ownership path normalization, line defaults, deduplication, and ordering semantics match the
  architecture profile; a semantic-parity regression prevents silent drift.
- Per-kind limits are applied before the deterministic aggregate 256-item round-robin budget.
- Facts render with modules; relations render cited claims and Mermaid edges; diagnostics render cited
  open findings.
- Omitted module/fact/relation/diagnostic counts are explicit and `unsupported_relations` reflects
  omitted module-scoped relation candidates.
- Unrelated canonical items cannot leak into module scope.

### Slice 2C — Immutable Module Publication, Store Operation, CLI, And Inspection

Module generation reuses the existing versioned request/manifest/current contracts and shared immutable
generation root without a schema migration:

```text
.athanor/generated/documentation/
  current.json                       # profile-aware pointer
  generations/<8-digit-generation>/
    manifest.json
    modules/index.md
    validation-report.json
```

- `publish_documentation_module_generation` builds the pure module profile, writes a staged immutable
  generation, binds Markdown/report checksums, and advances `current.json` only after publication.
- Exact `UpToDate`, `--force`, immutable history, tamper recovery, and cooperative cancellation mirror the
  established architecture lifecycle.
- The shared pointer is intentionally profile-aware: module inspection rejects architecture current and
  architecture inspection rejects module current before returning unchecked data.
- `generate_documentation_module_with_composition` initializes the configured Store and loads only the
  exact committed requested snapshot; missing/uncommitted or mismatched identities fail closed.
- Supported module commands are:

```bash
ath docs generate-module <PATH> --snapshot <EXACT-ID> \
  [--max-entities N] [--max-facts N] [--max-relations N] [--max-diagnostics N] \
  [--force] [--json]
ath docs module current <PATH> [--json]
ath docs module manifest <PATH> [--json]
ath docs module validation <PATH> [--json]
```

Focused source regressions cover immutable publication/reuse, tamper repair, cancellation, deterministic
content, profile isolation, validated current/manifest/report inspection, and executable index → module
generate → inspect → `up_to_date` plus missing snapshot. Execution evidence is still pending.

## Execution Evidence

- Slices 0A–0B: source `2a049303e797f00ac53f1e91fc010f284993926d`; CI `30005828864`,
  AppSec `30005828850`, Store `30005828956`.
- Slices 1A–1B: source `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`; CI `30013208011`,
  AppSec `30013208197`, Store `30013208312`.
- Slice 1C1: source `4f567271ed6d38d30b3c15dc6999aa33152a9312`; CI `30015689753`, AppSec
  `30015691399`, Store `30015689363`.
- Slice 1C2: source `042d02ac6b4c89d90a5b76c818098eb0c6b41920`; CI `30025932615`, AppSec
  `30025931953`, Store `30025932704`.
- First Rustok attempt: source `5e0b28099c48e22bdc172fa57b6d51db9e6efb7b`, run `30029451096`;
  indexing completed but generation exposed the original 256-citation defect.
- Repaired evaluation: source `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e`; evaluation `31625608720`,
  probe `31625608721`, CI `31625608729`, AppSec `31625608723`, Store `31625608739`.
- Relation disclosure: source `6862aee81dd0f53fa8372d1ce3fcb6e2ed198cca`; evaluation `32712992516`,
  probe `32712992421`; artifact records `unsupported_relations = 6962` and
  `unsupported_relation_disclosed = true`.
- Slice 2A landed on `658d53fb03dd47a971beb6cf67b46cfe1f20b3fe`; post-merge Rustok evaluation,
  probe, AppSec, and Store contexts are green. Focused module evidence is not claimed.
- Slices 2B–2C: source implementations and regressions are present; focused execution evidence pending.

## Next Bounded Step

1. Record formatting/build/test/Clippy evidence for module Slices 2B–2C, including module profile,
   publication, inspection, and binary CLI regressions.
2. Then begin an API documentation profile as a new pure deterministic package before publication.
3. Keep provider, daemon, MCP, operations/onboarding profiles, and coordinated `ath generate` changes out
   of that package unless separately approved.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test documentation_module_profile_inventory --locked
cargo test -p athanor-app --test documentation_module_publication_inventory --locked
cargo test -p athanor-app --test documentation_module_inspection_inventory --locked
cargo test -p athanor-app --test documentation_architecture_inspection_inventory --locked
cargo test -p ath --test documentation_module_cli --locked
cargo test -p ath --test documentation_architecture_cli --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```
