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
| `DS-JSON-001` | P1 | `[-] in progress` | Registry расширен до 17 golden-protected v1 contracts; inventory выявил два compatibility blockers |
| `DS-JSON-002` | P1 | `[-] in progress` | Inventory создан и bounded enforcement включён для известных app-layer agent-facing modules |
| `DS-JSON-003` | P1 | `[ ] planned` | CLI/daemon/MCP parity и compatibility enforcement |

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
- [x] Зарегистрированы шесть core Graph reports: Export/Related/Path/Hubs/PageRank/Cycles.
- [x] Зарегистрирован `ProjectResolutionReport`.
- [x] Семнадцать public report types реализуют общий contract trait.
- [x] `RepositoryOverview` и `EntityExplanation` создают schema через shared constants.
- [x] Локальные Coverage/Capabilities, Graph и Project Resolution constants проверяются на равенство shared registry constants.
- [x] Unit regressions для valid, malformed, unversioned, missing и mismatched schemas.
- [x] Unit regressions для duplicate schema/type ownership.
- [x] Golden fixtures Overview/Search.
- [x] Непустые golden fixtures Explain/Impact.
- [x] Golden fixtures Diagnostic/Affected/Operations Docs Check.
- [x] Golden fixtures Coverage/Capabilities.
- [x] Second-wave fixture защищает ChangeMap, core Graph и Project Resolution shapes.
- [x] Integration tests сериализуют public report types и сравнивают их с fixtures.
- [x] Fixtures фиксируют nested entities, enum scopes, counters, limits, omission blocks, graph nodes и project paths.
- [x] Публичные v1 payload shapes не изменены.
- [!] Локальный Rust toolchain в рабочей среде отсутствует; fmt/test/Clippy не заявляются выполненными.
- [!] Hosted Actions/check status для `main` не виден через connector; YAML не считается подтверждением успешного run.

### Найденные compatibility blockers

- [!] `ContextPack` хранит `athanor.context_pack.v1` внутри `payload`, а не в top-level schema field. Нужен typed wrapper или отдельный nested-payload contract.
- [!] `ProjectRegistry` и `ProjectRegistryReport` используют один `athanor.project_registry.v1` при разных top-level shapes. Нужны отдельные schema ids и migration window.

### Следующий срез

- [x] Создать inventory зарегистрированных и обнаруженных JSON boundaries.
- [ ] Заменить оставшиеся локальные schema literals shared constants, начиная с Search, Impact, Check и ChangeMap builders.
- [ ] Спроектировать versioned context response без обхода top-level validation.
- [ ] Разделить persisted `ProjectRegistry` и public `ProjectRegistryReport` schemas с compatibility tests.
- [ ] Зарегистрировать specialized Rustok graph/audit families с непустыми representative fixtures.
- [ ] Инвентаризировать daemon/MCP envelopes, process-adapter protocols и persisted state documents.
- [ ] Добавить CLI/daemon/MCP parity tests для общих read-only операций.
- [ ] Выполнить targeted и workspace verification.

### Compatibility policy

- [x] Schema id содержит положительный major.
- [x] Runtime instance совпадает с associated schema constant.
- [x] Overview/Search field names и nesting защищены fixtures.
- [x] Explain/Impact nested entity и enum serialization защищены непустыми fixtures.
- [x] Check scope enum и nested report composition защищены fixtures.
- [x] Coverage/Capabilities limits, totals и omission shape защищены fixtures.
- [x] ChangeMap, core Graph и Project Resolution top-level shapes защищены fixture.
- [ ] Optional/defaulted additive fields могут сохранять major.
- [ ] Удаление, переименование, изменение типа или семантики требует нового major.
- [ ] Старый major сохраняется на объявленный compatibility window.
- [ ] CLI/daemon/MCP mapping фиксируется parity regressions.

### Definition of Done

`DS-JSON-001` станет `[x] verified`, когда first-wave и second-wave outputs используют shared constants в builders, registry-wide inventory завершена, Context/Project Registry collisions разрешены, parity tests пройдены, fixtures выполняются, а fmt, targeted tests, workspace tests и Clippy зелёные.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

