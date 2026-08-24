---
id: doc://docs/development/evidence-backed-documentation-generation-plan.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Evidence-Backed Documentation Generation Plan

## Status

In progress. Slices 0A–1C are execution-confirmed. The supported exact-snapshot architecture CLI and
validated inspection surface has passed the full cross-platform matrix. The bounded Rustok evaluation
repair is fully matrix-confirmed on `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e`; the follow-up
unsupported-relation disclosure is exact-evaluation-confirmed on
`6862aee81dd0f53fa8372d1ce3fcb6e2ed198cca`.

Module profile expansion is now source-implemented through Slice 2B. Slice 2A established a pure
source-backed module inventory. Slice 2B adds module-scoped facts, relations, open diagnostics, Mermaid
edges, scoped omission accounting, and focused semantic-parity regressions. Execution evidence for the
module slices remains pending and is not inferred from source presence.

The existing coordinated `ath generate` command is unchanged. No model provider, daemon, MCP, module
publication, Store-loading operation, CLI command, or new dependency is enabled.

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
- Inspection validates path confinement, identities, exact artifact layout, and checksums.
- Editable documentation remains an explicit patch workflow.
- New profiles are introduced as pure deterministic owners before publication or transport integration.

## Implemented Contracts

- `athanor.documentation_generation_request.v1`
- `athanor.documentation_generation_manifest.v1`
- `athanor.documentation_outline.v1`
- `athanor.documentation_context.v1`
- `athanor.documentation_citation.v1`
- `athanor.documentation_draft.v1`
- `athanor.documentation_validation_report.v1`
- `athanor.documentation_current.v1`

`DocumentationProfile` contains the backward-compatible serialized variants `architecture` and `module`;
the request and downstream v1 schemas remain unchanged.

No reference becomes a dependency without license, maintenance, MSRV, security, adapter-boundary,
fixture-comparison, and replacement analysis.

## Implemented Slices

### Slices 0A–0B — Contracts And Evidence Flow

Strict schemas, hard limits, omissions, portable paths, SHA-256, data policy, quality metrics, fixture
repository, Rustok evaluation corpus, and full-chain fail-closed alignment.

### Slice 1A — Deterministic Architecture Profile

`build_documentation_architecture_profile` consumes one explicit `CanonicalSnapshot`, sorts canonical
objects, applies limits, and emits Overview, Components, Relationships, Diagnostics, cited claims,
Mermaid source, evidence footnotes, validation metrics, and Markdown SHA-256 without filesystem, store,
network, or provider access.

### Slice 1B — Immutable Publication

```text
.athanor/generated/documentation/
  current.json
  generations/<8-digit-generation>/
    manifest.json
    architecture/index.md
    validation-report.json
```

Publication is staged and immutable. `UpToDate` requires the exact expected artifact IDs, paths, media
types, identities, and deterministic hashes. Force, tamper recovery, history retention, and cancellation
are regression protected.

### Slice 1C1 — Exact Committed-Snapshot Operation

The composition-aware operation validates the request, resolves the canonical root, initializes the
configured Store, loads `SnapshotId(request.snapshot)` through `CanonicalSnapshotStore`, verifies the
returned identity, checks cancellation around the Store boundary, and delegates publication. Missing or
uncommitted snapshots fail without creating documentation output.

### Slice 1C2 — CLI And Validated Inspection

Supported commands remain:

```bash
ath docs generate-architecture <PATH> --snapshot <EXACT-ID> \
  [--max-entities N] [--max-facts N] [--max-relations N] [--max-diagnostics N] \
  [--force] [--json]
ath docs architecture current <PATH> [--json]
ath docs architecture manifest <PATH> [--json]
ath docs architecture validation <PATH> [--json]
```

Generation uses production `RuntimeComposition` and drains after Ctrl-C cancellation. Inspection rejects
non-normalized pointers, path escape, unsupported artifact layouts, snapshot/profile drift, invalid
validation status, and checksum mismatch.

### Slice 2A — Deterministic Module Inventory Profile

`build_documentation_module_profile` is a pure application owner for `DocumentationProfile::Module`:

