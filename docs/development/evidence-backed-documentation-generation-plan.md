---
id: doc://docs/development/evidence-backed-documentation-generation-plan.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Evidence-Backed Documentation Generation Plan

## Status

In progress. Slices 0A–1C are execution-confirmed. The supported exact-snapshot CLI generation and
validated inspection surface has passed the full cross-platform matrix. The package remains active only
for the first bounded Rustok generation evaluation. The existing coordinated `ath generate` command is
unchanged. No model provider, daemon, MCP, or new dependency is enabled.

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

## Implemented Contracts

- `athanor.documentation_generation_request.v1`
- `athanor.documentation_generation_manifest.v1`
- `athanor.documentation_outline.v1`
- `athanor.documentation_context.v1`
- `athanor.documentation_citation.v1`
- `athanor.documentation_draft.v1`
- `athanor.documentation_validation_report.v1`
- `athanor.documentation_current.v1`

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

Supported commands:

```bash
ath docs generate-architecture <PATH> --snapshot <EXACT-ID> \
  [--max-entities N] [--max-facts N] [--max-relations N] [--max-diagnostics N] \
  [--force] [--json]
ath docs architecture current <PATH> [--json]
ath docs architecture manifest <PATH> [--json]
ath docs architecture validation <PATH> [--json]
```

Generation uses production `RuntimeComposition` and drains after Ctrl-C cancellation. Inspection never
prints unchecked files: it rejects non-normalized pointers, path escape, unsupported artifact layouts,
snapshot/profile drift, invalid validation status, and checksum mismatch.

Executable regression coverage indexes a real temporary project, captures the exact snapshot ID,
generates and inspects all outputs, verifies repeated `up_to_date`, and rejects a missing snapshot.

## Execution Evidence

- Slices 0A–0B: source `2a049303e797f00ac53f1e91fc010f284993926d`; CI `30005828864`,
  AppSec `30005828850`, Store `30005828956`.
- Slices 1A–1B: source `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`; CI `30013208011`,
  AppSec `30013208197`, Store `30013208312`.
- Slice 1C1: source `4f567271ed6d38d30b3c15dc6999aa33152a9312`; CI `30015689753`,
  AppSec `30015691399`, Store `30015689363`.
- Slice 1C2: source `042d02ac6b4c89d90a5b76c818098eb0c6b41920`; CI `30025932615`,
  AppSec `30025931953`, Store `30025932704`.

The Slice 1C2 matrix covered formatting, workspace tests, Clippy, Linux/macOS/Windows smoke, installers,
default/store-surreal/js-ts-precision/all-features, cargo-deny, documentation checks, and source coverage.

## Next Bounded Step

Run the architecture command against one bounded Rustok snapshot and record:

- useful and missing sections;
- citation and diagram validity;
- omitted counts and unsupported relations;
- deterministic repeatability;
- review findings and tuning decisions.

Provider, daemon, MCP, additional profiles, and changes to coordinated `ath generate` remain later work.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
cargo test -p athanor-app --test documentation_architecture_publication_inventory --locked
cargo test -p athanor-app --test documentation_architecture_inspection_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p ath --test documentation_architecture_cli --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```