- [x] Создан `docs/development/json-contract-inventory.md`.
- [x] Зафиксированы 17 зарегистрированных public contracts и их Rust owners.
- [x] Зафиксированы Context nested-schema и Project Registry schema-collision blockers.
- [x] Отдельно выделены specialized Rustok reports, process protocols и persistence documents.
- [x] Добавлен bounded enforcement test для известных app-layer agent-facing owner modules.
- [x] Новые schema literals в этих модулях должны быть зарегистрированы или явно внесены в migration allowlist.
- [x] Stale allowlist entries отклоняются после регистрации или удаления соответствующего schema id.
- [ ] Завершить inventory daemon envelopes и MCP responses.
- [ ] Завершить inventory extractor/linker/checker process protocols.
- [ ] Завершить inventory index/publication/read-model persistence schemas.
- [ ] Расширить enforcement на transport, process и persistence boundaries после их классификации.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test versioned_json_contracts --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app json_contract --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Активный рабочий пакет

**Сейчас:** `DS-JSON-001/002` — 17 уникальных public contracts зарегистрированы; bounded inventory enforcement включён; verification и compatibility blockers остаются.

**Дальше:** shared-literal migration, затем typed Context contract и разделение Project Registry schemas до transport parity.

## Журнал

### 2026-07-16 — DS-JSON-002 bounded inventory enforcement

- Добавлен integration test, сканирующий известные app-layer agent-facing owner modules.
- Каждый обнаруженный canonical schema id должен быть зарегистрирован или находиться в явном migration allowlist.
- Allowlist ограничен Context/Project Registry blockers и specialized Rustok graph/audit family.
- Тест отклоняет stale exceptions, если schema зарегистрирована или исчезла из inventoried sources.
- Enforcement остаётся bounded до завершения transport/process/persistence inventory.

### 2026-07-16 — DS-JSON-001 second-wave и inventory

- Registry расширен с 9 до 17 agent-facing contracts.
- Добавлены ChangeMap, шесть core Graph reports и Project Resolution.
- Добавлен combined golden fixture для second-wave payload shapes.
- Создан registry-wide inventory с границами public, process и persistence JSON.
- Выявлено, что Context schema вложена в arbitrary payload и не удовлетворяет текущему trait contract.
- Выявлено, что `ProjectRegistry` и `ProjectRegistryReport` используют один schema id при разных формах.
- Work item остаётся `in progress` до migration решений и Rust verification.

### 2026-07-16 — DS-JSON-001 coverage/capabilities slice

- Registry расширен до девяти agent-facing contracts.
- Зарегистрированы Coverage и Capabilities reports.
- Добавлены fixtures для filters, limits, totals, confidence threshold и omission counters.
- Existing module constants проверяются на равенство shared contract ids.
- Work item остаётся `in progress` до remaining inventory, literal migration, parity и Rust verification.

### 2026-07-16 — DS-JSON-001 check-family slice

- Registry расширен до семи agent-facing contracts.
- Зарегистрированы Diagnostic, Affected и Operations Docs Check reports.
- Добавлены fixtures для scope enum, affected counters и nested operations-check composition.
- Integration suite валидирует все зарегистрированные Check documents через общий trait.
- Work item остаётся `in progress` до shared-literal migration, полной inventory, parity и Rust verification.

### 2026-07-16 — DS-JSON-001 explain/impact slice

- Registry расширен до четырёх agent-facing contracts.
- Добавлена проверка уникальности schema id и Rust type owner.
- `EntityExplanation` переведён на shared schema constant.
- Добавлены непустые Explain/Impact golden fixtures и integration tests реальных builders.
- Work item остаётся `in progress` до полной inventory, parity и Rust verification.

### 2026-07-16 — DS-JSON-001 golden slice

- Добавлены golden fixtures Overview/Search.
- Добавлен integration test реальных public report types.
- План отмечает завершённые пункты, но work item остаётся `in progress` до проверок и registry-wide migration.

### 2026-07-16 — DS-JSON-001 foundation

- Создан shared versioned JSON contract boundary.
- Добавлены schema validation rules, registry и typed errors.
- `DS-RESOLVE-003` зафиксирован как `verified`.
