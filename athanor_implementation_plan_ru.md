# Athanor: консолидированный план реализации

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-17  
> Статус: active implementation plan

## Правила статусов

- `[x] verified` — реализация и regressions подтверждены.
- `[-] in progress` — полезный срез в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — работа не начата.
- `[!] blocked` — требуется compatibility decision или недоступная платформенная проверка.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id. Эквивалентные CLI, daemon и MCP операции не должны иметь разные application payload shapes.

## Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |
| `DS-JSON-001` | P1 | `[-] in progress` | Registry содержит 45 public contracts с regressions |
| `DS-JSON-002` | P1 | `[-] in progress` | Scanner различает public, embedded, generated, persisted и interchange schemas |
| `DS-JSON-003` | P1 | `[-] in progress` | Context, Index, Generation, Wiki и HTML typed parity реализованы; Config CLI pending |

## DS-RESOLVE-003 — Validated Artifact Resolver Migration

**Статус:** `[x] verified`.

- [x] Generated read-model и index-state paths разрешаются pointer-first.
- [x] Path/type/schema/snapshot/generation/checksum identity проверяется до использования.
- [x] Runtime consumers используют shared resolver boundary.
- [x] Rustok resolver coverage сохранена после runtime decomposition.

## DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`.

### Contract foundation

- [x] App-layer модуль `json_contract` и trait `VersionedJsonContract`.
- [x] Canonical schema-id validator и typed `JsonContractError`.
- [x] Проверка top-level object/string `schema` и instance/associated-constant equality.
- [x] Registry отклоняет duplicate schema id и duplicate Rust owner.
- [x] Owned, static-schema и transparent-wrapper contract macros.
- [x] Constant-backed owner implementation для custom-serialized `IndexReport`.

### Зарегистрированные public families

- [x] Overview/Search, Explain/Impact, Check family.
- [x] Coverage/Capabilities, ChangeMap и ContextReport.
- [x] Core Graph: Export/Related/Path/Hubs/PageRank/Cycles.
- [x] ProjectRegistryReport и ProjectResolutionReport.
- [x] RustokArchitectureContext.
- [x] RusTok FFA, FBA и Page Builder audit/graph families.
- [x] IndexReport, BenchmarkReport и ChangedValidationReport.
- [x] GenerationReport.
- [x] ConfigValidateReport и ConfigDoctorReport.
- [x] DocsCheckReport, DocsDriftReport и DocsApplyPatchReport.
- [x] ApiContractDiff и ApiCleanupReport.
- [x] WikiReport и HtmlReport.
- [x] Всего 45 public report type реализуют общий contract trait.

### Compatibility migrations

- [x] `ContextReport` добавляет top-level schema через flatten без нового nesting.
- [x] Persisted Project Registry отделён от public report schema; legacy input мигрирует при mutation.
- [x] Shared RusTok graph calculation types разделены transparent command-specific owners.
- [x] Direct CLI Index/Update и daemon index job используют один versioned `IndexReport`.
- [x] Direct CLI Generate и daemon generation job используют один versioned `GenerationReport`.
- [x] Wiki и HTML app results и daemon jobs используют соответствующие typed reports.
- [x] Wiki/HTML daemon fields сохранены; `schema` и `root` добавлены аддитивно.
- [x] Config Validate сохраняет flattened effective config и добавляет top-level `schema`/`root`.
- [x] Config Doctor получил typed owner без изменения существующей `{ schema, root, config, checks }` формы.
- [-] Direct CLI Config всё ещё сериализует legacy raw/ad-hoc values; wiring на app-layer reports не завершён.
- [x] Index и Generation metrics классифицированы как embedded fragments.
- [x] Validation result и Generation manifest/current pointer классифицированы как generated artifacts.
- [x] `DocsPatchProposal` классифицирован как interchange artifact.
- [x] `ApiContractDiff` использует один shape как public response и persisted diff artifact.
- [x] API snapshot/latest pointer классифицированы как generated artifacts.
- [!] `DocsProposeFixReport` остаётся unversioned top-level wrapper вокруг versioned proposal.
- [!] `ApiSnapshotReport` остаётся unversioned command summary вокруг generated snapshot artifact.

### Golden и integration regressions

- [x] Existing first-wave, Context, Project Registry, core Graph и RusTok fixtures.
- [x] Application-output fixture защищает IndexReport, BenchmarkReport и ChangedValidationReport.
- [x] Generation/docs fixture защищает GenerationReport, public Docs reports и DocsPatchProposal.
- [x] API fixture защищает public Diff/Cleanup и generated Snapshot/Latest shapes.
- [x] Wiki/HTML fixture защищает оба application report shapes.
- [x] Config fixture защищает Validate и Doctor application report shapes.
- [x] Daemon unit regressions сравнивают Index, Generation, Wiki и HTML payloads с direct serialization.
- [x] Registry unit regressions защищают 45 schema id и ownership.
- [x] Inventory regression проверяет взаимную исключительность classifications и сканирует `config.rs`.
- [!] Локальный Rust toolchain отсутствует; fmt/test/Clippy не заявляются выполненными.
- [!] Hosted Actions/check status для `main` не виден через connector.

