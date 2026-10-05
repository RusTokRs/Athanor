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

Module/API/operations/onboarding/completeness/framework expansion is exact-verified through Slices 2A–7C on
the commits recorded in `roadmap-status.md`; Slices 8F–8H are exact-verified on `e246a71b…` with the post-8H
self-evaluation green at `685/731` (`9370` bps). Exact Athanor self-evaluation `33722472453` on
`c10ab848e9f0250da14ccaa0e455ab7d26d920bd` confirms the Slice 8E effect at 670/720 (`9305` bps), TOML 35/35;
focused verification for Slices 8A–8E remains pending. Slice 8I (first-party agent-tool MCP configuration) is
source-implemented on `6fb5e34…`; exact self-evaluation `37292220926` confirms `691/735` (`9401` bps),
JSON `2/35`.

The existing coordinated `ath generate` command is unchanged. No model provider, daemon, MCP, or new dependency is enabled.

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
- New profiles start as pure deterministic owners before publication or transport integration.
- Coverage expansion follows exact completeness evidence and bounded product semantics rather than raw percentage chasing.

## Implemented Contracts

- `athanor.documentation_generation_request.v1`
- `athanor.documentation_generation_manifest.v1`
- `athanor.documentation_outline.v1`
- `athanor.documentation_context.v1`
- `athanor.documentation_citation.v1`
- `athanor.documentation_draft.v1`
- `athanor.documentation_validation_report.v1`
- `athanor.documentation_current.v1`
- `athanor.documentation_completeness.v1`

`DocumentationProfile` contains `architecture`, `module`, `api`, `operations`, and `onboarding`; the v1
schema names remain unchanged.

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

The operation initializes the configured Store, loads exactly `SnapshotId(request.snapshot)` through
`CanonicalSnapshotStore`, verifies identity, checks cancellation, and delegates publication.

```bash
ath docs generate-architecture <PATH> --snapshot <EXACT-ID> \
  [--max-entities N] [--max-facts N] [--max-relations N] [--max-diagnostics N] [--force] [--json]
ath docs architecture current <PATH> [--json]
ath docs architecture manifest <PATH> [--json]
ath docs architecture validation <PATH> [--json]
```

Generation drains after Ctrl-C cancellation. Inspection rejects path escape, wrong profile, identity
or layout drift, invalid validation status, and checksum drift.

### Slices 2A–2C — Module Documentation Profile

- Slice 2A established the exact-snapshot evidence-backed module inventory and `modules/index.md`.
- Slice 2B added module-scoped facts, relations, open diagnostics, Mermaid edges, omissions, and the
  deterministic aggregate 256-item round-robin budget.
- Slice 2C added immutable publication, exact Store loading, `generate-module`, and validated
  `module current|manifest|validation` with profile-isolated shared `current.json`.
- Module publication/Store/CLI source regressions are present; focused execution evidence is pending.

### Slices 3A–3C — API Documentation Profile

- Slice 3A established exact endpoint/schema/example inventory, portable evidence, deterministic
  round-robin selection, omissions, citations, `api/index.md`, and checksum binding.
- Slice 3B added API-scoped facts, supported canonical relations, open diagnostics, aggregate 256-item
  budgeting, and cited Mermaid relationships.
- Slice 3C added immutable publication, exact Store loading, `generate-api`, and validated
  `api current|manifest|validation` with profile-isolated shared `current.json`.
- API source regressions are present; focused execution evidence remains pending.

### Slices 4A–4C — Operations Documentation Profile

- Slices 4A–4B establish the pure exact-snapshot operations profile over evidence-backed `EnvVar`,
  script/CI, deployment, database, configuration, runbook, and operation-step entities; general
  `Package`/`Dependency` entities remain outside the bounded inventory.
- Facts are scoped by selected operations anchors. Supported relations are `Defines`, `Contains`,
  `Documents`, `DocumentsOperation`, `UsesEnv`, and `QueriesTable`; open diagnostics require an anchor.
- Shared portable evidence normalization, per-kind limits, aggregate `DOCUMENTATION_REFERENCE_LIMIT = 256`,
  cited Mermaid, omissions, validation metrics, deterministic `operations/index.md`, and SHA-256 remain
  regression protected.
- Slice 4C publishes `operations/index.md` plus `validation-report.json` in the shared immutable generation
  root and advances the profile-aware `current.json` only after a complete staged publication.
