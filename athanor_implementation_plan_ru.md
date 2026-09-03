# Athanor: план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-09-03  
> Статус: `API-001`, `REL-001` verified; `DOCGEN-001 / Slices 0A–1C` execution-confirmed;
> relation-disclosure exact-evaluation-confirmed; module 2A–2C, API 3A–3C, operations 4A–4C,
> onboarding 5A–5C, completeness 6A–6B, framework projections 7A–7C и Slices 8A–8E source-implemented;
> exact completeness подтверждает coverage effects 8A–8E; focused verification later slices pending

## 1. Статусы и evidence

- `[x] implemented` — код, документация и source regressions присутствуют.
- `[-] in progress` — package Definition of Done закрыт частично.
- `[x] verified` — required matrix успешна на одном exact source commit.
- `[ ] planned` — следующий bounded этап.

Promotion требует успешных `athanor/verification-matrix`, `athanor/appsec` и
`athanor/store-conformance` на одном SHA, когда такой matrix входит в Definition of Done конкретного
пакета. Наличие кода или metadata не является execution evidence.

## 2. Baselines

Release baseline `609027eb02caa05346ebfea8538552c42b588c31`:
CI `29995959544`, AppSec `29995960063`, Store `29995959512`, Release `29996579628`, clean-install
smoke `29998347890`. Annotated `v0.2.1` указывает на этот SHA.

Documentation generation:

- Slices 0A–0B: `2a049303e797f00ac53f1e91fc010f284993926d`, CI `30005828864`, AppSec
  `30005828850`, Store `30005828956`;
- Slices 1A–1B: `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71`, CI `30013208011`, AppSec
  `30013208197`, Store `30013208312`;
- Slice 1C1: `4f567271ed6d38d30b3c15dc6999aa33152a9312`, CI `30015689753`, AppSec
  `30015691399`, Store `30015689363`;
- Slice 1C2: `042d02ac6b4c89d90a5b76c818098eb0c6b41920`, CI `30025932615`, AppSec
  `30025931953`, Store `30025932704`;
- first bounded Rustok gate: source `5e0b28099c48e22bdc172fa57b6d51db9e6efb7b`, run `30029451096`,
  failure `documentation draft citations must contain between 1 and 256 entries`;
- isolated probe evidence: source `12a8687c5d098ab05a5988508816aad5f0dc3e23`, run `30030131126`;
- repaired bounded Rustok evaluation: `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e`, evaluation
  `31625608720`, probe `31625608721`, CI `31625608729`, AppSec `31625608723`, Store `31625608739`;
- relation-disclosure tuning: `6862aee81dd0f53fa8372d1ce3fcb6e2ed198cca`, evaluation
  `32712992516`, probe `32712992421`; artifact confirms `unsupported_relation_disclosed = true`;
- Slice 2A landed on `658d53fb03dd47a971beb6cf67b46cfe1f20b3fe`; post-merge contexts green:
  Rustok evaluation `32714912841`, Rustok probe `32714912826`, AppSec `32714912809`, Store
  Conformance `32714912932`. Focused module-profile format/test/Clippy evidence не заявляется.
- Slice 2C landed on `b9e0eadc46175e15ec62f915a4729287f3884cd2`; post-merge Store Conformance
  `32718598218`, Rustok evaluation `32718598212` и Rustok probe `32718598232` green. Это не заменяет
  focused module execution evidence.
- Slice 3A landed on `0a4c0f78ef05b6c2ba9480b770c1ebf72038e049`; post-merge Rustok evaluation
  `32719989413` и Rustok probe `32719989450` green. Это не заменяет focused API execution evidence.
- Athanor self-evaluation на exact `2c5e94220b8fb2cb396938f96bb3dedb0e535816`, run `32815625060`,
  дал selection evidence для Slice 8A: `ps1 tracked = 2`, `processed = 0`.
- Athanor self-evaluation на exact `75562e19a8eed3a84c47a346a19d0f078550fa22`, run `33676558603`,
  green: `ps1 = 2/2 processed`; тот же artifact выбрал Slice 8B из `toml = 33/35`.
