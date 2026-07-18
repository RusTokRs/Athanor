# Athanor: консолидированный план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-18  
> Статус: active architecture audit

## 1. Правила статусов

- `[x] implemented` — изменение находится в `main`, но ещё не подтверждено execution evidence.
- `[x] verified` — реализация и regressions подтверждены выполненными проверками на одном commit.
- `[-] in progress` — полезные срезы находятся в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — подтверждённая работа ещё не начата.
- `[!] blocked` — безопасное изменение или проверка временно недоступны.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id или
protocol version. CLI, daemon и MCP варианты одной операции должны сохранять эквивалентные
application payloads.

## 2. Текущее состояние

Process-global runtime state, installer APIs, adapter/projector/store/search globals, Context
compatibility owners и no-composition write-service wrappers удалены.

Composition-only execution уже действует для:

- Context и operation-aware Context;
- daemon host, query, derived read и write jobs;
- Search и snapshot Search;
- Index, Generation, Wiki, HTML report и benchmark;
- Explain;
- ChangeMap;
- API Registry;
- Overview;
- Capabilities.

ChangeMap, Overview и Capabilities физически разделены на conventional bounded modules без
`include!` и forwarding compatibility facades. Их public root modules содержат только module wiring
и стабильные re-exports.

Остающийся composition debt сосредоточен в public `store::init_store` facade и read-service owners,
которые ещё импортируют его или принимают `Option<&RuntimeComposition>`.

## 3. Активный пакет — `COMP-003C2B2C2B`

**Цель:** полностью удалить dependency-hidden Store/read-service compatibility paths.

### Завершено на уровне implementation

- [x] Удалены operation-aware no-composition snapshot Search wrappers.
- [x] Удалён последний no-composition snapshot Search helper.
- [x] Explain требует mandatory `&RuntimeComposition`.
- [x] ChangeMap требует mandatory composition и использует composition-aware task Search.
- [x] ChangeMap декомпозирован на `model`, `execution`, `ranking`, `evidence`, `tests`.
- [x] API Registry использует только `query_api_registry_with_composition`.
- [x] Overview использует только `overview_project_with_composition`.
- [x] Overview декомпозирован на `model`, `execution`, `aggregation`, `tests`.
- [x] Capabilities использует только `capabilities_project_with_composition`.
- [x] Capabilities декомпозирован на `model`, `execution`, `aggregation`, `tests`.
- [x] Source inventories запрещают возврат удалённых APIs и фиксируют line budgets.

### Следующий обязательный срез

`Impact`:

- удалить `impact_project`;
- оставить только `impact_project_with_composition`;
- удалить `Option<&RuntimeComposition>`;
- удалить `crate::store::init_store` и fallback match;
- отделить execution/loading от pure `impact_snapshot` и graph traversal;
- сохранить `IMPACT_ANALYSIS_SCHEMA_V1`, BFS semantics, deterministic paths и target/diff behavior;
- добавить source/line-budget inventory.

### После Impact

Последовательно мигрировать:

1. Coverage;
2. Graph и Graph operation;
3. Check families;
4. API read owners;
5. Repair latest/recovery;
6. Docs service;
7. remaining embedding examples и tests.

После migration всех callers:

- удалить public `store::init_store` facade;
- запретить его source inventory;
- удалить stale compatibility documentation;
- выполнить targeted/default/all-features verification.

## 4. Программа работ

| ID | Priority | Status | Результат / критерий закрытия |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Composition owners декомпозированы; remaining Store/read cleanup и execution pending |
| `COMP-003` | P2 | `[-] in progress` | Все runtime dependencies explicit; public Store compatibility facade удалён |
| `COMP-003C2B2C2B` | P2 | `[-] in progress` | Все remaining read services composition-only; source enforcement и execution complete |
| `MCP-007` | P1 | `[ ] planned` | Index cancellation различает pre-commit rollback и post-commit durable success |
| `JSON-003` | P1 | `[-] in progress` | Repeat repository-wide schema scan и enforcement matrix выполнены |
| `DOC-001` | P3 | `[-] in progress` | Stale `verified` claims и removed paths удалены |
| `DOC-002` | P3 | `[ ] planned` | Pipeline current/target/history согласованы |
| `VERIFY-001` | P1 | `[!] blocked` | fmt/check/tests/Clippy/smoke выполнены на одном commit |

## 5. Реализованные архитектурные срезы

### Runtime composition

