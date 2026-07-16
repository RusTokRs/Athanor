# Athanor: консолидированный план реализации

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Дата актуализации: 2026-07-16  
> Статус: active implementation plan

## 0. Правила исполнения

Статусы:

- `[x] verified` — код, документация и требуемые regressions подтверждены;
- `[-] in progress` — полезный срез реализован, но acceptance criteria закрыты не полностью;
- `[ ] planned` — работа не начата;
- `[!] blocked` — требуется внешнее или платформенное подтверждение.

Правила:

1. Изменения коммитятся напрямую в `main`, пока пользователь явно не изменит режим.
2. Один коммит должен содержать одну логически завершённую часть.
3. Нельзя отмечать work item как `verified` без воспроизводимой проверки.
4. JSON-контракт считается внешним API: несовместимое изменение требует нового major schema id.
5. CLI, daemon и MCP не должны определять разные формы одного и того же документа.
6. Generated artifacts остаются read models; agents получают bounded typed JSON через команды и transports.
7. Документация и отметки roadmap обновляются в том же рабочем пакете.
8. Hosted/platform evidence не заявляется, если connector не возвращает run/status/artifact.

## 1. Текущий baseline

Подтверждённая кодовая база включает:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default и opt-in SurrealDB;
- committed snapshot isolation и backend conformance contracts;
- immutable generation identity, checksums, pointer-first repair и recovery;
- bounded CLI/daemon/MCP reads со stable error contracts;
- operation context, deadline/cancellation и drained worker/process lifecycle;
- staged search/projector publication;
- injected external-process runner;
- feature, coverage, AppSec, installer и release workflows;
- validated artifact resolvers для generated read models и index state.

Основные проверки:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
```

Платформенное ограничение текущей среды:

- [!] локальный Rust toolchain отсутствует;
- [!] GitHub connector не подтверждает push-triggered hosted run/status;
- [!] поэтому новые кодовые срезы остаются `in progress` до фактической проверки.

## 2. Текущая последовательность DS work items

| ID | Приоритет | Статус | Результат |
| --- | --- | --- | --- |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated artifact resolver migration завершена |
| `DS-JSON-001` | P1 | `[-] in progress` | Versioned JSON contract foundation добавлена |
| `DS-JSON-002` | P1 | `[ ] planned` | Golden compatibility fixtures и registry-wide enforcement |
| `DS-JSON-003` | P1 | `[ ] planned` | CLI/daemon/MCP parity и compatibility policy enforcement |

## 3. DS-RESOLVE-003 — Validated Artifact Resolver Migration

**Статус:** `[x] verified`.

Завершённый результат:

- generated read-model и index-state paths разрешаются pointer-first;
- path/type/schema/snapshot/generation/checksum identity проверяется до использования;
- shared resolver helpers экспортированы из publication boundary;
- runtime consumers переведены с прямого path construction;
- Rustok resolver adapter coverage сохранена после runtime decomposition;
- regressions защищают validated resolver behavior.

Этот work item больше не является активным.

## 4. DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`.

**Цель:** сделать каждый agent-facing JSON document явным, версионированным и одинаковым для CLI, daemon и MCP.

### 4.1. Реализованный foundation slice

- [x] Добавлен app-layer модуль `json_contract`.
- [x] Добавлен trait `VersionedJsonContract`.
- [x] Добавлен validator schema id формата `athanor.<name>[.<name>...].v<positive-major>`.
- [x] Добавлена проверка top-level JSON object и строкового поля `schema`.
- [x] Добавлена deterministic `JsonContractError` taxonomy.
- [x] Добавлен registry `VERSIONED_JSON_CONTRACTS`.
- [x] Зарегистрированы `athanor.overview.v1` и `athanor.search.v1`.
- [x] Существующие `RepositoryOverview` и `SearchReport` реализуют общий contract trait.
- [x] Добавлены unit regressions для valid, malformed, unversioned, missing, duplicate и mismatched schemas.
- [x] Публичная форма существующих JSON payloads не изменена.
- [!] `cargo fmt/test/clippy` не запущены: Rust toolchain отсутствует в доступной среде.