- Athanor self-evaluation на exact `e219157ac0afd4be7e6b2982342096e2ab926445`, run `33680051765`,
  green: total `665/718` (`9261` basis points), `toml = 34/35`, единственный TOML gap — `deny.toml`.
  Пять YAML gaps дали bounded target `.github/actions/setup-rust/action.yml` для Slice 8C.
- Self-evaluation `33712810332` на `5d195069dc068d830de81ecd2f7eef3fb181018f` подтверждает Slice 8C:
  `667/719` (`9276` bps), `yaml = 11/15`; `33714498524` на `424da862d34ef7d8812b65c4c467a17015f9a907`
  подтверждает Slice 8D: `669/720` (`9291` bps), `yaml = 12/15`, `toml = 34/35`.
- Self-evaluation `33722472453` на `c10ab848e9f0250da14ccaa0e455ab7d26d920bd` подтверждает Slice 8E:
  `670/720` (`9305` bps), `toml = 35/35`, `yaml = 12/15`, facts `9051`, relations `13208`, diagnostics `169`.

## 3. Завершённые пакеты

- `COMP-003` / `COMP-003C2B2C2B`: explicit runtime composition и bounded owners.
- `MCP-007`: cancellation-safe transactional Index publication.
- `JSON-003`: recursive, disjoint, fixture-backed contract lifecycle.
- `DOC-001` / `DOC-002`: status hygiene, current owners и line budgets.
- `MCP-004`: control-plane responsiveness under saturation.
- `VERIFY-001`: exact cross-platform CI/AppSec/Store evidence.
- `API-001`: verified GraphQL/OpenAPI request/response/status/security consistency.
- `REL-001`: verified immutable `v0.2.1` release and clean Linux/Windows installs.

## 4. Активная разработка

### 4.1 `DOCGEN-001` — evidence-backed documentation generation

#### Slices 0A–0B — contracts

- [x] Strict request/manifest, hard limits, omissions, paths и checksums.
- [x] Versioned outline/context/citation/draft/validation contracts.
- [x] Data policy, quality metrics, fixture repository и Rustok evaluation corpus.
- [x] Full chain alignment и fail-closed regressions.

#### Slice 1A — deterministic architecture profile

- [x] Exact `CanonicalSnapshot`, deterministic sorting и hard limits.
- [x] Overview/Components/Relationships/Diagnostics.
- [x] Stable-key/evidence citations, Mermaid edges, footnotes и SHA-256.

#### Slice 1B — immutable publication

- [x] `.athanor/generated/documentation/generations/<8-digit-id>`.
- [x] Manifest, Markdown, validation report и atomic `current.json`.
- [x] Exact `UpToDate`, force, immutable history, tamper recovery и cancellation.

#### Slice 1C1 — exact committed-snapshot operation

- [x] `RuntimeComposition::init_store` and exact `CanonicalSnapshotStore::load_snapshot`.
- [x] Missing/uncommitted and identity mismatch fail closed.
- [x] Cancellation checks around Store and publication boundaries.
- [x] Exact matrix on `4f567271ed6d38d30b3c15dc6999aa33152a9312`.

#### Slice 1C2 — CLI generation and inspection

- [x] `ath docs generate-architecture <PATH> --snapshot <EXACT-ID>`.
- [x] `--force`, hard-limit flags, text/JSON output и Ctrl-C drain.
- [x] `ath docs architecture current|manifest|validation` с confinement/identity/checksum validation.
- [x] Exact matrix on `042d02ac6b4c89d90a5b76c818098eb0c6b41920`.

#### Bounded Rustok evaluation — repaired gate и artifact review closed

- [x] Первый Rustok gate `5e0b28099c48e22bdc172fa57b6d51db9e6efb7b` / `30029451096`
  остановился на `documentation draft citations must contain between 1 and 256 entries`; общий reference
  budget исправлен на 256.
- [x] Probe `12a8687c5d098ab05a5988508816aad5f0dc3e23` / `30030131126` зафиксировал repair path.
- [x] Exact source `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e` полностью green по evaluation,
  probe, Verification Matrix, AppSec и Store Conformance.