- exact snapshot/profile identity is mandatory;
- only evidence-backed `EntityKind::Module` candidates are selected;
- stable key + entity id ordering makes canonical input ordering irrelevant;
- `max_entities` and the shared `DOCUMENTATION_REFERENCE_LIMIT = 256` bound the context;
- each selected module owns a citation;
- deterministic `modules/index.md` is SHA-256 bound;
- provider/network/raw-file/secrets remain disabled;
- wrong profile, wrong snapshot, or absence of evidence-backed modules fails closed.

### Slice 2B — Module-Scoped Evidence Enrichment

Slice 2B keeps the same pure owner boundary and enriches only the bounded module context:

- a fact is in scope when its subject or object is one of the selected canonical modules;
- a relation is in scope when its source or target is one of the selected canonical modules;
- a diagnostic is in scope only when it is open and references a selected canonical module;
- facts, relations, and diagnostics require canonical stable-key resolution plus evidence or ownership
  locations using the same portable-path, line-defaulting, deduplication, and deterministic ordering
  semantics established by the architecture profile;
- per-kind limits are applied before a deterministic round-robin aggregate budget of 256 context items;
- facts are rendered with modules, relations get cited relationship claims and Mermaid edges, and open
  diagnostics get cited diagnostic claims;
- omissions are reported for the module-scoped bounded context and `unsupported_relations` reflects
  omitted module-scoped relation candidates;
- human-facing Markdown explicitly states the Slice 2B boundary and omitted module/fact/relation/
  diagnostic counts;
- unrelated canonical facts/relations/diagnostics cannot leak into the selected module scope.

The focused regression suite now checks exact scope, stable ordering, input-order invariance, relation
endpoint identity, citation/checksum binding, fail-closed cases, and semantic parity with the architecture
profile for corresponding entity/fact/relation/diagnostic stable keys and evidence locations. The parity
regression prevents silent evidence-normalization drift without broad churn in the already-green
architecture owner.

Slice 2B still does **not** publish files or load a Store. Module publication, exact Store operation, CLI,
validated inspection, daemon, MCP, provider, and coordinated `ath generate` integration remain closed.

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
  `30029451096`; indexing completed but architecture generation hit the original 256-citation defect.
- Repaired bounded evaluation: source `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e`; evaluation
  `31625608720`, probe `31625608721`, CI `31625608729`, AppSec `31625608723`, Store `31625608739`.
- Relation-disclosure tuning: source `6862aee81dd0f53fa8372d1ce3fcb6e2ed198cca`; evaluation
  `32712992516`, probe `32712992421`; artifact records `unsupported_relations = 6962`,
  `unsupported_relation_disclosed = true`, and deterministic `up_to_date` repeatability.
- Slice 2A landed on `658d53fb03dd47a971beb6cf67b46cfe1f20b3fe`; post-merge contexts are green for
  Rustok evaluation `32714912841`, Rustok probe `32714912826`, AppSec `32714912809`, and Store
  Conformance `32714912932`. Focused format/test/Clippy evidence is not claimed.
- Slice 2B source implementation and regressions are present; execution evidence remains pending.

## Rustok Evaluation Gate

The architecture profile applies the shared `DOCUMENTATION_REFERENCE_LIMIT` of `256` context
items/citations after requested per-kind caps. Its deterministic round-robin selector preserves every
represented kind while capacity remains. The exact Rustok runs above prove repaired architecture output
and the human-facing unsupported-relation disclosure.

The module profile now applies the same aggregate ceiling to its own module-scoped kinds. It remains a
pure source-level profile until Slice 2C adds a production publication/operation path.

## Next Bounded Step

1. Obtain formatting/build/test/Clippy execution evidence for the enriched Slice 2B module profile,
   including `documentation_module_profile_inventory`.
2. After that quality gate, implement Slice 2C: immutable module publication, exact committed-snapshot
   Store loading, and validated module inspection/CLI as one separate package.
3. Keep provider, daemon, MCP, API/operations/onboarding profiles, and coordinated `ath generate` changes
   out of Slice 2C unless a separate contract migration is approved.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
cargo test -p athanor-app --test documentation_module_profile_inventory --locked
cargo test -p athanor-app --test documentation_architecture_publication_inventory --locked
cargo test -p athanor-app --test documentation_architecture_inspection_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p ath --test documentation_architecture_cli --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```
