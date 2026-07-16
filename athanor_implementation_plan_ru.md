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
| `DS-JSON-001` | P1 | `[-] in progress` | Registry содержит 19 golden-protected contracts; literal migration, Context и Project Registry collisions закрыты |
| `DS-JSON-002` | P1 | `[-] in progress` | Inventory различает public, migration и persisted schemas; transport/process/persistence pass остаётся |
| `DS-JSON-003` | P1 | `[-] in progress` | Context JSON parity реализована для direct CLI, daemon и активного MCP runtime |

## DS-RESOLVE-003 — Validated Artifact Resolver Migration

**Статус:** `[x] verified`.

- [x] Generated read-model и index-state paths разрешаются pointer-first.
- [x] Path/type/schema/snapshot/generation/checksum identity проверяется до использования.
- [x] Runtime consumers используют shared resolver boundary.
- [x] Rustok resolver coverage сохранена после runtime decomposition.

## DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`.

### Реализовано

- [x] App-layer модуль `json_contract`.
- [x] Trait `VersionedJsonContract`.
- [x] Schema-id validator: `athanor.<name>[.<name>...].v<positive-major>`.
- [x] Проверка top-level object и строкового поля `schema`.
- [x] Typed `JsonContractError`.
- [x] Registry `VERSIONED_JSON_CONTRACTS`.
- [x] Registry validation отклоняет duplicate schema id.
- [x] Registry validation отклоняет conflicting/duplicate Rust type owner.
- [x] Зарегистрированы Overview/Search.
- [x] Зарегистрированы Explain/Impact.
- [x] Зарегистрировано семейство Check: Diagnostic/Affected/Operations Docs.
- [x] Зарегистрированы Coverage/Capabilities.
- [x] Зарегистрирован ChangeMap.
- [x] Зарегистрирован `ContextReport` для `athanor.context_pack.v1`.
- [x] Зарегистрированы шесть core Graph reports: Export/Related/Path/Hubs/PageRank/Cycles.
- [x] Зарегистрирован `ProjectRegistryReport` для `athanor.project_registry.v1`.
- [x] Зарегистрирован `ProjectResolutionReport`.
- [x] Девятнадцать public report types реализуют общий contract trait.
- [x] `RepositoryOverview` и `EntityExplanation` создают schema через shared constants.
- [x] Search builder использует `SEARCH_SCHEMA_V1` вместо локального literal.
- [x] Impact normal и empty-diff builders используют `IMPACT_ANALYSIS_SCHEMA_V1`.
- [x] Diagnostic/Affected/Operations Docs Check builders используют shared constants.
- [x] ChangeMap builder использует `CHANGE_MAP_SCHEMA_V1`.
- [x] Source-level regression запрещает возврат quoted schema literals во всех шести мигрированных builders.
- [x] `ContextReport` добавляет top-level schema через `serde(flatten)` без нового `pack` nesting.
- [x] Internal generator и daemon cache продолжают использовать доменный `ContextPack`.
- [x] Context migration allowlist exception удалено после регистрации.
- [x] Persisted `ProjectRegistry` пишет `athanor.project_registry_state.v1`.
- [x] Public `ProjectRegistryReport` сохраняет прежний `athanor.project_registry.v1` для CLI compatibility.
- [x] Legacy persisted `athanor.project_registry.v1` принимается и нормализуется в current state schema.
- [x] Следующая add/remove mutation атомарно переписывает legacy registry в `project_registry_state.v1`.
- [x] Deprecated Rust alias `PROJECT_REGISTRY_SCHEMA` сохранён для source compatibility.
- [x] Локальные Coverage/Capabilities, Graph, Project Registry и Project Resolution constants проверяются на равенство shared registry constants.
- [x] Unit regressions для valid, malformed, unversioned, missing и mismatched schemas.
- [x] Unit regressions для duplicate schema/type ownership.
- [x] Golden fixtures Overview/Search.
- [x] Непустые golden fixtures Explain/Impact.
- [x] Golden fixtures Diagnostic/Affected/Operations Docs Check.
- [x] Golden fixtures Coverage/Capabilities.
- [x] Dedicated Context golden fixture защищает top-level schema и прежнюю flattened форму.
- [x] Dedicated Project Registry report fixture защищает public schema и `registry_path` shape.
- [x] Second-wave fixture защищает ChangeMap, core Graph и Project Resolution shapes.
- [x] Project Registry unit regressions защищают current writes, legacy reads, migration-on-mutation и unknown-schema rejection.
- [x] Integration tests сериализуют public report types и сравнивают их с fixtures.
- [x] Публичные v1 payload shapes не удаляли существующие поля; Context получил additive top-level schema.
- [!] Локальный Rust toolchain в рабочей среде отсутствует; fmt/test/Clippy не заявляются выполненными.
- [!] Hosted Actions/check status для `main` не виден через connector; YAML не считается подтверждением успешного run.

