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
4C, and onboarding through Slice 5B. Focused module, API, operations, and onboarding format/test/Clippy
evidence remains pending and is not inferred from source presence.

The existing coordinated `ath generate` command is unchanged. No model provider, daemon, MCP, or new dependency is enabled.

```text
source files -> adapters -> exact committed snapshot -> bounded context -> cited document -> immutable generation
```

## Non-Negotiable Boundaries

- The canonical snapshot is the only source for factual claims.
- Every request names an exact committed snapshot and hard limits; there is no latest fallback.
- Claims and diagram edges carry in-scope stable keys/evidence or an explicit inference marker.
- Omitted counts are visible, including unrepresented supported relations.
- Secrets/raw-file explorer access are forbidden; network/provider use is opt-in and currently absent.
- Invalid, tampered, or cancelled publication cannot advance `current.json`.
- New profiles start as pure deterministic owners before publication or transport integration.
- Portable evidence normalization has one shared owner; profile code must not duplicate path rules.

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

### Slices 1A–1C — Architecture Production Surface

The architecture profile consumes one explicit `CanonicalSnapshot`, emits deterministic cited Markdown
and Mermaid, publishes an immutable generation, loads only the exact committed snapshot through the
configured Store, and exposes validated CLI inspection.

```bash
ath docs generate-architecture <PATH> --snapshot <EXACT-ID> \
  [--max-entities N] [--max-facts N] [--max-relations N] [--max-diagnostics N] [--force] [--json]
ath docs architecture current <PATH> [--json]
ath docs architecture manifest <PATH> [--json]
ath docs architecture validation <PATH> [--json]
```

Slice 1C1 is exact Store operation wiring; Slice 1C2 is CLI generation and inspection. Generation drains
after Ctrl-C cancellation, and inspection rejects path escape, wrong profile, identity/layout drift,
invalid validation status, and checksum drift.

### Slices 2A–2C — Module Documentation Profile

Pure module inventory expanded to module-scoped facts/relations/open diagnostics with the aggregate
256-item budget, then immutable publication, exact Store loading, CLI generation, and validated inspection.
Focused module execution evidence is pending.

### Slices 3A–3C — API Documentation Profile

Endpoint/schema/example inventory expanded to API-scoped facts, supported canonical relations, open
diagnostics, aggregate 256-item budgeting, and cited Mermaid, then immutable publication, exact Store,
CLI, and validated inspection. Focused API execution evidence is pending.

### Slices 4A–4C — Operations Documentation Profile

Six-category operations inventory expanded to operations-scoped facts, supported relations, open
diagnostics, cited Mermaid, and aggregate `DOCUMENTATION_REFERENCE_LIMIT = 256`, then immutable
`operations/index.md` publication, exact Store loading, CLI generation, and validated inspection.
Existing `ath docs operations check` remains unchanged. Focused operations execution evidence is pending.

### Slices 5A–5B — Onboarding Documentation Profile

- Slice 5A established the pure exact-snapshot onboarding inventory over evidence-backed
  `DocumentationPage`, `DocumentationSection`, `Package`, `ScriptCommand`, `EnvVar`, and
  `TestCase`/`CiJob` anchors with deterministic six-category round-robin selection.
- Slice 5B adds facts when subject or object is a selected onboarding anchor.
- Supported relations are deliberately bounded to `Contains`, `Documents`, `UsesEnv`, and `TestedBy`;
  each relation must touch a selected anchor and carry portable evidence. General call/import/change graph
  relations do not leak into onboarding documentation.
- Only open diagnostics referencing a selected onboarding anchor enter the context.
- Per-kind limits are applied before deterministic aggregate round-robin across Entity/Fact/Relation/
  Diagnostic, capped by `DOCUMENTATION_REFERENCE_LIMIT = 256`.
- Every context item owns one citation. Supported relations produce cited Mermaid edges. Omitted supported
  relations are disclosed and feed `validation.metrics.unsupported_relations`.
- `onboarding/index.md` remains deterministic and lowercase-SHA-256-bound. Rendering/validation is split
  into an onboarding-specific sibling module; shared evidence/path normalization remains the common owner.
- Source regressions cover unrelated/unsupported leakage, resolved diagnostics, input-order invariance,
  relation-limit disclosure, portable evidence, and a mixed-kind 256-item budget.
- Publication, Store loading, CLI, inspection, daemon, MCP, provider/LLM, and coordinated generation remain
  outside Slice 5B. Focused onboarding execution evidence is pending.

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
- Slice 2A landed on `658d53fb03dd47a971beb6cf67b46cfe1f20b3fe`; later module contexts are green
  but focused module evidence is not claimed.
- Slice 2C landed on `b9e0eadc46175e15ec62f915a4729287f3884cd2`; Store `32718598218`, Rustok
  evaluation `32718598212`, and Rustok probe `32718598232` were green.
- Slice 3A landed on `0a4c0f78ef05b6c2ba9480b770c1ebf72038e049`; Rustok evaluation
  `32719989413` and Rustok probe `32719989450` were green.
- Module 2B–2C, API 3A–3C, operations 4A–4C, and onboarding 5A–5B remain execution-pending for focused gates.

The repaired bounded Rustok architecture-generation evaluation retains `DOCUMENTATION_REFERENCE_LIMIT`,
`workflow_dispatch` support, and diagnostic evidence checks as explicit regression boundaries.

## Next Bounded Step

1. Record focused format/test/Clippy evidence for the source-implemented profiles.
2. Implement Slice 5C as immutable onboarding publication, exact Store operation, CLI generation, and
   validated inspection with profile-isolated shared `current.json`.
3. Keep provider, daemon, MCP, and coordinated `ath generate` changes out until separate deterministic
   quality gates justify them.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test documentation_onboarding_profile_inventory --locked
cargo test -p athanor-app --test documentation_operations_profile_inventory --locked
cargo test -p athanor-app --test documentation_api_profile_inventory --locked
cargo test -p athanor-app --test documentation_module_profile_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p ath --test documentation_architecture_cli --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```
