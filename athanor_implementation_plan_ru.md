# Athanor: консолидированный план реализации

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-16  
> Статус: active implementation plan

## Правила статусов

- `[x] verified` — реализация и regressions подтверждены.
- `[-] in progress` — полезный срез в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — работа не начата.
- `[!] blocked` — требуется недоступная платформенная проверка или compatibility decision.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id. Эквивалентные CLI, daemon и MCP операции не должны иметь разные формы документа.

## Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |
| `DS-JSON-001` | P1 | `[-] in progress` | Registry содержит 32 golden-protected public contract; bounded migration allowlist пуст |
| `DS-JSON-002` | P1 | `[-] in progress` | Public read/project/Rustok inventory завершена; index/transport/process/persistence pass остаётся |
| `DS-JSON-003` | P1 | `[-] in progress` | Context JSON parity реализована; executable parity остальных операций остаётся |

## DS-RESOLVE-003 — Validated Artifact Resolver Migration

**Статус:** `[x] verified`.

- [x] Generated read-model и index-state paths разрешаются pointer-first.
- [x] Path/type/schema/snapshot/generation/checksum identity проверяется до использования.
- [x] Runtime consumers используют shared resolver boundary.
- [x] Rustok resolver coverage сохранена после runtime decomposition.

## DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`.

### Contract foundation

- [x] App-layer модуль `json_contract`.
- [x] Trait `VersionedJsonContract`.
- [x] Schema-id validator: `athanor.<name>[.<name>...].v<positive-major>`.
- [x] Проверка top-level object и строкового поля `schema`.
- [x] Typed `JsonContractError`.
- [x] Registry `VERSIONED_JSON_CONTRACTS`.
- [x] Registry validation отклоняет duplicate schema id.
- [x] Registry validation отклоняет duplicate Rust type owner.
- [x] Transparent wrapper contracts используют отдельный macro с явным `AsRef`.

### Зарегистрированные public families

- [x] Overview/Search.
- [x] Explain/Impact.
- [x] Diagnostic/Affected/Operations Docs Check.
- [x] Coverage/Capabilities.
- [x] ChangeMap.
- [x] ContextReport.
- [x] Core Graph: Export/Related/Path/Hubs/PageRank/Cycles.
- [x] ProjectRegistryReport и ProjectResolutionReport.
- [x] RustokArchitectureContext.
- [x] RusTok FFA: Audit/Surface Graph/Violations Graph.
- [x] RusTok FBA: Audit/Module Graph/Port Graph/Dependencies Graph/Violations Graph.
- [x] RusTok Page Builder: Audit/Provider Graph/Consumer Graph/Violations Graph.
- [x] Всего 32 public report type реализуют общий contract trait.

### Shared constants и builders

- [x] Overview и Explain используют shared constants.
- [x] Search использует `SEARCH_SCHEMA_V1`.
- [x] Impact normal и empty-diff paths используют `IMPACT_ANALYSIS_SCHEMA_V1`.
- [x] Diagnostic/Affected/Operations Docs Check используют shared constants.
- [x] ChangeMap использует `CHANGE_MAP_SCHEMA_V1`.
- [x] Source-level regression запрещает возврат quoted literals в шести мигрированных builders.
- [x] Coverage/Capabilities, core Graph, Rustok Architecture, specialized RusTok, Project Registry и Project Resolution constants проверяются на равенство shared registry constants.

### Compatibility migrations

