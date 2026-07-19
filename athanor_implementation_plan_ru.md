# Athanor: консолидированный план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-19  
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
- Capabilities;
- Impact;
- Coverage;
- стандартных operation-aware Graph reads.

ChangeMap, Overview, Capabilities, Impact и Coverage физически разделены на conventional bounded
modules без `include!` и forwarding compatibility facades. Их public root modules содержат только
module wiring и стабильные re-exports.

`graph_operation.rs` теперь принимает mandatory `&RuntimeComposition` во всех шести execution
entrypoints. Монолитный `graph.rs` всё ещё содержит no-composition project loading и остаётся активным
physical decomposition debt.

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
- [x] Impact использует только `impact_project_with_composition`.
- [x] Impact декомпозирован на `model`, `execution`, `traversal`, `tests`.
- [x] Coverage использует только `coverage_project_with_composition`.
- [x] Coverage декомпозирован на `model`, `execution`, `aggregation`, `tests`.
- [x] Шесть стандартных Graph operation entrypoints требуют composition.
- [x] Graph operation snapshot loading не имеет `store::init_store` fallback.
- [x] Source inventory запрещает возврат удалённых Graph operation wrappers.
- [x] Source inventory использует общие assertions для routing, mandatory composition и line budgets.

### Следующий обязательный срез

Физический cleanup `graph.rs`:

- удалить no-composition project entrypoints `export_graph`, `related_graph`, `shortest_graph_path`,
  `graph_hubs`, `graph_pagerank`, `graph_cycles` и Rustok graph/audit project wrappers;
- оставить composition-aware execution и operation-aware execution;
- удалить `crate::store::init_store` и `load_latest_graph_snapshot` fallback owner;
- сохранить pure snapshot builders, GraphML conversion, Rustok audit/graph schemas и deterministic
  ordering;
- разделить 4,8k-line owner на conventional bounded model, standard graph, Rustok FFA/FBA/Page
  Builder, serialization и tests;
- не использовать forwarding facade или `include!`;
- расширить source/line-budget inventory.

Connector не предоставляет patch write для repository contents: для `graph.rs` доступна только полная
замена файла. Поэтому текущий срез закрыл operation owner, а physical decomposition остаётся активной,
не отмеченной как implemented.

### После Graph

Последовательно мигрировать:

1. Check families;
2. API read owners;
3. Repair latest/recovery;
4. Docs service;
5. remaining embedding examples и tests.

После migration всех callers:

- удалить public `store::init_store` facade;
- запретить его source inventory;
- удалить stale compatibility documentation;
- выполнить targeted/default/all-features verification.

## 4. Программа работ

| ID | Priority | Status | Результат / критерий закрытия |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Graph operation закрыт; graph.rs и remaining Store/read cleanup pending |
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
- [x] Explain, ChangeMap, API Registry, Overview, Capabilities, Impact и Coverage composition-only.
- [x] Standard Graph operation reads composition-only.
- [-] Graph project reads и Rustok graph/audit wrappers ещё имеют hidden Store path.

### Bounded ownership

- [x] Runtime, Docs, JSONL Store, Search, MCP и CLI монолиты декомпозированы.
- [x] ChangeMap декомпозирован на conventional bounded owners.
- [x] Overview декомпозирован на conventional bounded owners.
- [x] Capabilities декомпозирован на conventional bounded owners.
- [x] Impact декомпозирован на conventional bounded owners.
- [x] Coverage декомпозирован на conventional bounded owners.
- [x] Graph operation owner ограничен composition-aware execution и cooperative worker lifecycle.
- [ ] `graph.rs` декомпозирован без facade/include compatibility layer.
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
cargo test -p athanor-app graph_operation --locked
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

### 2026-07-19 — Composition-only Graph operation reads

- Удалены `export_graph_with_operation_context`, `related_graph_with_operation_context`,
  `shortest_graph_path_with_operation_context`, `graph_hubs_with_operation_context`,
  `graph_pagerank_with_operation_context` и `graph_cycles_with_operation_context`.
- Все шесть public entrypoints принимают mandatory `&RuntimeComposition` и `&OperationContext`.
- Snapshot loading использует только `composition.init_store` и
  `load_latest_snapshot_with_operation_context`.
- `Option<&RuntimeComposition>`, `crate::store::init_store` и fallback match удалены.
- Cooperative blocking-worker cancellation/deadline tests сохранены.
- Direct Graph CLI уже использовал composition-aware variants, поэтому caller migration не
  потребовалась.
- `read_service_composition_inventory` проверяет physical absence wrappers и line budget.
- `graph.rs` остаётся active physical debt и не отмечен implemented.
- Статус среза — implemented, Rust/hosted verification pending.

### 2026-07-18 — Composition-only bounded Coverage

- `coverage.rs` заменён small module root.
- Model, composition execution, pure aggregation и tests вынесены в conventional modules.
- Удалены no-composition API, optional composition и Store fallback.
- Schema, filters, totals, ordering и omission semantics сохранены.
- Статус — implemented, verification pending.

### 2026-07-18 — Composition-only bounded Impact

- `impact.rs` заменён small module root.
- Public model, composition execution, pure traversal и tests вынесены в conventional modules.
- Удалены no-composition API, optional composition и Store fallback.
- Schema, target/diff behavior, BFS propagation и deterministic paths сохранены.
- Статус — implemented, verification pending.

### 2026-07-18 — Composition-only bounded Capabilities

- `capabilities.rs` заменён small module root.
- Model, composition execution, completeness aggregation и tests вынесены в conventional modules.
- Удалены no-composition API, optional composition и Store fallback.
- Schema, ordering, limits и omitted counts сохранены.
- Статус — implemented, verification pending.

### 2026-07-18 — Composition-only bounded Overview

- `overview.rs` заменён small module root.
- Model, execution, aggregation и tests вынесены в conventional modules.
- Удалены no-composition API, optional composition и Store fallback.
- Pure builder, schema и ordering contracts сохранены.
- Статус — implemented, verification pending.

### 2026-07-18 — Composition-only API Registry

- Удалён `query_api_registry`.
- `query_api_registry_with_composition` создаёт Store только через supplied composition.
- Unit test использует isolated test composition.
- Статус — implemented, verification pending.

### 2026-07-18 — ChangeMap cleanup и decomposition

- Удалены no-composition ChangeMap/Search paths.
- ChangeMap task Search использует supplied composition.
- Owner разделён на model/execution/ranking/evidence/tests.
- Public schema и report model сохранены.
- Статус — implemented, verification pending.

## 8. Ранее закрытые этапы

- `COMP-003A/B1/B2/C1/C2A/C2B1/C2B2A/C2B2B1/C2B2B2A/C2B2B2B/C2B2C1/C2B2C2A`
  реализованы в `main`.
- `DS-RESOLVE-003` verified.
- Runtime, MCP, publication, JSONL Store, Docs и CLI modularity changes находятся в `main`.