- [x] Artifact review выявил undisclosed `unsupported_relations = 6962`; `6862aee8…` добавил disclosure.
- [x] Exact evaluation `32712992516` и probe `32712992421` успешны;
  `unsupported_relation_disclosed = true`, deterministic repeatability unchanged.

#### Slices 2A–2C — module documentation profile

- [x] `DocumentationProfile::Module`, exact pure inventory, module-scoped facts/relations/open diagnostics.
- [x] Shared 256-item round-robin budget, cited Markdown/Mermaid, omissions и semantic-parity regressions.
- [x] Immutable `modules/index.md` publication, exact Store operation, `generate-module`, validated
  `module current|manifest|validation`, profile-isolated shared `current.json`.
- [ ] Focused format/test/Clippy evidence для Slices 2B–2C pending; source implementation не verified.

#### Slices 3A–3C — API documentation profile

- [x] `DocumentationProfile::Api`, pure endpoint/schema/example inventory и shared portable evidence owner.
- [x] API-scoped facts, bounded canonical relation allowlist, open diagnostics, aggregate 256-item budget,
  cited Mermaid, omissions и deterministic input-order regressions.
- [x] Immutable `api/index.md`, exact Store operation, `generate-api`, validated
  `api current|manifest|validation`, profile-isolated shared `current.json`.
- [ ] Focused format/test/Clippy evidence для API Slices 3A–3C pending; source implementation не verified.

#### Slices 4A–4C — operations documentation profile

- [x] `DocumentationProfile::Operations`, pure exact-snapshot operational inventory and `operations/index.md`.
- [x] Scope: evidence-backed `EnvVar`, `Script`/`ScriptCommand`/`CiJob`, `DockerService`,
  `DbMigration`/`DbTable`, `Feature`, `Runbook`, `OperationStep`; `Package`/`Dependency` остаются вне inventory.
- [x] Deterministic six-category entity round-robin, portable evidence, `max_entities` + shared 256 ceiling,
  citations, omissions, SHA-256 и fail-closed identity/no-surface regressions.
- [x] Slice 4B добавляет operations-scoped facts, bounded relation allowlist и open diagnostics with cited Mermaid.
- [x] Slice 4C публикует immutable `operations/index.md` + validation report в общем generation root и
  использует profile-aware atomic `current.json`; exact `UpToDate`, force, tamper recovery и cancellation защищены.
- [x] `DocumentationOperationsOperationOptions` использует `RuntimeComposition::init_store` и только exact
  `CanonicalSnapshotStore::load_snapshot(SnapshotId(request.snapshot))`; latest fallback отсутствует.
- [x] Missing/uncommitted snapshot и Store identity mismatch fail closed; cancellation checks стоят вокруг
  Store/publication boundaries.
- [x] `ath docs generate-operations <PATH> --snapshot <EXACT-ID>` поддерживает hard-limit flags, `--force`,
  `--json` и Ctrl-C cancellation/drain.
- [x] `ath docs operations current|manifest|validation` проверяет profile, confinement, generation path,
  exact artifact layout, manifest/report identity и SHA-256; старый `ath docs operations check` не меняется.
- [x] Shared `current.json` изолирует architecture/module/API/operations inspectors по profile.
- [x] Source regressions покрывают lifecycle, exact operation contract, profile isolation/path/checksum drift
  и binary `index -> generate-operations -> inspect -> UpToDate` round-trip.
- [ ] Focused format/test/Clippy execution evidence для operations Slices 4A–4C pending; source implementation не verified.

#### Slices 5A–5C — onboarding documentation profile

- [x] `DocumentationProfile::Onboarding` и pure exact-snapshot `build_documentation_onboarding_profile`.
- [x] Evidence-backed anchors: `DocumentationPage`, `DocumentationSection`, `Package`, `ScriptCommand`, `EnvVar`,
  `TestCase`/`CiJob`; deterministic six-category round-robin сохраняет low-limit fairness.
- [x] Slice 5B включает facts, если subject или object — selected onboarding anchor; relation allowlist ограничен
  `Contains`, `Documents`, `UsesEnv`, `TestedBy`, и relation должна касаться selected anchor.