### Следующий срез

- [x] Реализовать versioned IndexReport и typed CLI/daemon payload parity.
- [x] Зарегистрировать GenerationReport и typed CLI/daemon parity.
- [x] Зарегистрировать Docs Check/Drift/Apply reports.
- [x] Классифицировать generation artifacts/metrics и docs patch interchange.
- [x] Зарегистрировать API Diff/Cleanup и классифицировать Snapshot/Latest artifacts.
- [x] Реализовать versioned WikiReport и HtmlReport с daemon parity.
- [x] Создать typed app-layer Config Validate/Doctor owners, registry entries и golden fixture.
- [ ] Перевести direct CLI Config Validate/Doctor на typed app-layer reports.
- [ ] Спроектировать versioned ApiSnapshotReport и DocsProposeFixReport wrappers.
- [ ] Инвентаризировать repair JSON outputs и state.
- [ ] Инвентаризировать daemon request/response/error envelopes.
- [ ] Инвентаризировать MCP JSON-RPC и tool-content envelopes.
- [ ] Инвентаризировать extractor/linker/checker process protocols.
- [ ] Инвентаризировать index/publication/read-model persisted documents.
- [ ] Классифицировать projector payloads/manifests и remaining pointer documents.
- [ ] Добавить executable CLI/daemon parity regressions.
- [ ] Выполнить targeted и workspace verification.

### Definition of Done

`DS-JSON-001` станет `[x] verified`, когда remaining application/transport/process/persistence inventory завершена, executable parity regressions пройдены, fixtures выполняются, а fmt, targeted tests, workspace tests и Clippy зелёные.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

### Завершено

- [x] Создан `docs/development/json-contract-inventory.md`.
- [x] Зафиксированы 45 public contracts и их Rust owners.
- [x] Scanner охватывает public read/project/Rustok, index/benchmark/validation, generation, Config, docs, API, Wiki и HTML modules.
- [x] Public migration classification в текущем schema-bearing scope пуст.
- [x] Persisted: `athanor.project_registry_state.v1`.
- [x] Generated: validation result, generation manifest/current pointer, API snapshot/latest pointer.
- [x] Embedded: index metrics, index-report metrics и generation metrics.
- [x] Interchange: `athanor.docs_patch.v1`.
- [x] Все classification sets взаимно исключительны.
- [x] Classified schemas должны присутствовать в inventoried source и не могут быть public registry owners.

### Осталось

