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

Composition-only execution действует для:

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
- стандартных operation-aware Graph reads;
- Repair latest и publication recovery;
- Docs check, drift, proposal и patch application.

ChangeMap, Overview, Capabilities, Impact и Coverage физически разделены на conventional bounded
modules без `include!` и forwarding compatibility facades. Их public roots содержат только module
wiring и стабильные re-exports.

`graph_operation.rs` принимает mandatory `&RuntimeComposition` во всех шести execution entrypoints.
Монолитный `graph.rs` всё ещё содержит no-composition project loading и остаётся обязательным
physical decomposition debt.

Repair latest/recovery имеют один execution owner в `repair_composition/direct.rs`. Старые execution
paths удалены из model owners. Legacy `docs/service.rs` физически удалён; Docs execution идёт через
`application_report_composition/docs_direct`.

Остающийся production composition debt:

1. `graph.rs`;
2. `check.rs`;
3. `api.rs` snapshot owner;
4. public `store::init_store` facade после migration всех callers.

## 3. Активный пакет — `COMP-003C2B2C2B`

**Цель:** полностью удалить dependency-hidden Store/read-service compatibility paths.

### Завершено на уровне implementation

- [x] Удалены operation-aware no-composition snapshot Search wrappers.
- [x] Удалён последний no-composition snapshot Search helper.
- [x] Explain требует mandatory `&RuntimeComposition`.
- [x] ChangeMap требует mandatory composition и composition-aware task Search.
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
- [x] Graph operation snapshot loading не имеет Store fallback.
- [x] Repair latest/recovery duplicate execution owners удалены.
- [x] Repair regressions перенесены в единственный composition owner.
- [x] Legacy Docs service owner физически удалён.
- [x] Docs execution маршрутизируется только через composition facade/direct modules.
- [x] Source inventory проверяет routing, mandatory composition и line budgets.
- [x] Runtime composition migration guide согласован с текущим деревом.

### Активные следующие срезы

#### 3.1 Check family

- [ ] удалить `check_project`, `check_affected`, `check_operations_docs`;
- [ ] оставить composition-aware execution;
- [ ] удалить `Option<&RuntimeComposition>`, `crate::store::init_store` и fallback branches;
- [ ] разделить 1.3k-line owner на model, execution, diagnostic aggregation, affected-file/artifact
  inspection и bounded tests;
- [ ] сохранить diagnostic scopes, sorting, schemas и affected-file semantics;
- [ ] добавить source/line-budget inventory.

#### 3.2 API contract snapshot

- [ ] удалить no-composition `snapshot_api_contract`;
- [ ] оставить `snapshot_api_contract_with_composition` единственным Store-owning entrypoint;
- [ ] сохранить pure diff, cleanup, retention и immutable artifact semantics;
- [ ] разделить крупный `api.rs` на bounded model, snapshot, diff, retention и tests;
- [ ] добавить source/line-budget inventory.

#### 3.3 Physical cleanup `graph.rs`

- [ ] удалить no-composition standard Graph project entrypoints;
- [ ] удалить no-composition Rustok graph/audit project wrappers;
- [ ] удалить `crate::store::init_store` и `load_latest_graph_snapshot` fallback owner;
- [ ] сохранить pure snapshot builders, GraphML conversion, schemas и deterministic ordering;
- [ ] разделить 4.8k-line owner на conventional model, standard graph, Rustok FFA/FBA/Page Builder,
  serialization и tests;
- [ ] не использовать forwarding facade или `include!`;
- [ ] расширить source/line-budget inventory.

Connector не предоставляет patch write для repository contents: для `graph.rs` доступна только полная
замена файла. Поэтому безопасно завершён operation owner, а physical decomposition остаётся активной и
не отмечается implemented до реального split.

#### 3.4 Store facade removal

После migration `graph.rs`, `check.rs` и `api.rs`:

- [ ] удалить public `store::init_store`;
- [ ] удалить его re-exports;
- [ ] запретить symbol source inventory;
- [ ] обновить embedding examples и remaining tests;
- [ ] удалить последние compatibility claims.

## 4. Программа работ

