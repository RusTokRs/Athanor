# Athanor: консолидированный план реализации

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-16  
> Статус: active implementation plan

## Правила статусов

- `[x] verified` — реализация и regressions подтверждены.
- `[-] in progress` — полезный срез в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — работа не начата.
- `[!] blocked` — требуется недоступная платформенная проверка.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id. Эквивалентные CLI, daemon и MCP операции не должны иметь разные формы документа.

## Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |
| `DS-JSON-001` | P1 | `[-] in progress` | Versioned JSON registry, ownership checks и четыре golden-protected v1 contracts в `main` |
| `DS-JSON-002` | P1 | `[ ] planned` | Registry-wide inventory и enforcement |
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
- [x] Зарегистрированы `athanor.overview.v1` и `athanor.search.v1`.
- [x] Зарегистрированы `athanor.entity_explanation.v1` и `athanor.impact_analysis.v1`.
- [x] `RepositoryOverview`, `SearchReport`, `EntityExplanation` и `ImpactAnalysis` реализуют общий trait.
- [x] `RepositoryOverview` и `EntityExplanation` создают schema через shared constants.
- [x] Unit regressions для valid, malformed, unversioned, missing и mismatched schemas.
- [x] Unit regressions для duplicate schema/type ownership.
- [x] Golden fixture `overview.v1.json`.
- [x] Golden fixture `search.v1.json`.
- [x] Непустой golden fixture `entity_explanation.v1.json`.
- [x] Непустой golden fixture `impact_analysis.v1.json`.
- [x] Integration tests строят реальные public reports и сравнивают их с fixtures.
- [x] Публичные v1 payload shapes не изменены.
- [!] Локальный Rust toolchain в рабочей среде отсутствует; fmt/test/Clippy не заявляются выполненными.

### Следующий срез

- [ ] Инвентаризировать внешние JSON documents в app, CLI, daemon, MCP и process-adapter protocols.
- [ ] Следующей группой зарегистрировать `DiagnosticCheckReport`, `AffectedCheckReport` и `OperationsDocsCheckReport`.
- [ ] Затем зарегистрировать Coverage/Capabilities reports.
- [ ] Назначить одному Rust type единственного schema owner.
- [ ] Заменить оставшиеся локальные schema literals shared constants.
- [ ] Добавить CLI/daemon/MCP parity tests для общих read-only операций.
- [ ] Выполнить targeted и workspace verification.

### Compatibility policy

- [x] Schema id содержит положительный major.
- [x] Runtime instance совпадает с associated schema constant.
- [x] Overview/Search field names и nesting защищены golden fixtures.
- [x] Explain/Impact nested entity и enum serialization защищены непустыми fixtures.
- [ ] Optional/defaulted additive fields могут сохранять major.
- [ ] Удаление, переименование, изменение типа или семантики требует нового major.
- [ ] Старый major сохраняется на объявленный compatibility window.
- [ ] CLI/daemon/MCP mapping фиксируется parity regressions.

### Definition of Done

`DS-JSON-001` станет `[x] verified`, когда first-wave outputs используют shared constants, registry-wide inventory завершена, parity tests пройдены, fixtures выполняются, а fmt, targeted tests, workspace tests и Clippy зелёные.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test versioned_json_contracts --locked
cargo test -p athanor-app json_contract --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Активный рабочий пакет

**Сейчас:** `DS-JSON-001` — Overview, Search, Explain и Impact зарегистрированы и защищены fixtures; verification pending.

**Дальше:** Check-report family, затем Coverage/Capabilities и parity tests.

## Журнал

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