### 4.2. Следующий срез

- [ ] Найти все внешние JSON documents в `athanor-app`, `ath`, `athd`, MCP и process-adapter protocols.
- [ ] Для каждого документа назначить одного schema owner.
- [ ] Убрать локальные строковые literals в пользу shared constants.
- [ ] Добавить документы в registry без изменения v1 payload shapes.
- [ ] Добавить golden JSON fixtures для Overview и Search.
- [ ] Добавить contract test на уникальность schema id и Rust type ownership.
- [ ] Добавить test, запрещающий registered contract без top-level `schema`.
- [ ] Запустить targeted и workspace verification.

### 4.3. Compatibility rules

- [x] Schema id содержит положительный major version.
- [x] Runtime instance обязан совпадать с associated schema constant.
- [ ] Добавление optional/defaulted поля допускается в текущем major.
- [ ] Удаление/переименование поля, изменение типа или семантики требует нового major.
- [ ] Старый major сохраняется на период объявленного compatibility window.
- [ ] Golden fixtures фиксируют field names, nesting и enum serialization.
- [ ] Equivalent CLI/daemon/MCP operation использует один Rust document type или проверяемый canonical mapping.

### 4.4. Definition of Done

`DS-JSON-001` переводится в `[x] verified`, когда:

- все выбранные first-wave outputs используют shared schema constants;
- Overview и Search имеют golden compatibility fixtures;
- registry rejects duplicate ids и conflicting owners;
- malformed/missing/mismatched schemas детерминированно отклоняются;
- payload parity проверена для затронутых CLI/daemon/MCP paths;
- `cargo fmt`, targeted tests, workspace tests и Clippy успешно выполнены;
- roadmap и этот план содержат одинаковый статус.

## 5. Остающиеся глобальные направления

### P0 / hosted evidence

- [!] feature matrix runs и required checks;
- [!] coverage artifact и измеренный floor;
- [!] AppSec/SBOM/signing/tag evidence;
- [!] installer Linux/Windows smoke;
- [!] JSONL/embedded/remote transactional evidence.

### P1

- [-] `DS-JSON-001` versioned JSON contracts;
- [ ] OS sandbox policy, Linux limits и Windows Job Objects;
- [ ] завершение CLI/runtime decomposition и удаление global registries;
- [ ] aggregate performance budgets и large-repository scenarios.

### P2

- [ ] branch protection и required checks;
- [ ] issue/acceptance traceability;
- [ ] commit/changelog/version policy;
- [ ] unified local verification entrypoint.

## 6. Текущий рабочий пакет

**Активный work item:** `DS-JSON-001 — Versioned JSON Contracts`.

**Текущий статус:** foundation implemented, verification pending.

**Последние изменения в `main`:**

- создан `crates/athanor-app/src/json_contract.rs`;
- contract API экспортирован из `athanor-app`;
- Overview/Search подключены к общему trait и registry;
- добавлены schema validation regressions;
- plan переключён с resolver/runtime work на DS JSON sequence.

**Следующий атомарный кодовый срез:** inventory + golden fixtures для `RepositoryOverview` и `SearchReport`, затем миграция следующей группы agent-facing reports.

## 7. Журнал актуализаций

### 2026-07-16 — DS-JSON-001 foundation

- `DS-RESOLVE-003` зафиксирован как `verified` и снят с активной разработки.
- Создан единый app-layer versioned JSON contract boundary.
- Добавлены canonical schema-id rules и typed validation errors.
- Первой волной зарегистрированы Overview и Search без изменения payload shape.
- Добавлены regressions для schema format и serialized-field consistency.
- `DS-JSON-001` отмечен `in progress`, поскольку toolchain/hosted verification недоступны.

### 2026-07-16 — DS-RESOLVE-003 completion

- Validated pointer-first artifact resolvers приняты как завершённый work item.
- Runtime/publication callers используют shared resolver boundary.
- Resolver regression coverage сохранена после runtime decomposition.