- Exact `UpToDate` validates profile, snapshot, generation path, manifest layout, artifact checksums, and
  validation identity/status. Force, tamper recovery, immutable history, and cancellation are protected.
- `DocumentationOperationsOperationOptions` initializes the configured Store and loads only exact
  `SnapshotId(request.snapshot)`; missing snapshots and Store identity mismatch fail closed.
- `ath docs generate-operations <PATH> --snapshot <EXACT-ID>` supports hard limits, `--force`, `--json`,
  and Ctrl-C cancellation/drain. `ath docs operations current|manifest|validation` validates profile,
  confinement, layout, identities, and SHA-256. Existing `ath docs operations check` remains unchanged.
- Operations publication/Store/CLI source regressions are present; focused execution evidence is pending.

### Slices 5A–5C — Onboarding Documentation Profile

- `DocumentationProfile::Onboarding` serializes as `onboarding` without changing v1 schema names.
- `build_documentation_onboarding_profile` is pure over one exact `CanonicalSnapshot`; eligible evidence-backed
  anchors are `DocumentationPage`, `DocumentationSection`, `Package`, `ScriptCommand`, `EnvVar`, and `TestCase`/`CiJob`.
- Six category queues use stable-key/entity-id round-robin. Slice 5B adds facts when subject or object is a
  selected anchor, supported `Contains`, `Documents`, `UsesEnv`, and `TestedBy` relations touching an anchor,
  and open diagnostics referencing an anchor.
- Per-kind limits feed deterministic Entity/Fact/Relation/Diagnostic round-robin under the aggregate
  `DOCUMENTATION_REFERENCE_LIMIT = 256`; each selected item is cited and supported relations emit Mermaid edges.
- Omitted counts are scoped to eligible onboarding evidence; omitted supported relations are disclosed and
  feed `validation.metrics.unsupported_relations`. Shared portable evidence normalization remains the owner.
- Slice 5C publishes deterministic `onboarding/index.md` plus `validation-report.json` in the shared immutable
  generation root and advances profile-aware `current.json` only after complete staged publication.
- Exact `UpToDate` validates profile, snapshot, generation path, manifest layout, artifact checksums, and
  validation identity/status. Force, tamper recovery, immutable history, and cancellation are source protected.
- `DocumentationOnboardingOperationOptions` initializes the configured Store and loads only exact
  `SnapshotId(request.snapshot)`; missing snapshots and Store identity mismatch fail closed.
- `ath docs generate-onboarding <PATH> --snapshot <EXACT-ID>` supports hard limits, `--force`, `--json`, and
  Ctrl-C cancellation/drain. `ath docs onboarding current|manifest|validation` validates profile, confinement,
  layout, identities, and SHA-256.
- Shared `current.json` is profile-isolated across architecture, module, API, operations, and onboarding.
- Onboarding profile/publication/Store/CLI source regressions are present; focused execution evidence remains pending.

### Slices 6A–6B — Documentation Completeness Reporting

- Slice 6A adds a pure exact-snapshot completeness owner over canonical baseline `EntityKind::File` inventory.
  It reports tracked/processed/unprocessed files, per-language basis-point coverage, named non-baseline adapter
  contribution across facts/relations/diagnostics, deterministic limits/omissions, and keeps entity-only
  canonical processing visible without inventing adapter attribution.
- Adapter file contribution is confined to baseline-tracked portable paths; generated/untracked evidence cannot
  inflate tracked-file coverage. Shared documentation evidence/path normalization remains the path policy owner.
- Slice 6B initializes the configured Store and loads only exact `SnapshotId(request.snapshot)`; missing snapshots,
  identity mismatch, invalid requests, and cancellation fail closed. There is no latest-snapshot fallback.
- `ath docs completeness <PATH> --snapshot <EXACT-ID> [--limit N] [--json]` is read-only and publishes no
  generation/current pointer. Ctrl-C cancels and drains the exact Store/report operation.
- JSON output uses registered `athanor.documentation_completeness.v1` through
  `VersionedDocumentationCompletenessReport`; registry uniqueness and a golden fixture are source protected.
- Completeness source regressions are present; focused format/test/Clippy execution evidence remains pending.

### Slices 7A–7C — Bounded Framework Route Projections

- Slice 7A adds `NextJsExtractor`, projecting standard App/Pages Router filesystem conventions into
  adapter-scoped `nextjs_route` entities plus evidence-backed `RouteDeclared` facts without inferring React,
  schema, auth, middleware, or handler semantics.