### Compatibility migrations

- [x] Context nested schema заменена public typed wrapper без изменения внутренних generator/cache values.
- [x] Project Registry current emitted shapes разделены: public report сохраняет `athanor.project_registry.v1`, persisted state использует `athanor.project_registry_state.v1`.
- [x] Legacy persisted state принимается на протяжении v1 compatibility window; удаление legacy reader требует нового major decision.
- [x] Известных app-layer schema collisions после этого среза не осталось.

### Следующий срез

- [x] Создать inventory зарегистрированных и обнаруженных JSON boundaries.
- [x] Перевести Search и Impact builders на shared schema constants.
- [x] Перевести Diagnostic/Affected/Operations Docs Check и ChangeMap builders на shared schema constants.
- [x] Спроектировать и реализовать versioned Context response без обхода top-level validation.
- [x] Разделить persisted `ProjectRegistry` и public `ProjectRegistryReport` schemas с compatibility tests.
- [ ] Зарегистрировать specialized Rustok graph/audit families с непустыми representative fixtures.
- [ ] Инвентаризировать daemon/MCP envelopes, process-adapter protocols и остальные persisted state documents.
- [ ] Добавить parity tests для остальных общих CLI/daemon/MCP read-only операций.
- [ ] Выполнить targeted и workspace verification.

### Compatibility policy

- [x] Schema id содержит положительный major.
- [x] Runtime instance совпадает с associated schema constant.
- [x] Overview/Search field names и nesting защищены fixtures.
- [x] Explain/Impact nested entity и enum serialization защищены непустыми fixtures.
- [x] Check scope enum и nested report composition защищены fixtures.
- [x] Coverage/Capabilities limits, totals и omission shape защищены fixtures.
- [x] ChangeMap, core Graph и Project Resolution top-level shapes защищены fixture.
- [x] Context direct CLI/daemon/active MCP mapping использует один `ContextReport`.
- [x] Project Registry legacy input нормализуется до current state schema перед записью.
- [x] Public report и internal persistence schemas классифицируются раздельно.
- [ ] Optional/defaulted additive fields могут сохранять major.
- [ ] Удаление, переименование, изменение типа или семантики требует нового major.
- [x] Старый Project Registry persisted id сохраняется как accepted input на объявленный v1 compatibility window.
- [ ] Mapping остальных CLI/daemon/MCP operations фиксируется parity regressions.

### Definition of Done

`DS-JSON-001` станет `[x] verified`, когда registry-wide inventory завершена, specialized public families классифицированы, parity tests пройдены, fixtures выполняются, а fmt, targeted tests, workspace tests и Clippy зелёные.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