- [ ] Direct CLI Config wiring, ApiSnapshot/DocsProposeFix wrappers и Repair application outputs/state.
- [ ] Daemon/MCP envelope inventory и enforcement.
- [ ] Extractor/linker/checker process protocols inventory.
- [ ] Index state/publication journal/read-model manifest inventory.
- [ ] Projector payloads/manifests, repair journals/guards и remaining pointers classification.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`.

- [x] Direct CLI, daemon и active MCP Context используют `ContextReport`.
- [x] Direct CLI Index/Update и daemon index job используют один `IndexReport` shape.
- [x] Direct CLI Generate и daemon generation job используют один `GenerationReport` shape.
- [x] Wiki app result и daemon Wiki job используют один `WikiReport` shape.
- [x] HTML app result и daemon HTML job используют один `HtmlReport` shape.
- [x] Unit regressions сравнивают daemon Index/Generation/Wiki/HTML payloads с direct serialization.
- [x] Operation-aware Rustok outputs используют зарегистрированные contracts.
- [-] Config typed application shapes зарегистрированы; direct CLI пока не использует их.
- [ ] Добавить executable CLI/daemon Index parity regression.
- [ ] Добавить executable CLI/daemon Generation parity regression.
- [ ] Добавить executable Context parity regression.
- [ ] Добавить executable Wiki/HTML parity regressions.
- [ ] Добавить executable Config Validate/Doctor contract regressions после CLI wiring.
- [ ] Распространить parity enforcement на остальные общие operations.

## Оценка остатка

Осталось **3 implementation-пакета и 1 verification-пакет**, при этом Config application foundation завершена:

1. Remaining application outputs: direct CLI Config wiring, ApiSnapshot, DocsProposeFix и Repair reports/state.
2. Transport boundaries: daemon request/response/error envelopes и MCP JSON-RPC/tool-content envelopes.
3. Process + persistence boundaries: extractor/linker/checker protocols, index/publication/read-model manifests, projector documents, repair journals/guards и pointers.
4. Verification: executable parity, fmt, targeted tests, workspace tests и Clippy.

По объёму реализации осталось ориентировочно **18–28% DS-JSON работы**. Подтверждённая verification отстаёт сильнее, потому что Rust toolchain и hosted status сейчас недоступны.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test versioned_json_contracts --locked
cargo test -p athanor-app --test application_output_contracts --locked
cargo test -p athanor-app --test generation_docs_contracts --locked
cargo test -p athanor-app --test api_contracts --locked
cargo test -p athanor-app --test wiki_html_contracts --locked
cargo test -p athanor-app --test config_contracts --locked
cargo test -p athanor-app daemon_index_result_matches_public_index_report_shape --locked
cargo test -p athanor-app daemon_generation_result_matches_public_generation_report_shape --locked
cargo test -p athanor-app daemon_wiki_result_matches_public_wiki_report_shape --locked
cargo test -p athanor-app daemon_html_result_matches_public_html_report_shape --locked
cargo test -p athanor-app --test context_report_contract --locked
cargo test -p athanor-app --test project_registry_report_contract --locked
cargo test -p athanor-app --test rustok_architecture_context_contract --locked
cargo test -p athanor-app --test rustok_ffa_contracts --locked
cargo test -p athanor-app --test rustok_fba_contracts --locked
cargo test -p athanor-app --test rustok_page_builder_contracts --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app json_contract --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Активный рабочий пакет

**Сейчас:** `DS-JSON-001/002/003` — 45 public contracts; typed Config application owners, registry entries, golden fixture и inventory enforcement реализованы; verification pending.

**Дальше:** перевести direct CLI Config Validate/Doctor на typed reports, затем ApiSnapshot, DocsProposeFix и Repair, после чего перейти к transport и process/persistence boundaries.

## Журнал

### 2026-07-17 — DS-JSON-001/002 Config typed report foundation

- Зарегистрированы `athanor.config_validate.v1` и `athanor.config_doctor.v1`.
- `ConfigValidateReport` сохраняет flattened effective config и добавляет `schema`/`root`.
- `ConfigDoctorReport` сохраняет существующую форму и использует typed check entries.
- Добавлены application builders, golden fixture и contract regression.
- Inventory scanner расширен на `src/config.rs`.
- Registry вырос с 43 до 45 public contracts.
- Direct CLI wiring и executable parity оставлены активным следующим срезом.
- Локальный fmt/test/Clippy и hosted Actions не подтверждены из-за недоступности toolchain/status через текущую среду.

### 2026-07-17 — DS-JSON-001/002/003 Wiki и HTML parity

- Зарегистрированы `athanor.wiki_report.v1` и `athanor.html_report.v1`.
- App-layer reports получили обязательные top-level schema fields.
- Daemon Wiki/HTML jobs переведены с reduced objects на полную typed serialization.
- Добавлены golden fixture и четыре-level daemon serialization regressions для Index/Generation/Wiki/HTML.
- Projector input schemas оставлены отдельными generated boundaries.
- Registry вырос с 41 до 43 public contracts.

### 2026-07-17 — DS-JSON-001/002 API contracts

- Зарегистрированы `athanor.api_contract_diff.v2` и `athanor.api_cleanup.v1`.
- `ApiContractDiff` сохраняет один shape для CLI response и persisted diff artifact.
- API snapshot v2 и latest pointer v1 классифицированы как generated.
- Добавлен shared golden fixture для public и generated API shapes.
- `ApiSnapshotReport` зафиксирован как отдельный unversioned wrapper blocker.
- Registry вырос с 39 до 41 public contracts.

### 2026-07-17 — DS-JSON-001/002/003 Generation и Docs

- Зарегистрирован `athanor.generation.v1` с owner `GenerationReport`.
- Daemon generation result переведён на полную typed serialization.
- Зарегистрированы Docs Check, Drift и Apply Patch reports.
- Generation artifacts/metrics и Docs patch interchange классифицированы.

### 2026-07-17 — DS-JSON-001/003 IndexReport parity

- Зарегистрирован `athanor.index_report.v1` с owner `IndexReport`.
- Direct CLI Index/Update и daemon index job используют один typed shape.

### 2026-07-17 — DS-JSON-001/002 application outputs

- Зарегистрированы `athanor.index_benchmark.v1` и `athanor.changed_validation.v1`.
- Index metrics классифицированы как embedded; validation result — generated.

### 2026-07-16 — DS-JSON-001/002 Rustok Architecture и specialized families

- Зарегистрированы Architecture, FFA, FBA и Page Builder contracts.
- Shared graph calculation types получили transparent command-specific owners.

### 2026-07-16 — DS-JSON-001/002 Project Registry schema migration

- Public report и persisted state разделены; legacy input мигрирует при mutation.

### 2026-07-16 — DS-JSON-001/003 Context contract migration

- Direct CLI, daemon и active MCP Context используют flattened `ContextReport`.

### 2026-07-16 — DS-JSON-001 foundation и shared-literal migrations

- Созданы shared trait, registry, validation и typed errors.
- Search, Impact, Check family и ChangeMap переведены на shared constants.
