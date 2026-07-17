# Athanor: консолидированный план реализации

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-17  
> Статус: active implementation plan

## Правила статусов

- `[x] verified` — реализация и regressions подтверждены выполненными проверками.
- `[-] in progress` — полезный срез находится в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — работа не начата.
- `[!] blocked` — требуется compatibility decision или недоступная платформенная проверка.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id. Эквивалентные CLI, daemon и MCP операции не должны иметь разные application payload shapes.

## Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |
| `DS-JSON-001` | P1 | `[-] in progress` | Registry содержит 56 public contracts с regressions |
| `DS-JSON-002` | P1 | `[-] in progress` | Application outputs инвентаризированы; transport/process/persistence boundaries pending |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed CLI/application parity охватывает основные operations и Repair; verification pending |

## DS-RESOLVE-003 — Validated Artifact Resolver Migration

**Статус:** `[x] verified`.

- [x] Generated read-model и index-state paths разрешаются pointer-first.
- [x] Path/type/schema/snapshot/generation/checksum identity проверяется до использования.
- [x] Runtime consumers используют shared resolver boundary.
- [x] Rustok resolver coverage сохранена после runtime decomposition.

## DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`.

### Contract foundation

- [x] App-layer `json_contract` facade и trait `VersionedJsonContract`.
- [x] Canonical schema-id validator и typed `JsonContractError`.
- [x] Проверка top-level object/string `schema` и instance/associated-constant equality.
- [x] Registry отклоняет duplicate schema id и duplicate Rust owner.
- [x] Existing implementation изолирована в `json_contract_base.rs`; facade расширяет canonical registry.

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
- [x] Docs Check/Drift/Apply/Propose Fix reports.
- [x] API Snapshot/Diff/Cleanup reports.
- [x] WikiReport и HtmlReport.
- [x] Repair Inspect/Cleanup/Regenerate/Recover Canonical/Apply reports.
- [x] Transactional Repair Index Retention/Recover Index/Recover Cleanup/Canonical Latest reports.
- [x] Всего 56 public report type реализуют общий contract trait.

### Compatibility migrations

- [x] Context, Config и remaining application wrappers используют additive flatten без нового nesting.
- [x] Direct CLI Index/Update и daemon index job используют один versioned `IndexReport`.
- [x] Direct CLI Generate и daemon generation job используют один versioned `GenerationReport`.
- [x] Wiki и HTML app results и daemon jobs используют соответствующие typed reports.
- [x] Direct CLI Config Validate/Doctor используют typed app-layer reports.
- [x] Direct CLI API Snapshot и Docs Propose Fix используют additive versioned wrappers.
- [x] Generated API snapshot и Docs patch interchange остаются отдельными non-public документами.
- [x] Девять Repair report shapes зарегистрированы без изменения существующего CLI JSON.
- [x] Public Repair Inspect/Cleanup/Apply используют current v2 schemas; historical internal v1 constructors не считаются внешними контрактами.
- [x] Repair state summaries, issues, cleanup rows и tombstones классифицированы как embedded/filesystem protocol.
- [x] `athanor.index_current_publication.v1` классифицирован как persisted recovery journal.
- [x] Index/Generation metrics классифицированы как embedded; generated/interchange документы отделены от public registry.

### Golden и integration regressions

- [x] Existing first-wave, Context, Project Registry, core Graph и RusTok fixtures.
- [x] Application-output, Generation/Docs, API, Wiki/HTML и Config fixtures.
- [x] Remaining-application fixture для API Snapshot и Docs Propose Fix wrappers.
- [x] Repair family fixture для девяти report owners.
- [x] Executable Config CLI regressions.
- [x] Existing executable Repair CLI regressions для transactional commands.
- [x] Direct help regression для API Snapshot/Docs Propose Fix.
- [x] Daemon serialization regressions для Index/Generation/Wiki/HTML.
- [x] Extended registry regression защищает 56 schema id и ownership.
- [x] Inventory regressions запрещают смешивать public и embedded Repair types.
- [!] Локальный Rust toolchain отсутствует; fmt/test/Clippy не заявляются выполненными.
- [!] Hosted Actions/check status для `main` не виден через connector.

### Следующий срез

- [x] Versioned application outputs и direct CLI migrations.
- [x] Repair public outputs и embedded state inventory.
- [ ] Инвентаризировать daemon request/response/error envelopes.
- [ ] Инвентаризировать MCP JSON-RPC и tool-content envelopes.
- [ ] Инвентаризировать extractor/linker/checker process protocols.
- [ ] Инвентаризировать index/publication/read-model persisted documents.
- [ ] Классифицировать projector payloads/manifests, repair guards и remaining pointer documents.
- [ ] Добавить executable CLI/daemon parity regressions для оставшихся общих операций.
- [ ] Выполнить targeted и workspace verification.

### Definition of Done

`DS-JSON-001` станет `[x] verified`, когда transport/process/persistence inventory завершена, executable parity regressions пройдены, fixtures выполняются, а fmt, targeted tests, workspace tests и Clippy зелёные.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

### Завершено

- [x] Создан `docs/development/json-contract-inventory.md`.
- [x] Зафиксированы 56 public contracts и их Rust owners.
- [x] Known application-output blockers устранены в audited scope.
- [x] Public migration classification в текущем schema-bearing scope пуст.
- [x] Persisted: Project Registry state и index-current publication journal.
- [x] Generated: validation result, generation manifest/current pointer, API snapshot/latest pointer.
- [x] Embedded: metrics и Repair state/row fragments.
- [x] Interchange: `athanor.docs_patch.v1`.
- [x] Все classification sets взаимно исключительны.
- [x] Repair inventory regression подтверждает девять owners и исключает embedded types.

