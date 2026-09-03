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

The API profile is source-implemented through Slice 3C: endpoint/schema/example inventory, scoped facts,
supported canonical relations, open diagnostics, immutable publication, exact Store loading, CLI, and
validated inspection.

Operations documentation is source-implemented through Slice 4C. Slices 4A–4B provide bounded exact-snapshot
inventory/evidence with deterministic limits, citations, omissions, Mermaid, and SHA-256 output. Slice 4C adds
immutable `operations/index.md`, exact Store loading, CLI generation, and validated inspection.

Onboarding documentation is source-implemented through Slice 5C with bounded documentation/package/command/
environment/test-CI anchors, scoped facts/relations/open diagnostics, shared 256-item budgeting, cited Mermaid,
immutable publication, exact Store loading, CLI generation, and validated inspection.

Slices 6A–6B add completeness reporting from canonical `file` inventory: processed/unprocessed paths,
per-language basis-point coverage, named adapter contribution, deterministic limits/omissions, exact Store
loading, read-only CLI, cancellation/drain, and versioned JSON transport.

Slices 7A–7C add bounded Next.js, Axum, and Express route projections while base JS/TS and Rust extractors
remain framework-neutral. Schema/auth/middleware, route composition, and handler linking remain deferred.

Slice 8A adds bounded PowerShell `$env:NAME` / `${env:NAME}` references through the existing redacted
`env://<NAME>` / `EnvVarUsed` contract. Exact self-evaluation `33676558603` on `75562e19…` confirms `ps1 = 2/2`.

Slice 8B recognizes only root `athanor.toml` as first-party runtime config, reusing redacted `Feature` /
`SymbolDefined`. Exact self-evaluation `33680051765` on `e219157a…` confirms 665/718 (`9261` bps), TOML 34/35.

Slice 8C recognizes `.github/actions/**/action.yml|yaml` only when `runs.using: composite`, projecting bounded
composite `ScriptCommand`, `run`/`uses` steps, and redacted step environment declarations. Exact self-evaluation
`33712810332` on `5d195069…` confirms 667/719 (`9276` bps), YAML 11/15.

Slice 8D recognizes only first-party `.github/dependabot.yml|yaml` version-2 update policies through existing
`Feature` / `SymbolDefined` contracts. Exact self-evaluation `33714498524` on `424da862…` confirms 669/720
(`9291` bps), TOML 34/35 and YAML 12/15. Post-merge Verification Matrix `33714498496` exposed test-only rustfmt
hunks in Slices 8C/8D; #91 applied exactly those formatter changes.

Slice 8E recognizes only root `deny.toml` cargo-deny policy domains `advisories`, `licenses`, `bans`, and
`sources`. It records selected scalar enforcement modes plus list counts without raw advisory/license/source
lists or generic TOML expansion. Exact self-evaluation `33722472453` on `c10ab848e…` is green: 670/720
processed (`9305` bps), TOML 35/35, YAML 12/15, 9051 facts, 13208 relations, 169 diagnostics. The artifact
contains all four `config://deny.toml#cargo-deny:<section>` keys. Post-merge CI `33722472454` passed formatting
and workspace tests before exposing one `clippy::collapsible_if`; #93 applied exactly the requested let-chain,
and PR CI `33723374660` confirmed formatting, workspace tests, and Clippy green on macOS before merge.

The shared `current.json` is profile-aware across architecture, module, API, operations, and onboarding.
Every inspector fails closed when another published profile owns the current pointer.

Supported deterministic documentation CLI surface includes architecture/module/API/operations/onboarding
generation and validated inspection plus read-only `ath docs completeness`; generation has no latest fallback,
retains hard-limit flags, and drains cancellation before returning. Existing coordinated `ath generate` is unchanged.

### Exact Evidence

Released baseline remains `609027eb02caa05346ebfea8538552c42b588c31`: CI `29995959544`, AppSec
`29995960063`, Store `29995959512`, release `29996579628`, clean-install smoke `29998347890`.

Slices 1A–1B are confirmed on `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`: CI `30013208011`, AppSec
`30013208197`, Store `30013208312`. Slice 1C1 exact Store loading is confirmed on `4f567271…`: CI
`30015689753`, AppSec `30015691399`, Store `30015689363`. Slice 1C2 is confirmed on `042d02ac…`: CI
`30025932615`, AppSec `30025931953`, Store `30025932704`.

The first bounded Rustok evaluation `30029451096` on `5e0b2809…` failed at citation budgeting and remains
failure evidence. Repaired evaluation/probe/matrix on `f1024cbc…` (`31625608720` / `31625608721` /
`31625608729`) is green; relation disclosure is confirmed on `6862aee8…` by `32712992516` / `32712992421`.

Later profile source landings include module Slice 2C `b9e0eadc…` with green Store `32718598218`, Rustok
evaluation `32718598212`, probe `32718598232`, and API Slice 3A `0a4c0f78…` with green Rustok evaluation
`32719989413` / probe `32719989450`; these do not substitute for focused profile execution evidence.

Completeness progression is exact and monotonic for selected semantic gaps: 8B `665/718` (`9261` bps),
8C `667/719` (`9276` bps), 8D `669/720` (`9291` bps), 8E `670/720` (`9305` bps). After 8E, TOML is 35/35;
remaining deliberate gaps include two issue-form YAML files, an OpenAPI YAML fixture, generic/test JSON fixtures,
`Cargo.lock`, one Python release verifier, root `install.sh`, and non-semantic repository files.

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
- [x] Slices 2A–5C: module/API/operations/onboarding source surfaces and publication flows;
- [x] Slices 6A–6B: pure completeness plus exact Store/read-only CLI/versioned JSON transport;
- [x] Slices 7A–7C: bounded built-in Next.js, Axum, and Express route projections;
- [x] Slices 8A–8E source implementation plus exact completeness confirmation;
- [ ] focused verification for later profile/completeness/framework slices and Slices 8A–8E remains pending.

`DOCGEN-001` remains in progress. Exact completeness confirms Slices 8A–8E coverage effects; that evidence does
not promote later slices to fully verified unless their required focused gate succeeds on one exact source commit.

## Product Backlog

- select the next slice from the exact 8E artifact, preferring useful first-party semantics over raw percentage;
- inspect root `install.sh` and `scripts/verify_release_version.py` before considering generic JSON/test fixtures;
- keep issue forms and OpenAPI fixtures out unless independently justified by product semantics;
- do not add generic JSON/fixture parsing solely to raise coverage;
- keep Next.js/Axum/Express schema/auth/middleware and route-composition expansion evidence-driven;
- Dart/Flutter remains blocked on a portable DartScope dependency boundary rather than a local-only path dependency;
- optional i18n, semantic/vector retrieval, provider, daemon, and MCP integration after quality gates.
