---
id: doc://docs/development/roadmap-status.md
kind: developer_guide
language: en
source_language: en
status: active
---
# Roadmap Status

This compact ledger describes current `main`. Detailed evidence lives in
`athanor_implementation_plan_ru.md`; long-range work lives in `start.md`.

## Status Rules

- **Implemented** means code, documentation, and source regressions are present.
- **Verified** means the required format/build/test/Clippy/smoke/security/feature/coverage matrix
  succeeded on one exact source commit.
- Documentation metadata is not execution evidence.

## Current Architecture

### Explicit Runtime Composition

Application services receive `RuntimeComposition`; Store, search, projector, extractor, and transport
factories are selected by `ath`, `athd`, or MCP composition roots. Process-global installers are gone.

### Bounded Owners And Publication

Indexing uses bounded phases and staged publication. Documentation generation uses strict request,
manifest, outline, context, citation, draft, validation, and current-pointer contracts.

Architecture documentation is execution-confirmed through Slice 1C2: exact committed-snapshot Store
loading, immutable checksum-bound publication, CLI generation, and validated inspection.

Module, API, and operations profiles are source-implemented through Slices 2C, 3C, and 4C respectively.
Their production surfaces use deterministic bounded context, immutable publication, exact Store loading,
CLI generation, validated inspection, and profile-aware shared `current.json`.

Onboarding is source-implemented through Slice 5B as a pure exact-snapshot profile. Anchor inventory is
`DocumentationPage`, `DocumentationSection`, `Package`, `ScriptCommand`, `EnvVar`, and `TestCase`/`CiJob`
with deterministic six-category round-robin selection. Facts are scoped when subject or object is a
selected anchor. Supported relations are `Contains`, `Documents`, `UsesEnv`, and `TestedBy`; relations must
touch a selected anchor. Only open diagnostics referencing an anchor enter the context.

Onboarding applies per-kind hard limits followed by deterministic aggregate Entity/Fact/Relation/Diagnostic
round-robin under `DOCUMENTATION_REFERENCE_LIMIT = 256`. Every context item is cited; supported relations
produce cited Mermaid edges, and omitted supported relations are explicitly disclosed. Shared portable
evidence normalization remains the single path/evidence owner. `onboarding/index.md` is deterministic and
SHA-256-bound. Publication, Store, CLI, and inspection remain Slice 5C work.

The shared `current.json` remains profile-aware across architecture, module, API, and operations. Onboarding
does not join that publication lifecycle until Slice 5C. Existing inspectors fail closed on another profile.

Supported deterministic documentation CLI surface remains:

```bash
ath docs generate-architecture <PATH> --snapshot <EXACT-ID> [--force] [--json]
ath docs architecture current|manifest|validation <PATH> [--json]
ath docs generate-module <PATH> --snapshot <EXACT-ID> [--force] [--json]
ath docs module current|manifest|validation <PATH> [--json]
ath docs generate-api <PATH> --snapshot <EXACT-ID> [--force] [--json]
ath docs api current|manifest|validation <PATH> [--json]
ath docs generate-operations <PATH> --snapshot <EXACT-ID> [--force] [--json]
ath docs operations current|manifest|validation <PATH> [--json]
ath docs operations check [--path <PATH>] [--json]
```

All generation surfaces retain hard-limit flags and have no latest-snapshot fallback. Ctrl-C cancels and
drains generation. The existing coordinated `ath generate` command is unchanged.

### Exact Evidence

Released baseline remains `609027eb02caa05346ebfea8538552c42b588c31`: CI `29995959544`, AppSec
`29995960063`, Store `29995959512`, release `29996579628`, clean-install smoke `29998347890`.

Slices 1A–1B are confirmed on `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`: CI `30013208011`,
AppSec `30013208197`, Store `30013208312`.

Slice 1C1 exact Store loading is confirmed on `4f567271ed6d38d30b3c15dc6999aa33152a9312`:
CI `30015689753`, AppSec `30015691399`, Store `30015689363`.

Slice 1C2 CLI generation and inspection are confirmed on
`042d02ac6b4c89d90a5b76c818098eb0c6b41920`: CI `30025932615`, AppSec `30025931953`, Store
`30025932704`.

The first bounded Rustok architecture-generation evaluation on
`5e0b28099c48e22bdc172fa57b6d51db9e6efb7b`, run `30029451096`, failed with
`documentation draft citations must contain between 1 and 256 entries`; this remains failure evidence.

The repaired bounded Rustok architecture-generation evaluation is matrix-confirmed on
`f1024cbc52f05de4d3ce96c556ef044ad48b3a0e`: evaluation `31625608720`, probe `31625608721`, CI
`31625608729`, AppSec `31625608723`, Store `31625608739`. The gate retains
`DOCUMENTATION_REFERENCE_LIMIT`, `workflow_dispatch`, and diagnostic evidence checks.

Relation-disclosure tuning is evaluation-confirmed on
`6862aee81dd0f53fa8372d1ce3fcb6e2ed198cca`: evaluation `32712992516`, probe `32712992421`,
`unsupported_relations = 6962`, `unsupported_relation_disclosed = true`.

Slice 2A landed on `658d53fb03dd47a971beb6cf67b46cfe1f20b3fe`; Slice 2C landed on
`b9e0eadc46175e15ec62f915a4729287f3884cd2`. Slice 3A landed on
`0a4c0f78ef05b6c2ba9480b770c1ebf72038e049`. Their later green generic contexts do not substitute for
focused module/API/operations/onboarding execution evidence.

## Implemented Packages

- `COMP-003` / `COMP-003C2B2C2B`: explicit composition and bounded owners.
- `MCP-007`: cancellation-safe transactional Index publication.
- `JSON-003`: recursive, disjoint, fixture-backed contract lifecycle.
- `DOC-001` / `DOC-002`: status hygiene and bounded architecture documents.
- `MCP-004`: responsive control input under saturation.
- `VERIFY-001`: exact cross-platform CI/AppSec/Store evidence.
- `API-001`: verified GraphQL/OpenAPI request/response/security consistency.
- `REL-001`: verified immutable `v0.2.1` release and clean installs.

## Active Work

### `DOCGEN-001` — Evidence-Backed Documentation Generation

- [x] Slices 0A–1C: contracts, architecture profile, immutable publication, exact Store, CLI/inspection;
- [x] repaired aggregate citation selection, exact Rustok evaluation, and relation disclosure;
- [x] Slices 2A–2C: module inventory/evidence/publication/Store/CLI/inspection;
- [x] Slices 3A–3C: API inventory/evidence/publication/Store/CLI/inspection;
- [x] Slices 4A–4C: operations inventory/evidence/publication/exact Store/CLI/inspection;
- [x] Slice 5A: pure onboarding inventory, six-category selection, citations, omissions, checksum binding;
- [x] Slice 5B: onboarding-scoped facts, bounded `Contains`/`Documents`/`UsesEnv`/`TestedBy` relations,
  open diagnostics, cited Mermaid, relation disclosure, mixed-kind aggregate 256 budget, source regressions;
- [ ] Focused module 2B–2C, API 3A–3C, operations 4A–4C, and onboarding 5A–5B execution evidence pending.

`DOCGEN-001` remains in progress. Architecture is execution-confirmed; module/API/operations production
surfaces and onboarding 5A–5B are source-implemented but are not promoted to verified without focused gates.

## Product Backlog

- Slice 5C: immutable onboarding publication, exact Store operation, CLI, validated inspection;
- broader framework adapters and completeness reporting;
- optional i18n, semantic/vector retrieval, provider, daemon, and MCP integration after quality gates.
