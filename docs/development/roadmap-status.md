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

Operations documentation is source-implemented through Slice 4C. Slices 4A–4B provide the bounded exact
snapshot profile with six-category entity selection, operations-scoped facts, a canonical relation allowlist,
open diagnostics, aggregate 256-item budgeting, cited Mermaid, omissions, and deterministic SHA-256 output.
Slice 4C adds immutable `operations/index.md` publication, exact Store loading, CLI generation, and
validated inspection.

Onboarding documentation is source-implemented through Slice 5C. Slices 5A–5B provide evidence-backed
documentation pages/sections, packages, runnable script commands, environment variables, and test/CI anchors
with deterministic six-category selection, scoped facts, bounded `Contains`/`Documents`/`UsesEnv`/`TestedBy`
relations, open diagnostics, aggregate `DOCUMENTATION_REFERENCE_LIMIT = 256` budgeting, cited Mermaid, and
omission disclosure. Slice 5C adds immutable `onboarding/index.md` publication, exact Store loading, CLI
generation, and validated inspection.

Slices 6A–6B add documentation completeness reporting. The pure owner uses canonical baseline `file`
inventory to report processed/unprocessed files, per-language basis-point coverage, named non-baseline adapter
contribution, deterministic limits/omissions, and entity-only canonical processing without invented attribution.
Slice 6B adds exact Store loading, read-only `ath docs completeness`, cancellation/drain, and registered
`athanor.documentation_completeness.v1` JSON transport. No latest fallback or publication/current pointer exists.

Slices 7A–7C add the first bounded framework projections selected from explicit coverage gaps.
`NextJsExtractor` recognizes App/Pages Router filesystem conventions and emits adapter-scoped `nextjs_route`
entities plus `RouteDeclared` evidence. `AxumExtractor` recognizes literal `.route()` declarations backed by
supported `axum::routing` constructors and emits adapter-scoped `axum_route` knowledge. `ExpressExtractor`
recognizes explicit Express application/router bindings and exact two-argument literal route calls, emitting
adapter-scoped `express_route` knowledge. Base JS/TS and Rust extractors remain framework-neutral;
framework-specific schema/auth/middleware, route composition, and handler linking remain deferred.

Slice 8A adds bounded PowerShell environment references selected from exact completeness evidence. The
operations adapter recognizes `*.ps1` and projects `$env:NAME` and `${env:NAME}` references into the existing
`env://<NAME>` / `EnvVarUsed` contract without storing values. The exact Athanor self-evaluation on
`75562e19a8eed3a84c47a346a19d0f078550fa22`, run `33676558603`, is green and confirms `ps1` at 2/2
processed. General PowerShell functions, cmdlets, assignments, control flow, AST semantics, and inline
expression inference remain out of scope. This exact completeness confirmation is execution evidence for the
coverage effect; it does not promote Slice 8A to fully verified without the required focused gate.

Slice 8B recognizes only the first-party root `athanor.toml` as runtime configuration and reuses the existing
redacted `Feature` / `SymbolDefined` contracts. Exact post-merge self-evaluation on
`e219157ac0afd4be7e6b2982342096e2ab926445`, run `33680051765`, is green: completeness is 665/718
processed (`9261` basis points), TOML is 34/35, and `deny.toml` is the only remaining TOML gap. This confirms
the Slice 8B coverage effect without promoting the slice to fully verified focused execution.

Slice 8C adds bounded first-party GitHub composite actions. The operations adapter recognizes
`.github/actions/**/action.yml|yaml` and emits knowledge only when `runs.using: composite`: one composite-action
`ScriptCommand`, bounded `run`/`uses` step commands, and step `env` through the existing redacted
`github_actions` environment contract. Exact post-merge self-evaluation on
`5d195069dc068d830de81ecd2f7eef3fb181018f`, run `33712810332`, is green: completeness is 667/719
processed (`9276` basis points) and YAML is 11/15, confirming the composite-action coverage effect. Focused
execution evidence remains pending.