- Slice 7B adds `AxumExtractor`, recognizing bounded literal `.route()` declarations backed by supported
  `axum::routing` method constructors and emitting adapter-scoped `axum_route` knowledge.
- Slice 7C adds `ExpressExtractor`, recognizing explicit application/router bindings and exact two-argument
  literal route calls and emitting adapter-scoped `express_route` knowledge.
- Base JS/TS and Rust extractors remain framework-neutral. Framework schema/auth/middleware, composed routers,
  and handler linking remain separate future slices.
- Source regressions are present; focused execution evidence for Slices 7A–7C remains pending.

### Slice 8A — Completeness-Driven PowerShell Environment References

- Selection is grounded in exact self-evaluation `32815625060` on `2c5e94220b8fb2cb396938f96bb3dedb0e535816`:
  two tracked `ps1` files and zero processed.
- `athanor-extractor-operations` recognizes `*.ps1`, projects bounded `$env:NAME` / `${env:NAME}` through the
  existing `env://<NAME>` / `EnvVarUsed` contract, and stores no raw process-environment values.
- General PowerShell AST/cmdlet/function/assignment/control-flow semantics remain out of scope.
- Exact self-evaluation `33676558603` on `75562e19a8eed3a84c47a346a19d0f078550fa22` is green and confirms `ps1 = 2/2`.

### Slice 8B — First-Party Athanor Runtime Configuration

- Root `athanor.toml` reuses the existing redacted TOML runtime-config `Feature` / `SymbolDefined` contract;
  arbitrary root TOML remains unsupported by that owner.
- Exact self-evaluation `33680051765` on `e219157ac0afd4be7e6b2982342096e2ab926445` is green: 665/718
  (`9261` bps), TOML 34/35; `deny.toml` is the only remaining TOML gap.

### Slice 8C — Bounded GitHub Composite Actions

- `.github/actions/**/action.yml|yaml` is recognized only when `runs.using: composite`.
- Existing ScriptCommand and redacted environment contracts project the action, bounded `run`/`uses` steps,
  and step `env`; JS/Docker actions, inputs/outputs expression semantics, permissions, secrets, and generic YAML stay out.
- Exact self-evaluation `33712810332` on `5d195069dc068d830de81ecd2f7eef3fb181018f` is green: 667/719
  (`9276` bps), YAML 11/15.

### Slice 8D — Bounded Dependabot Update Policies

- Only `.github/dependabot.yml|yaml` with `version: 2` is recognized. `updates[]` entries with
  `package-ecosystem` + `directory` reuse existing `Feature` / `SymbolDefined`; optional schedule/target metadata is bounded.
- Registries/credentials, groups, ignore/allow rules, issue forms, extractor fixtures, and generic YAML remain out.
- Exact self-evaluation `33714498524` on `424da862d34ef7d8812b65c4c467a17015f9a907` is green: 669/720
  (`9291` bps), YAML 12/15, TOML 34/35.
- Verification Matrix `33714498496` exposed formatter-only test hunks in Slices 8C/8D; #91 applied exactly them.

### Slice 8I — First-party Agent-Tool MCP Configuration

- Only the root `.codex.json` (`mcp.servers`) and `antigravity.json` (`mcpServers`) files are recognized; one
  `ScriptCommand` per configured MCP server records the agent tool, server name, and literal command plus
  argument tokens through the existing `ScriptCommand` / `SymbolDefined` contracts with evidence and ownership.
- Malformed, partially supported, or server-less files are rejected atomically; generic JSON is not projected
  and the configured commands are never executed.
- Selected from the exact post-gate artifact on `aaf79d6…` (run `37290849626`, `688/734`, `9373` bps, 46
  unprocessed, only in-policy product-semantic gap); exact self-evaluation `37292220926` on `6fb5e34…`
  confirms `691/735` (`9401` bps), JSON `2/35`.

### Slice 8H — First-party GitHub Issue Forms

- `.github/ISSUE_TEMPLATE/*.yml|yaml` is recognized as a first-party contributor interface through a bounded YAML projection.
- The projection records form identity plus body-item type, label, and required state; descriptions and labels are retained as bounded metadata.
- GitHub rendering, expressions, markdown semantics, and arbitrary YAML are not projected.
- Path recognition is restricted to a single file directly under `.github/ISSUE_TEMPLATE/`; empty form names fail closed.
- Focused verification is green on `e246a71b…` (post-8H self-evaluation `685/731`, `9370` bps).