- [x] `ContextReport` добавляет top-level `athanor.context_pack.v1` через `serde(flatten)` без нового nesting.
- [x] Internal Context generator и daemon cache продолжают использовать `ContextPack`.
- [x] Persisted `ProjectRegistry` пишет `athanor.project_registry_state.v1`.
- [x] Public `ProjectRegistryReport` сохраняет `athanor.project_registry.v1`.
- [x] Legacy persisted `athanor.project_registry.v1` принимается на протяжении v1 compatibility window.
- [x] Следующая add/remove mutation атомарно мигрирует legacy registry в current state schema.
- [x] Deprecated Rust alias `PROJECT_REGISTRY_SCHEMA` сохранён для source compatibility.
- [x] FFA shared graph type разделён на unique transparent public owners.
- [x] FBA shared graph type разделён на четыре unique transparent public owners.
- [x] Page Builder shared graph type разделён на три unique transparent public owners.
- [x] Operation-aware Rustok APIs оборачивают результат после cooperative worker; cancellation/deadline path не изменён.
- [x] Text renderers сохраняют прежний API через `Deref`; serialized JSON не получает нового nesting.
- [x] Известных collisions и migration exceptions в текущем public read/project/Rustok scope не осталось.

### Golden и integration regressions

- [x] Overview/Search fixtures.
- [x] Explain/Impact non-empty fixtures.
- [x] Check-family fixtures.
- [x] Coverage/Capabilities fixtures.
- [x] Context dedicated fixture.
- [x] Project Registry report dedicated fixture.
- [x] ChangeMap/core Graph/Project Resolution second-wave fixture.
- [x] Rustok Architecture fixture с resolution/modules/contracts/interactions/tests/diagnostics/evidence.
- [x] FFA representative fixture с непустыми audit surface, nodes и edges.
- [x] FBA representative fixture с audit module и четырьмя graph variants.
- [x] Page Builder representative fixture с audit и тремя graph variants.
- [x] Project Registry unit regressions: current writes, legacy reads, migration-on-mutation, unknown-schema rejection.
- [x] Registry unit regressions: malformed ids, missing/mismatched schema, duplicate schema/type ownership.
- [!] Локальный Rust toolchain в рабочей среде отсутствует; fmt/test/Clippy не заявляются выполненными.
- [!] Hosted Actions/check status для `main` не виден через connector; отсутствие status entries не считается успешным run.

### Следующий срез

- [x] Закрыть bounded public read/project/Rustok contract inventory.
- [x] Закрыть Context и Project Registry schema collisions.
- [x] Зарегистрировать Rustok Architecture и FFA/FBA/Page Builder families.
- [ ] Инвентаризировать IndexReport/BenchmarkReport/validation-result и другие application outputs.
- [ ] Инвентаризировать daemon request/response/error envelopes.
- [ ] Инвентаризировать MCP JSON-RPC и tool-content envelopes.
- [ ] Инвентаризировать extractor/linker/checker process protocols.
- [ ] Инвентаризировать index/publication/generation/read-model persisted documents.
- [ ] Добавить executable CLI/daemon/MCP parity regressions.
- [ ] Выполнить targeted и workspace verification.

### Compatibility policy

- [x] Schema id содержит положительный major.
- [x] Runtime instance совпадает с associated schema constant.
- [x] Shared calculation type с несколькими public schema ids получает отдельный owner type для каждого id.
- [x] Public report и internal persistence schemas классифицируются раздельно.
- [x] Legacy input нормализуется до current schema перед записью.
- [x] Старый Project Registry persisted id сохраняется на объявленный v1 compatibility window.
- [ ] Формализовать правило optional/defaulted additive fields.
- [ ] Формализовать major bump для удаления, переименования, изменения типа или семантики.
- [ ] Зафиксировать mapping остальных CLI/daemon/MCP operations parity regressions.

### Definition of Done

`DS-JSON-001` станет `[x] verified`, когда remaining application/transport/process/persistence inventory завершена, parity regressions пройдены, fixtures выполняются, а fmt, targeted tests, workspace tests и Clippy зелёные.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

### Завершено

- [x] Создан `docs/development/json-contract-inventory.md`.
- [x] Зафиксированы 32 зарегистрированных public read/project/Rustok contracts и их Rust owners.
- [x] Scanner охватывает идентифицированные public read, project и Rustok owner modules.
- [x] Migration allowlist для этого bounded scope пуст.
- [x] Новые schema literals в scanned modules должны быть зарегистрированы или явно classified.
- [x] `athanor.project_registry_state.v1` классифицирован как internal persisted schema.
- [x] Persisted ids не могут находиться в public registry или migration allowlist.
- [x] Stale migration и persisted classifications отклоняются.
- [x] Shared internal graph types защищены отдельными transparent owners.