- [x] Создан `docs/development/json-contract-inventory.md`.
- [x] Зафиксированы 19 зарегистрированных public contracts и их Rust owners.
- [x] Context nested-schema blocker разрешён typed top-level wrapper.
- [x] Project Registry schema collision разрешён отдельным persisted state id.
- [x] `athanor.project_registry_state.v1` классифицирован как internal persisted schema, а не public report contract.
- [x] Отдельно выделены specialized Rustok reports, process protocols и persistence documents.
- [x] Добавлен bounded enforcement test для известных app-layer owner modules.
- [x] Новые schema literals должны быть зарегистрированы, tracked как migration item или classified как persisted.
- [x] Stale allowlist entries отклоняются после регистрации или удаления соответствующего schema id.
- [x] Persisted ids не могут одновременно находиться в public registry или migration allowlist.
- [x] Для мигрированных builders тест отклоняет повторное появление quoted schema literal.
- [ ] Завершить inventory daemon envelopes и MCP responses.
- [ ] Завершить inventory extractor/linker/checker process protocols.
- [ ] Завершить inventory index/publication/read-model persistence schemas.
- [ ] Расширить enforcement на transport, process и persistence boundaries после их классификации.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`.

- [x] Direct CLI context JSON сериализует `ContextReport`.
- [x] Operation-aware diff context сериализует `ContextReport`.
- [x] Cached/non-diff daemon context сериализует `ContextReport` без изменения cache value.
- [x] Active lifecycle-based MCP context использует operation-aware `ContextReport`.
- [x] Textовый CLI сохраняет прежний вывод через transparent dereference к `ContextPack`.
- [ ] Добавить executable parity regression для Context на CLI/daemon/MCP fixtures.
- [ ] Распространить parity enforcement на остальные общие read-only operations.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test versioned_json_contracts --locked
cargo test -p athanor-app --test context_report_contract --locked
cargo test -p athanor-app --test project_registry_report_contract --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app json_contract --locked
cargo test -p athanor-app project_registry --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Активный рабочий пакет

**Сейчас:** `DS-JSON-001/002/003` — 19 contracts зарегистрированы; literal migration, Context и Project Registry collisions закрыты; verification остаётся pending.

**Дальше:** specialized Rustok contracts с representative fixtures, затем transport/process/persistence inventory и parity enforcement.

## Журнал

### 2026-07-16 — DS-JSON-001/002 Project Registry schema migration

- Public `ProjectRegistryReport` сохранил `athanor.project_registry.v1` и зарегистрирован как 19-й unique contract owner.
- Persisted `ProjectRegistry` переведён на `athanor.project_registry_state.v1`.
- Legacy `projects.json` с прежним id принимается без read-time rewrite и мигрирует при следующей add/remove mutation.
- Добавлены current-write, legacy-read, migration-on-mutation, unknown-schema и golden-report regressions.
- Inventory enforcement теперь отдельно классифицирует internal persisted schemas.
- Известный app-layer schema collision закрыт на уровне реализации; Rust verification остаётся pending.

### 2026-07-16 — DS-JSON-001/003 Context contract migration

- Добавлен `ContextReport` с top-level `athanor.context_pack.v1` и flattened `ContextPack` fields.
- Contract зарегистрирован как 18-й уникальный owner и защищён dedicated non-empty golden fixture.
- Context schema удалена из migration allowlist.
- Direct CLI, operation-aware daemon, cached daemon и active MCP context используют одну versioned форму.
- Внутренний генератор и daemon cache остались на `ContextPack`; текстовый CLI output не изменён.
- Context blocker закрыт на уровне реализации, но Rust verification и executable parity test остаются pending.

### 2026-07-16 — DS-JSON-001 Check/ChangeMap shared-literal migration

- Diagnostic, Affected и Operations Docs Check builders переведены на shared constants.
- ChangeMap builder переведён на `CHANGE_MAP_SCHEMA_V1`.
- Unit assertions больше не дублируют schema literals.
- Source-level regression расширена на все шесть мигрированных schema ids.
- Literal-migration срез завершён.

### 2026-07-16 — DS-JSON-001 Search/Impact shared-literal migration

- Search builder переведён с `athanor.search.v1` literal на `SEARCH_SCHEMA_V1`.
- Impact normal report и empty-diff early return переведены на `IMPACT_ANALYSIS_SCHEMA_V1`.
- Добавлена source-level regression для migrated builders: schema должна оставаться зарегистрированной, а quoted literal не может вернуться в source.
- Документация inventory и активный пакет синхронизированы.

### 2026-07-16 — DS-JSON-002 bounded inventory enforcement

- Добавлен integration test, сканирующий известные app-layer agent-facing owner modules.
- Каждый обнаруженный canonical schema id должен быть зарегистрирован или находиться в явном migration allowlist.
- Allowlist был ограничен Project Registry blocker и specialized Rustok graph/audit family.
- Тест отклоняет stale exceptions, если schema зарегистрирована или исчезла из inventoried sources.
- Enforcement остаётся bounded до завершения transport/process/persistence inventory.

### 2026-07-16 — DS-JSON-001 second-wave и inventory

- Registry расширен с 9 до 17 agent-facing contracts.
- Добавлены ChangeMap, шесть core Graph reports и Project Resolution.
- Добавлен combined golden fixture для second-wave payload shapes.
- Создан registry-wide inventory с границами public, process и persistence JSON.
- Выявлены Context nested-schema и Project Registry same-id collisions.

### 2026-07-16 — DS-JSON-001 coverage/capabilities slice

- Registry расширен до девяти agent-facing contracts.
- Зарегистрированы Coverage и Capabilities reports.
- Добавлены fixtures для filters, limits, totals, confidence threshold и omission counters.
- Existing module constants проверяются на равенство shared contract ids.

### 2026-07-16 — DS-JSON-001 check-family slice

- Registry расширен до семи agent-facing contracts.
- Зарегистрированы Diagnostic, Affected и Operations Docs Check reports.
- Добавлены fixtures для scope enum, affected counters и nested operations-check composition.
- Integration suite валидирует все зарегистрированные Check documents через общий trait.

### 2026-07-16 — DS-JSON-001 explain/impact slice

- Registry расширен до четырёх agent-facing contracts.
- Добавлена проверка уникальности schema id и Rust type owner.
- `EntityExplanation` переведён на shared schema constant.
- Добавлены непустые Explain/Impact golden fixtures и integration tests реальных builders.

### 2026-07-16 — DS-JSON-001 golden slice

- Добавлены golden fixtures Overview/Search.
- Добавлен integration test реальных public report types.

### 2026-07-16 — DS-JSON-001 foundation

- Создан shared versioned JSON contract boundary.
- Добавлены schema validation rules, registry и typed errors.
- `DS-RESOLVE-003` зафиксирован как `verified`.
