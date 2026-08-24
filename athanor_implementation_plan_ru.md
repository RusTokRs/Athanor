# Athanor: план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-08-24  
> Статус: `API-001`, `REL-001` verified; `DOCGEN-001 / Slices 0A–1C` execution-confirmed;
> relation-disclosure exact-evaluation-confirmed; module 2A–2C, API 3A–3C, operations 4A–4C и
> onboarding 5A–5B implemented in source; focused execution evidence pending.

## 1. Статусы и evidence

- `[x] implemented` — код, документация и source regressions присутствуют.
- `[-] in progress` — package Definition of Done закрыт частично.
- `[x] verified` — required matrix успешна на одном exact source commit.
- `[ ] planned` — следующий bounded этап.

Promotion требует успешных `athanor/verification-matrix`, `athanor/appsec` и
`athanor/store-conformance` на одном SHA, когда такой matrix входит в Definition of Done конкретного
пакета. Наличие кода или metadata не является execution evidence.

## 2. Baselines

Release baseline `609027eb02caa05346ebfea8538552c42b588c31`: CI `29995959544`, AppSec
`29995960063`, Store `29995959512`, Release `29996579628`, clean-install smoke `29998347890`.
Annotated `v0.2.1` указывает на этот SHA.

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
- Slice 2A landed on `658d53fb03dd47a971beb6cf67b46cfe1f20b3fe`; post-merge Rustok evaluation
  `32714912841`, Rustok probe `32714912826`, AppSec `32714912809`, Store `32714912932` were green;
- Slice 2C landed on `b9e0eadc46175e15ec62f915a4729287f3884cd2`; Store `32718598218`, Rustok
  evaluation `32718598212`, Rustok probe `32718598232` were green;
- Slice 3A landed on `0a4c0f78ef05b6c2ba9480b770c1ebf72038e049`; Rustok evaluation
  `32719989413` and Rustok probe `32719989450` were green.

Последние post-merge contexts не заменяют focused execution evidence соответствующих профилей.

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

- [x] Strict request/manifest, hard limits, omissions, portable paths и checksums.
- [x] Versioned outline/context/citation/draft/validation contracts.
- [x] Data policy, quality metrics, fixture repository и Rustok evaluation corpus.
- [x] Full chain alignment и fail-closed regressions.

#### Slices 1A–1C — architecture production surface

- [x] Exact `CanonicalSnapshot`, deterministic ordering, hard limits, cited Markdown/Mermaid.
- [x] Immutable generation publication, validation report и atomic `current.json`.
- [x] Slice 1C1 exact Store loading через `RuntimeComposition::init_store` и
  `CanonicalSnapshotStore::load_snapshot`; latest fallback отсутствует.
- [x] Slice 1C2 `ath docs generate-architecture <PATH> --snapshot <EXACT-ID>` и validated
  `architecture current|manifest|validation`.
- [x] Exact matrix подтверждена на Slice 1C1/1C2 SHA и run IDs из раздела 2.

#### Bounded Rustok evaluation — repaired gate и artifact review closed

- [x] Первый gate `5e0b28099c48e22bdc172fa57b6d51db9e6efb7b` / `30029451096` остановился на
  `documentation draft citations must contain between 1 and 256 entries`.
- [x] Probe `12a8687c5d098ab05a5988508816aad5f0dc3e23` / `30030131126` зафиксировал repair path.
- [x] `f1024cbc52f05de4d3ce96c556ef044ad48b3a0e` полностью green по evaluation/probe/CI/AppSec/Store.
- [x] `6862aee8…` добавил human-facing relation disclosure; exact evaluation/probe успешны.

#### Slices 2A–2C — module documentation profile

- [x] Pure module inventory, scoped facts/relations/open diagnostics и shared 256-item budget.
- [x] Immutable `modules/index.md`, exact Store operation, `generate-module`, validated inspection.
- [ ] Focused format/test/Clippy execution evidence pending; source implementation не verified.