| ID | Priority | Status | Результат / критерий закрытия |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Repair/Docs/Graph operation закрыты; graph/check/api/Store pending |
| `COMP-003` | P2 | `[-] in progress` | Все runtime dependencies explicit; public Store facade удалён |
| `COMP-003C2B2C2B` | P2 | `[-] in progress` | Remaining read services composition-only; source enforcement и execution complete |
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
- [x] Repair latest/recovery composition-only.
- [x] Docs execution composition-only.
- [-] Graph project reads, Check и API snapshot ещё имеют hidden Store path.

### Bounded ownership

- [x] Runtime, Docs engine, JSONL Store, Search, MCP и CLI монолиты декомпозированы.
- [x] ChangeMap декомпозирован на conventional bounded owners.
- [x] Overview декомпозирован на conventional bounded owners.
- [x] Capabilities декомпозирован на conventional bounded owners.
- [x] Impact декомпозирован на conventional bounded owners.
- [x] Coverage декомпозирован на conventional bounded owners.
- [x] Graph operation owner ограничен composition execution и cooperative worker lifecycle.
- [x] Repair execution консолидирован в одном bounded owner.
- [x] Legacy Docs execution owner удалён; existing bounded direct modules являются canonical owners.
- [ ] `graph.rs` декомпозирован без facade/include compatibility layer.
- [ ] `check.rs` декомпозирован.
- [ ] `api.rs` декомпозирован.

### Protocols и publication

- [x] MCP bounded queues, in-flight limits и task reaping реализованы.
- [x] JSON-RPC `2.0`, error codes, id/null/session semantics реализованы.
- [x] Publication post-commit cleanup является best effort.
- [x] Pointer publication имеет один owner.
- [x] Repair latest не допускает rewind на неавторитетную generation.
- [x] Repair recovery сохраняет committed-pointer recovery semantics.

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
cargo test -p athanor-app repair_composition --locked
cargo test -p athanor-app docs --locked
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

Текущий runtime не может получить локальный checkout из-за отсутствия DNS-доступа к GitHub. Hosted
workflow runs для новых direct-to-main commits также не обнаружены, поэтому verification status
намеренно не повышается.

## 7. Последние изменения

### 2026-07-19 — Composition-only Docs execution

- Физически удалён `crates/athanor-app/src/docs/service.rs`.
- `docs.rs` больше не подключает и не re-export-ит legacy execution owner.
- Check, drift, proposal и patch application доступны через composition facade.
- Общий snapshot loader использует только `composition.init_store`.
- Existing bounded check/apply/propose/snapshot modules стали canonical execution owners.
- Source inventory проверяет routing, mandatory composition и line budgets.
- Runtime migration guide согласован с текущим состоянием.
- Статус — implemented, Rust/hosted verification pending.

### 2026-07-19 — Composition-only Repair family

- Удалён duplicate no-composition `repair_canonical_latest` owner.
- Удалён duplicate no-composition `recover_index_publication` owner.
- Latest/recovery files оставлены владельцами public option/report models и current recovery contracts.
- Execution консолидирован в `repair_composition/direct.rs`.
- Latest authoritative-generation и recovery regressions перенесены к единственному execution owner.
- Source inventory запрещает возврат legacy APIs и Store fallbacks.
- Статус — implemented, Rust/hosted verification pending.

### 2026-07-19 — Composition-only Graph operation reads

- Удалены шесть standard no-composition operation wrappers.
- Все public operation entrypoints принимают `&RuntimeComposition` и `&OperationContext`.
- Snapshot loading использует только composition и operation-aware Store API.
- Cooperative blocking-worker cancellation/deadline tests сохранены.
- Direct Graph CLI уже использовал retained APIs.
- `graph.rs` остаётся active physical debt.
- Статус — implemented, Rust/hosted verification pending.

### 2026-07-18 — Composition-only bounded read owners

- ChangeMap, Overview, Capabilities, Impact и Coverage переведены на mandatory composition.
- Owners разделены на conventional model/execution/aggregation-or-traversal/test modules.
- Search compatibility wrappers и Explain fallback удалены.
- API Registry переведён на composition-only execution.
- Public schemas, ordering, limits и report shapes сохранены.
- Статус — implemented, verification pending.

## 8. Ранее закрытые этапы

- `COMP-003A/B1/B2/C1/C2A/C2B1/C2B2A/C2B2B1/C2B2B2A/C2B2B2B/C2B2C1/C2B2C2A`
  реализованы в `main`.
- `DS-RESOLVE-003` verified.
- Runtime, MCP, publication, JSONL Store, Docs engine и CLI modularity changes находятся в `main`.
