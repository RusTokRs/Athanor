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

The architecture profile loads one exact committed canonical snapshot through the configured Store,
applies hard limits, emits cited Markdown/Mermaid output, and publishes immutable checksum-bound
generations. Its CLI and validated inspection are execution-confirmed.

The module profile is source-implemented through Slice 2C: pure inventory, module-scoped evidence,
immutable publication, exact Store loading, CLI generation, and validated inspection.

The API profile is source-implemented through Slice 3C. Slice 3A established the pure exact-snapshot
endpoint/schema/example inventory; Slice 3B added API-scoped facts, supported canonical relations, open
diagnostics, deterministic aggregate selection, cited Mermaid, and explicit omissions; Slice 3C adds
immutable `api/index.md` publication, exact Store loading, CLI generation, and validated inspection.

The shared `current.json` is profile-aware across architecture, module, and API inspection: each surface
fails closed when the pointer belongs to another profile.

Supported deterministic documentation CLI surface is now:

```bash
ath docs generate-architecture <PATH> --snapshot <EXACT-ID> [--force] [--json]
ath docs architecture current|manifest|validation <PATH> [--json]
ath docs generate-module <PATH> --snapshot <EXACT-ID> [--force] [--json]
ath docs module current|manifest|validation <PATH> [--json]
ath docs generate-api <PATH> --snapshot <EXACT-ID> [--force] [--json]
ath docs api current|manifest|validation <PATH> [--json]
```

All generation surfaces retain hard-limit flags. Generation has no latest-snapshot fallback. Ctrl-C
cancels and drains the operation before returning. The existing coordinated `ath generate` command is unchanged.

### Exact Evidence

Released product baseline remains `609027eb02caa05346ebfea8538552c42b588c31` with CI
`29995959544`, AppSec `29995960063`, Store `29995959512`, release `29996579628`, and clean-install
smoke `29998347890`.

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

Slice 2A landed on `658d53fb03dd47a971beb6cf67b46cfe1f20b3fe`; its post-merge Rustok evaluation,
probe, AppSec, and Store contexts are green. Slice 2C landed on
`b9e0eadc46175e15ec62f915a4729287f3884cd2`; Store Conformance `32718598218`, Rustok evaluation
`32718598212`, and Rustok probe `32718598232` are green. Slice 3A landed on
`0a4c0f78ef05b6c2ba9480b770c1ebf72038e049`; Rustok evaluation `32719989413` and Rustok probe
`32719989450` are green. These do not substitute for focused module/API format/test/Clippy evidence.

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

- [x] Slice 0A: strict request and manifest;
- [x] Slice 0B: bounded evidence flow, policy, metrics, and evaluation corpus;
- [x] Slice 1A: deterministic architecture composition;
- [x] Slice 1B: immutable atomic publication and tamper recovery;
- [x] Slice 1C1: exact committed-snapshot loading through `RuntimeComposition`;
- [x] Slice 1C2: exact-snapshot architecture CLI and validated inspection;
- [x] Repaired aggregate citation selection and exact Rustok evaluation;
- [x] Closed the human-facing unsupported-relation disclosure finding;
- [x] Slice 2A: deterministic module inventory with cited `modules/index.md` and SHA-256;
- [x] Slice 2B: module-scoped facts, relations, open diagnostics, Mermaid, omissions, parity regressions;
- [x] Slice 2C: immutable module publication, exact Store operation, module CLI and validated inspection;
- [x] Slice 3A: exact pure API endpoint/schema/example inventory, deterministic round-robin, citations,
  portable evidence normalization, omission accounting, and checksum binding;
- [x] Slice 3B: API-scoped facts, supported canonical API relation allowlist, open diagnostics, shared
  evidence normalization, aggregate limits, cited Mermaid, omissions, and scoped regressions;
- [x] Slice 3C implemented in source: immutable API publication, exact committed-snapshot Store operation,
  `generate-api`, validated `api current|manifest|validation`, profile-isolated shared current pointer,
  and publication/operation/inspection/binary-CLI regressions;
- [ ] Focused module Slices 2B–2C and API Slices 3A–3C execution evidence remains pending.

`DOCGEN-001` remains in progress. Architecture is execution-confirmed; module/API deterministic production
surfaces are source-implemented but are not promoted to verified without their focused execution gates.

## Product Backlog

- operations/onboarding documentation profiles;
- broader framework adapters and completeness reporting;
- i18n, concepts, and optional semantic/vector retrieval;
- optional provider, daemon, and MCP integration after deterministic quality gates.