### Slice 8G — First-party PowerShell Installer

- The repository-root `install.ps1` is recognized through an exact path and bounded first-party contract.
- The projection records `ath.exe`/`athd.exe`, `SHA256SUMS`, and SHA-256 verification through `Get-FileHash`.
- Runtime install directories, environment values, filesystem state, and generic PowerShell control flow are not projected.
- Focused verification is green on `e246a71b…`.

### Slice 8F — First-party Runtime Artifacts

- Root `install.sh` is recognized through a dedicated bounded installer projection with `ath`/`athd` targets and
  checksum-verification anchors, without generic shell parsing or runtime path/value capture.
- `scripts/verify_release_version.py` is recognized through exact local contract anchors for release-tag format,
  Cargo version coherence, changelog/date/substantive-notes checks, notes output, and CLI inputs, without Python
  execution or generic AST parsing.
- Both projections use existing `ScriptCommand`, evidence, ownership, and fail-closed extraction contracts.
- Focused verification is green on `e246a71b…`; the first post-merge matrix on `c978187d…` failed at formatting only.

### Slice 8E — Bounded Cargo-Deny Supply-Chain Policy

- Only root `deny.toml` is recognized, and only policy domains `advisories`, `licenses`, `bans`, `sources`.
- Existing `Feature` / `SymbolDefined` knowledge records selected scalar enforcement modes and list counts;
  advisory IDs, license lists/exceptions, registry/git values, nested rules, and generic TOML are not copied.
- Exact self-evaluation `33722472453` on `c10ab848e9f0250da14ccaa0e455ab7d26d920bd` is green: 670/720
  (`9305` bps), TOML 35/35, YAML 12/15, 9051 facts, 13208 relations, 169 diagnostics. The operations artifact
  includes `config://deny.toml#cargo-deny:{advisories,bans,licenses,sources}` evidence.
- Post-merge CI `33722472454` passed formatting and workspace tests before one `clippy::collapsible_if` failure.
  #93 applied only the requested let-chain; PR CI `33723374660` confirmed formatting, workspace tests, and Clippy
  green on macOS before squash merge to `2310215b61980de3300a93e43e10a68daa27f800`.

## Execution Evidence

- Architecture baselines: Slices 0A–0B `2a049303…`; 1A–1B `0cfeca8a…`; 1C1 `4f567271…`; 1C2 `042d02ac…`.
- First Rustok failure `30029451096` remains failure evidence; repaired evaluation `31625608720`, probe
  `31625608721`, CI `31625608729`, AppSec `31625608723`, Store `31625608739` are green on `f1024cbc…`.
- Relation disclosure is exact-evaluation-confirmed on `6862aee8…` by `32712992516` / `32712992421`.
- Later source landings do not substitute for focused profile verification: module 2C `b9e0eadc…`, API 3A `0a4c0f78…`.
- Completeness progression: 8B 665/718 (`9261`), 8C 667/719 (`9276`), 8D 669/720 (`9291`), 8E 670/720 (`9305`),
  post-8H `685/731` (`9370`), pre-8I `688/734` (`9373`), post-8I `691/735` (`9401`).
- Slices 8F–8H are exact-verified on `e246a71b…`; Slice 8I is confirmed by self-evaluation `37292220926` on
  `6fb5e34…` plus the green pull request workflow set; 8A–8E focused verification remains pending.

The repaired bounded Rustok architecture-generation evaluation retains `DOCUMENTATION_REFERENCE_LIMIT`,
`workflow_dispatch` support, and diagnostic evidence checks as explicit regression boundaries.

## Next Bounded Step

1. Keep the pull request workflow set green on the Slice 8I final head.
2. Select the next bounded semantic gap from the post-8I artifact: the only in-policy gap left is the
   typo-named `for agents. md` briefing (a repository rename, not extractor semantics); other unprocessed
   files are policy-excluded fixtures, `Cargo.lock`, non-semantic files, or the separately-gated OpenAPI
   fixture.
3. Keep provider, daemon, MCP, and coordinated `ath generate` changes separately gated.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-extractor-operations --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- docs check
```

The complete per-profile documentation inventory and CLI matrix is maintained in
`athanor_implementation_plan_ru.md` §6; the focused docgen workflows run the per-profile inventories and
CLI checks on every supported operating system.