### Осталось

- [ ] Daemon/MCP envelope inventory и enforcement.
- [ ] Extractor/linker/checker process protocols inventory.
- [ ] Index-current/index-state/publication/read-model manifest inventory.
- [ ] Projector payloads/manifests, repair guards, generated/canonical pointers и remaining persisted documents.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`.

- [x] Direct CLI, daemon и active MCP Context используют `ContextReport`.
- [x] Direct CLI Index/Update и daemon index job используют один `IndexReport` shape.
- [x] Direct CLI Generate и daemon generation job используют один `GenerationReport` shape.
- [x] Wiki/HTML app results и daemon jobs используют typed reports.
- [x] Direct CLI Config Validate/Doctor используют typed reports.
- [x] Direct CLI API Snapshot и Docs Propose Fix используют versioned wrappers.
- [x] Repair CLI reports имеют shared registry ownership; transactional schemas защищены executable regressions.
- [x] Operation-aware Rustok outputs используют зарегистрированные contracts.
- [ ] Добавить executable CLI/daemon Index parity regression.
- [ ] Добавить executable CLI/daemon Generation parity regression.
- [ ] Добавить executable Context parity regression.
- [ ] Добавить executable Wiki/HTML parity regressions.
- [ ] Распространить parity enforcement на remaining shared operations.

## Оценка остатка

Application-output inventory завершена по реализации. Осталось **2 implementation-пакета и 1 verification-пакет**:

1. Transport boundaries: daemon request/response/error envelopes и MCP JSON-RPC/tool-content envelopes.
2. Process + persistence boundaries: extractor/linker/checker protocols, index/publication/read-model/projector documents, repair guards и pointers.
3. Verification: executable parity, fmt, targeted tests, workspace tests и Clippy.

По объёму реализации осталось ориентировочно **10–18% DS-JSON работы**. Подтверждённая verification отстаёт сильнее, потому что Rust toolchain и hosted status сейчас недоступны.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test versioned_json_contracts --locked
cargo test -p athanor-app --test application_output_contracts --locked
cargo test -p athanor-app --test generation_docs_contracts --locked
cargo test -p athanor-app --test api_contracts --locked
cargo test -p athanor-app --test wiki_html_contracts --locked
cargo test -p athanor-app --test config_contracts --locked
cargo test -p athanor-app --test remaining_application_contracts --locked
cargo test -p athanor-app --test remaining_application_contract_inventory --locked
cargo test -p athanor-app --test repair_contracts --locked
cargo test -p athanor-app --test repair_contract_inventory --locked
cargo test -p ath --test direct_config_cli --locked
cargo test -p ath --test direct_application_report_cli --locked
cargo test -p ath --test repair_cli --locked
cargo test -p athanor-app daemon_index_result_matches_public_index_report_shape --locked
cargo test -p athanor-app daemon_generation_result_matches_public_generation_report_shape --locked
cargo test -p athanor-app daemon_wiki_result_matches_public_wiki_report_shape --locked
cargo test -p athanor-app daemon_html_result_matches_public_html_report_shape --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app json_contract --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Активный рабочий пакет

**Сейчас:** `DS-JSON-001/002/003` — 56 public contracts; application outputs и Repair inventory реализованы; verification pending.

**Дальше:** daemon/MCP envelope inventory, затем process/persistence boundaries.

## Журнал

### 2026-07-17 — DS-JSON-001/002 Repair reports и state inventory

- Зарегистрированы девять Repair report owners.
- Registry вырос с 47 до 56 public contracts.
- Public Inspect/Cleanup/Apply current schemas закреплены как v2; остальные Repair reports — v1.
- Добавлены shared ownership implementations, family golden fixture и inventory regression.
- Embedded repair states/rows/tombstones исключены из public registry.
- `athanor.index_current_publication.v1` классифицирован как persisted journal; остальная pointer/manifest enforcement перенесена в persistence package.
- Existing Repair CLI JSON shapes не изменены.
- Локальный запуск fmt/test/Clippy не выполнен: Rust toolchain и исходящий network в среде отсутствуют; hosted status через connector не виден.

### 2026-07-17 — DS-JSON-001/002/003 remaining application wrappers

- Добавлены `athanor.api_snapshot.v1` и `athanor.docs_propose_fix.v1`.
- Registry расширен с 45 до 47 owners.
- `api snapshot` и `docs propose-fix` направлены через direct dispatcher.

### 2026-07-17 — DS-JSON-001/003 direct Config CLI parity

- Config Validate/Doctor переведены на typed reports.
- Добавлен executable regression; human-readable output сохранён.

### 2026-07-17 — DS-JSON-001/002 Config typed report foundation

- Зарегистрированы Config Validate/Doctor; registry вырос с 43 до 45.

### 2026-07-17 — DS-JSON-001/002/003 Wiki и HTML parity

- Зарегистрированы Wiki/HTML reports; daemon jobs переведены на typed serialization.

### 2026-07-17 — DS-JSON-001/002 API, Generation и Docs

- Зарегистрированы API Diff/Cleanup, Generation и public Docs reports.
- Generated, embedded и interchange boundaries классифицированы.

### 2026-07-16 — DS-JSON foundation и specialized families

- Созданы shared trait, registry, validation и typed errors.
- Зарегистрированы Project Registry, Context, Architecture, FFA, FBA и Page Builder families.