#### Slices 3A–3C — API documentation profile

- [x] Endpoint/schema/example inventory, scoped facts, canonical relation allowlist, open diagnostics.
- [x] Aggregate 256 budget, cited Mermaid, immutable `api/index.md`, exact Store, CLI/inspection.
- [ ] Focused format/test/Clippy execution evidence pending; source implementation не verified.

#### Slices 4A–4C — operations documentation profile

- [x] Six-category operations inventory, scoped facts, bounded relations, open diagnostics, cited Mermaid.
- [x] Immutable `operations/index.md`, exact Store, `generate-operations`, validated inspection.
- [x] Existing `ath docs operations check` и coordinated generation не изменены.
- [ ] Focused format/test/Clippy execution evidence pending; source implementation не verified.

#### Slices 5A–5B — onboarding documentation profile

- [x] `DocumentationProfile::Onboarding` и pure exact-snapshot `build_documentation_onboarding_profile`.
- [x] Anchor scope: `DocumentationPage`, `DocumentationSection`, `Package`, `ScriptCommand`, `EnvVar`,
  `TestCase`/`CiJob`; six-category stable-key/entity-id round-robin сохраняет low-limit fairness.
- [x] Slice 5B добавляет facts, если subject или object — selected onboarding anchor.
- [x] Bounded supported relations: `Contains`, `Documents`, `UsesEnv`, `TestedBy`; relation должна касаться
  selected anchor и иметь portable evidence. Общие `Calls`/`Imports`/change relations не входят в профиль.
- [x] Только `Open` diagnostics, ссылающиеся на selected onboarding anchor, входят в bounded context.
- [x] Per-kind limits применяются до aggregate `DOCUMENTATION_REFERENCE_LIMIT = 256`; финальный selection
  round-robin по Entity/Fact/Relation/Diagnostic, omissions считаются по eligible onboarding scope.
- [x] Каждый context item имеет citation; supported relations получают cited Mermaid edges;
  `unsupported_relations = context.omitted.relations` и disclosure присутствует в Markdown.
- [x] `onboarding/index.md` остаётся deterministic SHA-256-bound pure output; rendering/validation вынесены
  в узкий sibling module, portable evidence использует общий owner.
- [x] Source regressions покрывают scope leakage, unsupported relations, resolved/unrelated diagnostics,
  input-order invariance, relation omissions и смешанный 256-item aggregate budget.
- [ ] Focused format/test/Clippy execution evidence для onboarding 5A–5B pending; source implementation не verified.

Existing coordinated `ath generate` is unchanged. Provider/LLM, daemon and MCP remain out of scope.
`DOCGEN-001` остаётся `[-] in progress`: module/API/operations production surfaces и onboarding 5A–5B
source-implemented; focused verification и Slice 5C остаются отдельными packages.

### 4.2 Product backlog

- [ ] Slice 5C: immutable onboarding publication, exact Store operation, CLI и validated inspection;
- [ ] broader framework adapters and completeness reports;
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
| `DOCGEN-001` | P2 | `[-] in progress` | Module 2A–2C + API 3A–3C + operations 4A–4C + onboarding 5A–5B source-implemented; execution pending |

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
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p ath --test documentation_architecture_cli --locked
cargo test -p ath --test documentation_module_cli --locked
cargo test -p ath --test documentation_api_cli --locked
cargo test -p ath --test documentation_operations_cli --locked
cargo run -p ath --quiet --locked -- docs check
```

## 7. Следующий шаг

Получить focused format/test/Clippy execution evidence для module 2B–2C, API 3A–3C, operations 4A–4C и onboarding 5A–5B.
Следующим отдельным bounded package сделать Slice 5C: immutable onboarding publication, exact Store operation,
CLI и validated inspection; не подключать provider/daemon/MCP и не менять coordinated `ath generate`.