Slice 8D is source-implemented from that exact report's remaining first-party operational YAML gap. The
operations adapter recognizes only `.github/dependabot.yml|yaml` with `version: 2` and projects `updates[]`
entries that provide `package-ecosystem` plus `directory` into existing `Feature` / `SymbolDefined` contracts;
optional `schedule.interval` and `target-branch` metadata remain bounded payload. Registries/credentials,
groups, ignore/allow rules, issue forms, extractor fixtures, and generic YAML remain out of scope. Exact
post-merge completeness and focused execution evidence remain pending.

The shared `current.json` is profile-aware across architecture, module, API, operations, and onboarding.
Every inspector fails closed when another published profile owns the current pointer.

Supported deterministic documentation CLI surface includes:

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
ath docs generate-onboarding <PATH> --snapshot <EXACT-ID> [--force] [--json]
ath docs onboarding current|manifest|validation <PATH> [--json]
ath docs completeness <PATH> --snapshot <EXACT-ID> [--limit N] [--json]
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
`32719989450` are green. These do not substitute for focused module/API/operations/onboarding execution evidence.

The exact Athanor self-evaluation on `2c5e94220b8fb2cb396938f96bb3dedb0e535816`, run `32815625060`,
selected Slice 8A: the completeness artifact reported `ps1` with two tracked files and zero processed.

The exact Athanor self-evaluation on `75562e19a8eed3a84c47a346a19d0f078550fa22`, run `33676558603`,
is green and confirms the Slice 8A coverage effect: `ps1` is 2/2 processed. It also selects Slice 8B from
`toml = 33/35`, where only `athanor.toml` and `deny.toml` were unprocessed.

The exact Athanor self-evaluation on `e219157ac0afd4be7e6b2982342096e2ab926445`, run `33680051765`,
is green and confirms Slice 8B: total completeness is 665/718 (`9261` basis points), `toml = 34/35`, and only
`deny.toml` remains unprocessed in TOML. Its five YAML gaps selected Slice 8C around the first-party composite
action `.github/actions/setup-rust/action.yml`; generic YAML expansion is not implied.

The exact Athanor self-evaluation on `5d195069dc068d830de81ecd2f7eef3fb181018f`, run `33712810332`,
is green and confirms Slice 8C: total completeness is 667/719 (`9276` basis points), `yaml = 11/15`, and the
composite action is processed. The four remaining YAML gaps are the two issue forms, `.github/dependabot.yml`,
and the OpenAPI extractor fixture; the first-party operational Dependabot policy selects Slice 8D.

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
- [x] Slices 5A–5C: onboarding inventory/evidence/publication/exact Store/CLI/inspection;
- [x] Slices 6A–6B: pure completeness plus exact Store/read-only CLI/versioned JSON transport;
- [x] Slices 7A–7C: bounded built-in Next.js, Axum, and Express route projections;
- [x] Slice 8A source implementation plus exact completeness confirmation: bounded PowerShell environment references;
- [x] Slice 8B source implementation plus exact completeness confirmation: first-party `athanor.toml` runtime config;
- [x] Slice 8C source implementation plus exact completeness confirmation: bounded GitHub composite actions;
- [x] Slice 8D source implementation: bounded first-party Dependabot update policies;
- [ ] Focused module 2B–2C, API 3A–3C, operations 4A–4C, onboarding 5A–5C, completeness 6A–6B,
  framework 7A–7C, Slices 8A–8C focused verification, and Slice 8D post-merge completeness remain pending.

`DOCGEN-001` remains in progress. Architecture is execution-confirmed; later profile/completeness/framework
surfaces and Slices 8A–8D are not promoted without their required focused gates.

## Product Backlog

- after Slice 8D lands, use its automatic exact Athanor self-evaluation/completeness result to select the next bounded semantic gap;
- do not add generic JSON/fixture parsing solely to raise coverage without explicit product semantics;
- keep arbitrary root tool/policy TOML such as `deny.toml` outside runtime config unless explicit product semantics justify it;
- keep issue-form and OpenAPI-fixture YAML outside Slice 8D unless separately justified;
- keep Next.js/Axum/Express schema/auth/middleware and route-composition expansion separate until evidence justifies it;
- Dart/Flutter remains blocked on a portable DartScope dependency boundary rather than a local-only path dependency;
- optional i18n, semantic/vector retrieval, provider, daemon, and MCP integration after quality gates.
