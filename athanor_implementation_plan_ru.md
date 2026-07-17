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
| `DS-JSON-001` | P1 | `[-] in progress` | Registry содержит 34 golden-protected public contracts; IndexReport остаётся structural blocker |
| `DS-JSON-002` | P1 | `[-] in progress` | Scanner различает public, embedded, generated и persisted schemas |
| `DS-JSON-003` | P1 | `[-] in progress` | Context parity реализована; Index и остальные operations остаются |

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

### Зарегистрированные public families

- [x] Overview/Search, Explain/Impact, Check family.
- [x] Coverage/Capabilities, ChangeMap и ContextReport.
- [x] Core Graph: Export/Related/Path/Hubs/PageRank/Cycles.
- [x] ProjectRegistryReport и ProjectResolutionReport.
- [x] RustokArchitectureContext.
- [x] RusTok FFA, FBA и Page Builder audit/graph families.
- [x] `BenchmarkReport` для `athanor.index_benchmark.v1`.
- [x] `ChangedValidationReport` для `athanor.changed_validation.v1`.
- [x] Всего 34 public report types реализуют общий contract trait.

### Compatibility migrations

- [x] `ContextReport` добавляет top-level schema через flatten без нового nesting.
- [x] Persisted Project Registry отделён от public report schema; legacy input мигрирует при mutation.
- [x] Shared RusTok graph calculation types разделены transparent command-specific owners.
- [x] Benchmark и Changed Validation зарегистрированы без изменения существующего CLI JSON shape.
- [x] `IndexPipelineMetrics` и `IndexReportMetrics` классифицированы как embedded fragments.
- [x] `athanor.validation_result.v1` классифицирован как generated artifact.
- [!] `IndexReport` остаётся unversioned top-level CLI object; daemon index job публикует отличающийся reduced result.
- [ ] Спроектировать один versioned index result shape, сохраняющий все существующие CLI fields.
- [ ] Перевести direct CLI Index/Update и daemon job result на общий typed document.

### Golden и integration regressions

- [x] Existing first-wave, Context, Project Registry, core Graph и RusTok fixtures.
- [x] Application-output fixture защищает BenchmarkReport и ChangedValidationReport.
- [x] Benchmark fixture защищает nested IndexReport, IndexReportMetrics и IndexPipelineMetrics shapes.
- [x] Registry unit regressions защищают schema ids и ownership.
- [x] Inventory regression проверяет взаимную исключительность classifications.
- [!] Локальный Rust toolchain отсутствует; fmt/test/Clippy не заявляются выполненными.
- [!] Hosted Actions/check status для `main` не виден через connector.

### Следующий срез

- [x] Зарегистрировать BenchmarkReport и ChangedValidationReport.
- [x] Классифицировать index metrics как embedded и validation result как generated.
- [ ] Реализовать versioned IndexReport document и CLI/daemon parity.
- [ ] Инвентаризировать config/API/docs/generation/wiki/HTML/repair JSON outputs.
- [ ] Инвентаризировать daemon request/response/error envelopes.
- [ ] Инвентаризировать MCP JSON-RPC и tool-content envelopes.
- [ ] Инвентаризировать extractor/linker/checker process protocols.
- [ ] Инвентаризировать index/publication/generation/read-model persisted documents.
- [ ] Выполнить targeted и workspace verification.

### Definition of Done

`DS-JSON-001` станет `[x] verified`, когда remaining application/transport/process/persistence inventory завершена, parity regressions пройдены, fixtures выполняются, а fmt, targeted tests, workspace tests и Clippy зелёные.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

### Завершено

- [x] Создан `docs/development/json-contract-inventory.md`.
- [x] Зафиксированы 34 public contracts и их Rust owners.
- [x] Scanner охватывает public read/project/Rustok plus benchmark/index-metrics/changed-validation modules.
- [x] Public migration classification в текущем scope пуст.
- [x] Persisted classification: `athanor.project_registry_state.v1`.
- [x] Generated classification: `athanor.validation_result.v1`.
- [x] Embedded classifications: `athanor.index_metrics.v1`, `athanor.index_report_metrics.v1`.
- [x] Public, migration, persisted, generated и embedded sets взаимно исключительны.
- [x] Classified schemas должны присутствовать в inventoried source и не могут быть public registry owners.

### Осталось

- [ ] Unversioned IndexReport structural migration.
- [ ] Remaining application outputs inventory.
- [ ] Daemon/MCP envelope inventory и enforcement.
- [ ] Extractor/linker/checker process protocols inventory.
- [ ] Index state/publication journal/generation pointer/read-model manifest inventory.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`.

- [x] Direct CLI, daemon и active MCP Context используют `ContextReport`.
- [x] Textовый Context CLI сохраняет прежний вывод.
- [x] Operation-aware Rustok outputs используют зарегистрированные contracts.
- [!] Index CLI и daemon job result имеют разные top-level shapes и не имеют общего versioned owner.
- [ ] Добавить typed IndexReport document и executable CLI/daemon parity regression.
- [ ] Добавить executable Context parity regression.
- [ ] Распространить parity enforcement на остальные общие read-only operations.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test versioned_json_contracts --locked
cargo test -p athanor-app --test application_output_contracts --locked
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

**Сейчас:** `DS-JSON-001/002/003` — 34 public contracts; application-output classifications добавлены; verification pending.

**Дальше:** versioned IndexReport document с одним CLI/daemon shape, затем remaining application outputs и transport/process/persistence inventory.

## Журнал

### 2026-07-17 — DS-JSON-001/002 application outputs

- Зарегистрированы `athanor.index_benchmark.v1` и `athanor.changed_validation.v1`.
- Добавлен shared golden fixture для BenchmarkReport и ChangedValidationReport.
- Fixture защищает nested IndexReport, report metrics и pipeline metrics.
- `index_metrics.v1` и `index_report_metrics.v1` классифицированы как embedded fragments.
- `validation_result.v1` классифицирован как generated artifact.
- Scanner расширен на bench/index runtime/pipeline/changed-validation sources.
- Выявлен следующий blocker: standalone IndexReport не имеет schema, daemon публикует reduced shape.

### 2026-07-16 — DS-JSON-001/002 Rustok Architecture и specialized families

- Зарегистрирован `athanor.rustok_architecture_context.v1`.
- Зарегистрированы все FFA, FBA и Page Builder audit/graph schema ids.
- Shared graph calculation types получили transparent command-specific owners.
- Registry вырос с 19 до 32 public contracts.

### 2026-07-16 — DS-JSON-001/002 Project Registry schema migration

- Public report сохранил `athanor.project_registry.v1`.
- Persisted state переведён на `athanor.project_registry_state.v1`.
- Legacy input мигрирует при следующей mutation.

### 2026-07-16 — DS-JSON-001/003 Context contract migration

- Добавлен flattened `ContextReport` с top-level schema.
- Direct CLI, daemon и active MCP Context используют одну versioned форму.

### 2026-07-16 — DS-JSON-001 foundation и shared-literal migrations

- Созданы shared trait, registry, validation и typed errors.
- Search, Impact, Check family и ChangeMap переведены на shared constants.
- Добавлены golden fixtures и bounded inventory enforcement.
