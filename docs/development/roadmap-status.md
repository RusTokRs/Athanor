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
- [x] Attempted the first bounded Rustok architecture-generation evaluation on
  `5e0b28099c48e22bdc172fa57b6d51db9e6efb7b`,
  run `30029451096`, against `RusTokRs/RusTok` `3b76a530c82c0faaed22ccec384d4fd950811862`.
  Indexing completed, but generation failed because the draft contained more than the contract maximum of
  256 citations. The exact error was `documentation draft citations must contain between 1 and 256 entries`.
- [x] Reproduced the failure in an isolated local checkout. Reducing the aggregate selected context to
  `64` entities, `64` facts, `64` relations, and `32` diagnostics produced a valid document and a
  deterministic `up_to_date` repeat; this diagnostic reproduction is not matrix evidence.
- [x] Made context selection respect the shared `DOCUMENTATION_REFERENCE_LIMIT` of `256` cited items
  with deterministic round-robin allocation across kinds, correct omission counts, and Markdown budget
  disclosure. A rich `100/100/100/1` fixture proves full-budget, valid, order-invariant selection.
- [x] Repaired the evaluation and probe workflows: JSON stdout and diagnostics are separated, status
  endpoints use environment variables rather than direct GitHub-expression expansion, failures still
  publish sanitized outputs, and `workflow_dispatch` makes the bounded run rerunnable. Both files pass
  local `actionlint` and `zizmor 1.26.1`.
- [x] Repeated the pinned evaluation locally in an isolated checkout on 2026-08-01 with the original
  `96/192/192/96` caps: `snap_jsonl_00000001`, published generation `00000001`, `256` citations,
  valid 10,000-bp metrics, all required sections, and deterministic `up_to_date`. This is local
  diagnostic evidence, not a committed matrix.
- [ ] Publish the repaired source, dispatch the Rustok evaluation workflows, record artifact review and
  tuning decisions, then obtain the full exact-commit CI/AppSec/Store verification matrix.

`DOCGEN-001` remains in progress until the repaired GitHub bounded Rustok evaluation records usefulness,
omissions, unsupported relations, repeatability, artifact review findings, and exact-commit matrix
evidence before profile expansion.

## Product Backlog

- module/API/operations/onboarding profiles;
- broader framework adapters and completeness reporting;
- i18n, concepts, and optional semantic/vector retrieval;
- optional provider, daemon, and MCP integration after deterministic quality gates.