- [x] Только `Open` diagnostics, ссылающиеся на selected anchor, входят в context; unsupported/unrelated evidence не протекает.
- [x] Per-kind limits + aggregate `DOCUMENTATION_REFERENCE_LIMIT = 256`, cited Mermaid, omissions и
  `unsupported_relations = context.omitted.relations` regression-protected.
- [x] Slice 5C публикует immutable `onboarding/index.md` + validation report в общем generation root и
  использует profile-aware atomic `current.json`; exact `UpToDate`, force, tamper recovery и cancellation защищены.
- [x] `DocumentationOnboardingOperationOptions` использует `RuntimeComposition::init_store` и только exact
  `CanonicalSnapshotStore::load_snapshot(SnapshotId(request.snapshot))`; latest fallback отсутствует.
- [x] Missing/uncommitted snapshot и Store identity mismatch fail closed; cancellation checks стоят вокруг
  Store/publication boundaries.
- [x] `ath docs generate-onboarding <PATH> --snapshot <EXACT-ID>` поддерживает hard-limit flags, `--force`,
  `--json` и Ctrl-C cancellation/drain.
- [x] `ath docs onboarding current|manifest|validation` проверяет profile, confinement, generation path,
  exact artifact layout, manifest/report identity и SHA-256.
- [x] Shared `current.json` изолирует architecture/module/API/operations/onboarding inspectors по profile.
- [x] Source regressions покрывают lifecycle, exact operation contract, profile isolation/path/checksum drift
  и binary `index -> generate-onboarding -> inspect -> UpToDate` round-trip.
- [ ] Focused format/test/Clippy execution evidence для onboarding 5A–5C pending; source implementation не verified.

#### Slices 6A–6B — documentation completeness

- [x] Slice 6A добавляет pure completeness owner на canonical baseline `file` inventory: processed/unprocessed
  paths, per-language basis-point coverage, named non-baseline adapter contribution, deterministic limits и omissions.
- [x] Canonical entity presence считается processing evidence без выдуманной attribution; baseline `basic` не
  маскирует coverage gaps.
- [x] Slice 6B добавляет exact Store loading, read-only `ath docs completeness`, cancellation/drain и
  зарегистрированный `athanor.documentation_completeness.v1` JSON transport.
- [x] Latest fallback, publication generations и shared `current.json` для completeness отсутствуют намеренно.
- [ ] Focused execution evidence для 6A–6B pending; source implementation не verified.

#### Slices 7A–7C — bounded framework route projections

- [x] Slice 7A: `NextJsExtractor` проецирует стандартные App/Pages Router filesystem conventions в
  adapter-scoped `nextjs_route` entities + `RouteDeclared` evidence без React/HTTP inference.
- [x] Slice 7B: `AxumExtractor` распознаёт bounded literal `.route()` declarations с поддерживаемыми
  `axum::routing` constructors и создаёт adapter-scoped `axum_route` knowledge.
- [x] Slice 7C: `ExpressExtractor` распознаёт explicit application/router bindings и exact two-argument
  literal route calls, создавая adapter-scoped `express_route` knowledge.
- [x] Base JS/TS и Rust extractors остаются framework-neutral; schemas/auth/middleware, route composition и
  handler linking сознательно отложены.
- [ ] Focused execution evidence для 7A–7C pending; source implementation не verified.

#### Slices 8A–8E — completeness-driven first-party operational semantics

- [x] 8A: bounded PowerShell environment references; exact `33676558603` confirms `ps1 = 2/2`.
- [x] 8B: root `athanor.toml` runtime config; exact `33680051765` confirms 665/718, TOML 34/35.
- [x] 8C: first-party composite actions; exact `33712810332` confirms 667/719, YAML 11/15.
- [x] 8D: `.github/dependabot.yml|yaml` version-2 update policies; exact `33714498524` confirms 669/720,
  YAML 12/15. Verification Matrix `33714498496` exposed rustfmt-only test hunks; #91 repaired them.
- [x] 8E: root `deny.toml` cargo-deny `advisories/licenses/bans/sources` summaries, selected scalar modes and
  list counts only. Exact `33722472453` confirms 670/720 (`9305` bps), TOML 35/35 and all four cargo-deny keys.