### Осталось

- [ ] Additional application outputs: index/benchmark/validation documents.
- [ ] Daemon envelopes inventory и enforcement.
- [ ] MCP envelopes inventory и enforcement.
- [ ] Extractor/linker/checker process protocols inventory.
- [ ] Index state/publication journal/generation pointer/read-model manifest inventory.
- [ ] Расширение source scanner и fixtures после классификации этих boundaries.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`.

- [x] Direct CLI Context JSON сериализует `ContextReport`.
- [x] Operation-aware diff Context сериализует `ContextReport`.
- [x] Cached/non-diff daemon Context сериализует `ContextReport` без изменения cache value.
- [x] Active lifecycle-based MCP Context использует operation-aware `ContextReport`.
- [x] Textовый CLI сохраняет прежний вывод через transparent dereference.
- [x] Operation-aware Rustok Architecture и graph outputs используют зарегистрированные contracts.
- [ ] Добавить executable Context parity regression для CLI/daemon/MCP.
- [ ] Распространить parity enforcement на Overview/Search/Explain/Impact/Check/ChangeMap.
- [ ] Зафиксировать transport envelopes отдельно от application payload contracts.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test versioned_json_contracts --locked
cargo test -p athanor-app --test context_report_contract --locked
cargo test -p athanor-app --test project_registry_report_contract --locked
cargo test -p athanor-app --test rustok_architecture_context_contract --locked
cargo test -p athanor-app --test rustok_ffa_contracts --locked
cargo test -p athanor-app --test rustok_fba_contracts --locked
cargo test -p athanor-app --test rustok_page_builder_contracts --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app json_contract --locked
cargo test -p athanor-app project_registry --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Активный рабочий пакет

**Сейчас:** `DS-JSON-001/002/003` — 32 public read/project/Rustok contracts зарегистрированы; bounded migration allowlist пуст; verification остаётся pending.

**Дальше:** additional application outputs, daemon/MCP envelopes, process-adapter protocols и persisted/generated document inventory, затем executable parity enforcement.

## Журнал

### 2026-07-16 — DS-JSON-001/002 Rustok Architecture и specialized families

- Зарегистрирован `athanor.rustok_architecture_context.v1` с dedicated non-empty fixture.
- Зарегистрированы все FFA, FBA и Page Builder audit/graph schema ids.
- Общие graph calculation types разделены transparent command-specific owners без изменения JSON shape.
- Operation-aware APIs возвращают wrappers после cooperative worker.
- Specialized schema ids удалены из migration allowlist; bounded allowlist стал пустым.
- Registry вырос с 19 до 32 public contracts.
- `json_contract.rs` использует обычное formatting и explicit transparent-owner macro.

### 2026-07-16 — DS-JSON-001/002 Project Registry schema migration

- Public `ProjectRegistryReport` сохранил `athanor.project_registry.v1`.
- Persisted `ProjectRegistry` переведён на `athanor.project_registry_state.v1`.
- Legacy persisted input принимается и мигрирует при следующей mutation.
- Добавлены golden и compatibility regressions.

### 2026-07-16 — DS-JSON-001/003 Context contract migration

- Добавлен flattened `ContextReport` с top-level schema.
- Direct CLI, daemon и active MCP Context используют одну versioned форму.
- Internal generator/cache остались на `ContextPack`.

### 2026-07-16 — DS-JSON-001 shared-literal migration

- Search, Impact, Check family и ChangeMap переведены на shared constants.
- Source-level regression запрещает возврат schema literals.

### 2026-07-16 — DS-JSON-001 foundation и second wave

- Созданы shared contract trait, registry, validation и typed errors.
- Зарегистрированы first-wave и core second-wave public reports.
- Добавлены golden fixtures и bounded inventory enforcement.