- [x] Process-global runtime owners физически удалены.
- [x] Installer APIs и legacy factory errors удалены.
- [x] Parallel isolated compositions покрывают Store/Search/Wiki/HTML factories.
- [x] Context normal/operation cores требуют mandatory composition.
- [x] Daemon state и lifecycle требуют mandatory composition.
- [x] Write services и projectors composition-only.
- [x] Search facade composition-only.
- [x] Explain, ChangeMap, API Registry, Overview и Capabilities composition-only.

### Bounded ownership

- [x] Runtime, Docs, JSONL Store, Search, MCP и CLI монолиты декомпозированы.
- [x] ChangeMap декомпозирован на conventional bounded owners.
- [x] Overview декомпозирован на conventional bounded owners.
- [x] Capabilities декомпозирован на conventional bounded owners.
- [x] Daemon command dispatch вынесен из transport lifecycle.
- [x] Publication lifecycle имеет отдельные bounded owners и commit-point semantics.

### Protocols и publication

- [x] MCP bounded queues, in-flight limits и task reaping реализованы.
- [x] JSON-RPC `2.0`, error codes, id/null/session semantics реализованы.
- [x] Publication post-commit cleanup является best effort.
- [x] Pointer publication имеет один owner.

### JSON contracts

- [x] Adapter manifest использует versioned schema id.
- [x] Persisted registry и public report имеют разные schema owners.
- [x] Canonical re-export path для adapter contracts один.
- [-] Repeat adapter-boundary inventory и execution matrix pending.

## 6. Verification matrix

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test -p athanor-app --test read_service_composition_inventory --locked
cargo test -p athanor-app --test service_composition_inventory --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
cargo test -p athanor-app --test composition_isolation --locked
cargo test -p athanor-app --test context_composition_inventory --locked
cargo test -p athanor-app --test daemon_composition_inventory --locked
cargo test -p athanor-app --test write_service_composition_inventory --locked
cargo test -p athanor-app --test runtime_modularity_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-transport-mcp --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo test -p athanor-store-jsonl --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo run -p ath --quiet --locked -- --help
cargo run -p ath --quiet --locked -- index .
```

Новые срезы сохраняют статус `implemented`, пока этот набор не выполнен на одном commit.

Текущий runtime не может получить локальный checkout из-за отсутствия DNS-доступа к GitHub.
Hosted workflow runs для новых direct-to-main commits также не обнаружены, поэтому verification
status намеренно не повышается.

## 7. Последние изменения

### 2026-07-18 — Composition-only bounded Capabilities

- Монолитный `capabilities.rs` заменён small module root.
- Public report model и constants перенесены в `capabilities/model.rs`.
- Composition-aware snapshot/state loading перенесён в `capabilities/execution.rs`.
- Completeness aggregation, ordering, limits и evidence-path logic перенесены в
  `capabilities/aggregation.rs`.
- Unit regressions перенесены в `capabilities/tests.rs`.
- Удалены no-composition `capabilities_project`, optional composition,
  `crate::store::init_store` и fallback match.
- `read_service_composition_inventory` фиксирует routing, physical fallback absence и line budgets.
- Schema `athanor.capabilities.v1`, completeness semantics, ordering и omitted counts сохранены.
- Статус — implemented, Rust/hosted verification pending.

### 2026-07-18 — Composition-only bounded Overview

- `overview.rs` заменён small module root.
- Model, execution, aggregation и tests вынесены в conventional modules.
- Удалены no-composition API, optional composition и Store fallback.
- Pure `build_repository_overview`, schema и ordering contracts сохранены.
- Статус — implemented, verification pending.

### 2026-07-18 — Composition-only API Registry

- Удалён `query_api_registry`.
- `query_api_registry_with_composition` создаёт Store только через supplied composition.
- Unit test использует isolated test composition.
- Статус — implemented, verification pending.

### 2026-07-18 — ChangeMap cleanup и decomposition

- Удалены no-composition ChangeMap/Search paths.
- ChangeMap task Search использует supplied composition.
- Temporary facade удалён.
- Owner разделён на model/execution/ranking/evidence/tests.
- Public schema и report model сохранены.
- Статус — implemented, verification pending.

## 8. Ранее закрытые этапы

- `COMP-003A/B1/B2/C1/C2A/C2B1/C2B2A/C2B2B1/C2B2B2A/C2B2B2B/C2B2C1/C2B2C2A`
  реализованы в `main`.
- `DS-RESOLVE-003` verified.
- Runtime, MCP, publication, JSONL Store, Docs и CLI modularity changes находятся в `main`.