- [x] #93 fixed the one post-8E `clippy::collapsible_if`; PR CI `33723374660` confirmed formatting, workspace
  tests and Clippy green on macOS before squash merge to `2310215b61980de3300a93e43e10a68daa27f800`.
- [ ] Focused execution evidence for Slices 8A–8E remains pending; completeness evidence is not full promotion.

Existing coordinated `ath generate` is unchanged. Provider/LLM, daemon and MCP remain out of scope.
`DOCGEN-001` остаётся `[-] in progress`: later profile/completeness/framework surfaces and Slices 8A–8E are
source-implemented; focused verification remains pending.

### 4.2 Product backlog

- [ ] выбрать следующий bounded semantic gap из exact 8E artifact; сначала проверить root `install.sh` и
  `scripts/verify_release_version.py`, а не generic JSON/test fixtures;
- [ ] не добавлять generic JSON/fixture parsing только ради coverage без explicit product semantics;
- [ ] issue forms и OpenAPI fixture YAML не включать без отдельного evidence-backed scope;
- [ ] Next.js/Axum/Express schemas/auth/middleware и route composition расширять отдельными slices;
- [ ] optional provider, daemon, MCP, i18n and semantic retrieval after deterministic quality gates.

## 5. Программа работ

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[x] verified` | Audit run `29836572040` |
| `COMP-003` | P2 | `[x] implemented` | Explicit composition |
| `MCP-007` | P1 | `[x] implemented` | Transactional cancellation |
| `JSON-003` | P1 | `[x] implemented` | Checked schema lifecycle |
| `DOC-001` / `DOC-002` | P3 | `[x] implemented` | Status hygiene |
| `MCP-004` | P1 | `[x] implemented` | Responsive control plane |
| `VERIFY-001` | P1 | `[x] verified` | Full release baseline matrix |
| `API-001` | P1 | `[x] verified` | Cross-protocol consistency |
| `REL-001` | P1 | `[x] verified` | `v0.2.1` published and installed |
| `DOCGEN-001` | P2 | `[-] in progress` | Profiles 2A–5C + completeness 6A–6B + framework 7A–7C + Slices 8A–8E source-implemented |

## 6. Verification matrix

```bash
cargo-deny check
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
cargo test -p athanor-app --test documentation_module_profile_inventory --locked
cargo test -p athanor-app --test documentation_api_profile_inventory --locked
cargo test -p athanor-app --test documentation_operations_profile_inventory --locked
cargo test -p athanor-app --test documentation_onboarding_profile_inventory --locked
cargo test -p athanor-app --test documentation_architecture_publication_inventory --locked
cargo test -p athanor-app --test documentation_module_publication_inventory --locked
cargo test -p athanor-app --test documentation_api_publication_inventory --locked
cargo test -p athanor-app --test documentation_operations_publication_inventory --locked
cargo test -p athanor-app --test documentation_onboarding_publication_inventory --locked
cargo test -p athanor-app --test documentation_api_operation_inventory --locked
cargo test -p athanor-app --test documentation_operations_operation_inventory --locked
cargo test -p athanor-app --test documentation_onboarding_operation_inventory --locked
cargo test -p athanor-app --test documentation_architecture_inspection_inventory --locked
cargo test -p athanor-app --test documentation_module_inspection_inventory --locked
cargo test -p athanor-app --test documentation_api_inspection_inventory --locked
cargo test -p athanor-app --test documentation_operations_inspection_inventory --locked
cargo test -p athanor-app --test documentation_onboarding_inspection_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p ath --test documentation_architecture_cli --locked
cargo test -p ath --test documentation_module_cli --locked
cargo test -p ath --test documentation_api_cli --locked
cargo test -p ath --test documentation_operations_cli --locked
cargo test -p ath --test documentation_onboarding_cli --locked
cargo run -p ath --quiet --locked -- docs check
```

## 7. Следующий шаг

Выбрать Slice 8F из exact `33722472453`: предпочтение полезной first-party семантике. Root `install.sh`
уже подтверждён как отдельный unprocessed operational surface; до реализации проверить текущий shell parser и
ограничить scope без generic shell AST. Generic JSON/test fixtures не использовать как coverage target.
