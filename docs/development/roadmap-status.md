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

Application services receive `RuntimeComposition`; store, search, projector, extractor, and transport
factories are selected by `ath`, `athd`, or MCP composition roots. Process-global installers are gone.

### Bounded Owners And Publication

Indexing uses bounded phases and staged publication. Documentation generation uses strict request,
manifest, outline, context, citation, draft, validation, and current-pointer contracts.

The deterministic architecture profile loads one **exact committed** canonical snapshot through the
configured Store, applies hard limits, emits cited Markdown and relation-backed Mermaid source, and
publishes immutable checksum-bound generations. Validated inspection rejects pointer escape, unsupported
artifact layouts, identity drift, and checksum drift.

Supported CLI surface:

```bash
ath docs generate-architecture <PATH> --snapshot <EXACT-ID> [--force] [--json]
ath docs architecture current <PATH> [--json]
ath docs architecture manifest <PATH> [--json]
ath docs architecture validation <PATH> [--json]
```

Generation has no latest-snapshot fallback. Ctrl-C cancels and drains the operation before returning.
The existing coordinated `ath generate` command is unchanged. Daemon, MCP, and provider integration are
not enabled.

### Exact Evidence

Released product baseline remains `609027eb02caa05346ebfea8538552c42b588c31` with CI
`29995959544`, AppSec `29995960063`, Store `29995959512`, release `29996579628`, and clean-install
smoke `29998347890`.

Slices 1A–1B are confirmed on `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`: CI `30013208011`,
AppSec `30013208197`, Store `30013208312`.

Slice 1C1 exact committed-snapshot loading is confirmed on
`4f567271ed6d38d30b3c15dc6999aa33152a9312`: CI `30015689753`, AppSec `30015691399`, Store
`30015689363`.

Slice 1C2 CLI generation and validated inspection are confirmed on
`042d02ac6b4c89d90a5b76c818098eb0c6b41920`: CI `30025932615`, AppSec `30025931953`, Store
`30025932704`.

The repaired bounded Rustok evaluation is confirmed on
`f1024cbc52f05de4d3ce96c556ef044ad48b3a0e`: evaluation `31625608720`, probe `31625608721`, CI
`31625608729`, AppSec `31625608723`, Store `31625608739`. The uploaded artifact is valid and
repeatable, but review found that `6962` unsupported relations were not explicitly disclosed in the
human-facing Markdown.

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
- [x] Slice 1C2: exact-snapshot CLI generation, hard-limit/force flags, text/JSON output, validated
  current/manifest/validation inspection, Ctrl-C cancellation, and executable round-trip tests;
- [x] Slice 1C exact evidence on `042d02ac6b4c89d90a5b76c818098eb0c6b41920`: CI `30025932615`,
  AppSec `30025931953`, Store `30025932704`;
- [x] Repaired aggregate citation selection and evaluation/probe workflows;
- [x] Exact bounded Rustok evaluation on `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e` with green
  evaluation/probe/CI/AppSec/Store statuses, `256` citations, `84` Mermaid edges, valid metrics, and
  deterministic `up_to_date` repeat;
- [x] Artifact review recorded the remaining tuning finding: `6962` omitted/unsupported relations are
  reported by validation but `unsupported_relation_disclosed = false` in the generated Markdown;
- [ ] Publish explicit unsupported-relation disclosure plus a fail-closed evaluation guard, then confirm
  `unsupported_relation_disclosed = true` on one exact source commit.

`DOCGEN-001` remains in progress only until that artifact-review finding is closed with exact-commit
evidence. Additional profiles stay out of this bounded fix.

## Product Backlog

- module/API/operations/onboarding profiles;
- broader framework adapters and completeness reporting;
- i18n, concepts, and optional semantic/vector retrieval;
- optional provider, daemon, and MCP integration after deterministic quality gates.
