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

`COMP-003` и последний подпакет `COMP-003C2B2C2B` завершены на уровне implementation.
Process-global runtime state, installer APIs, adapter/projector/store/search globals,
dependency-hidden Store initialization и no-composition service wrappers удалены.

Composition-only execution действует для:

- Context и operation-aware Context;
- daemon host, query, derived read и write jobs;
- Search и snapshot Search;
- Index, Generation, Wiki, HTML report и benchmark;
- Explain и ChangeMap;
- API Registry и API contract snapshot;
- Overview, Capabilities, Impact и Coverage;
- Check, affected Check и operations-docs Check;
- standard и RusTok operation-aware Graph reads;
- Repair latest и publication recovery;
- Docs check, drift, proposal и patch application.

Conventional bounded owners без `include!` и forwarding compatibility facades реализованы для:

- ChangeMap;
- Overview;
- Capabilities;
- Impact;
- Coverage;
- Check;
- API contracts;
- Graph.

Former monolith `crates/athanor-app/src/graph.rs` физически удалён. Graph contracts находятся в
`graph/model.rs`, standard pure operations — в `graph/standard.rs`, RusTok adapters — в
`graph/rustok.rs`, regressions — в `graph/tests.rs`. Traversal-heavy logic имеет один canonical owner
в cooperative modules.

Public `store::init_store` удалён. `store_facade.rs` экспортирует только `AthanorStore` и
`StoreFactory`; создание Store принадлежит `RuntimeComposition::init_store`.

## 3. Завершённый пакет `COMP-003C2B2C2B`

- [x] Удалены operation-aware no-composition snapshot Search wrappers.
- [x] Удалён последний no-composition snapshot Search helper.
- [x] Explain требует mandatory `&RuntimeComposition`.
- [x] ChangeMap требует mandatory composition и composition-aware task Search.
- [x] ChangeMap декомпозирован на `model`, `execution`, `ranking`, `evidence`, `tests`.
- [x] API Registry использует только composition-aware execution.
- [x] API contract snapshot использует только `snapshot_api_contract_with_composition`.
- [x] API contracts декомпозированы на `model`, `snapshot`, `diff`, `retention`, `tests`.
- [x] API duplicate Store/publication path удалён; immutable publication semantics сохранены.
- [x] Overview, Capabilities, Impact и Coverage переведены на mandatory composition и bounded owners.
- [x] Check project, affected и operations-docs entrypoints требуют composition.
- [x] Check декомпозирован на `model`, `execution`, `diagnostics`, `affected`, `tests`.
- [x] Check Store fallback, optional composition и legacy entrypoints удалены.
- [x] Шесть standard Graph operation entrypoints требуют composition.
- [x] RusTok audit/graph project execution требует composition и operation context.
- [x] Все no-composition standard Graph project entrypoints удалены.
- [x] Все no-composition RusTok graph/audit project wrappers удалены.
- [x] `load_latest_graph_snapshot` и final Graph Store fallback удалены.
- [x] Graph schemas, report shapes, GraphML и deterministic ordering сохранены.
- [x] 4.8k-line Graph monолит физически заменён bounded model/standard/RusTok/test owners.
- [x] Pure traversal adapters используют единственные cooperative algorithm owners.
- [x] Legacy `graph.rs` физически удалён без facade/include compatibility layer.
- [x] Repair latest/recovery duplicate execution owners удалены.
- [x] Repair regressions перенесены в единственный composition owner.
- [x] Legacy Docs service owner физически удалён.
- [x] Docs execution маршрутизируется только через composition owners.
- [x] Public `store::init_store` удалён.
- [x] Store facade оставлен только владельцем backend-neutral types.
- [x] Source inventories запрещают возврат compatibility APIs и фиксируют line budgets.
- [x] Runtime composition migration guide согласован с текущим деревом.

## 4. Следующие активные пакеты

### 4.1 `MCP-007` — transactional Index cancellation

- [ ] определить durable commit point для Index operation;
- [ ] различать cancellation до commit и после durable commit;
- [ ] до commit выполнять rollback/cleanup prepared state;
- [ ] после commit возвращать durable success, не маскировать его cancellation error;
- [ ] добавить pre-commit, commit-race и post-commit regressions;
- [ ] проверить CLI/daemon/MCP payload parity.

### 4.2 `JSON-003` — repeat contract inventory

- [ ] повторить repository-wide scan schema emitters и parsers;
- [ ] проверить adapter/plugin boundaries и legacy input-only schema ids;
- [ ] подтвердить CLI/daemon/MCP payload parity;
- [ ] выполнить source inventory и JSON regression matrix;
- [ ] удалить stale claims о complete inventory без execution evidence.

### 4.3 `DOC-001` / `DOC-002` — documentation status hygiene

- [ ] убрать stale `verified` claims, не подтверждённые одним current commit;
- [ ] заменить ссылки на удалённые monolith paths актуальными bounded owners;
- [ ] согласовать pipeline current/target/history;
- [ ] синхронизировать roadmap, architecture guides и implementation plan.

### 4.4 `VERIFY-001` — execution matrix

- [!] Локальный checkout недоступен из текущего runtime из-за DNS-доступа к GitHub.
- [!] Hosted workflow runs для новых direct-to-main commits пока отсутствуют.
- [ ] выполнить fmt/check/test/Clippy/smoke matrix на одном commit;
- [ ] повысить только подтверждённые `[x] implemented` пункты до `[x] verified`.

## 5. Программа работ

| ID | Priority | Status | Результат / критерий закрытия |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Composition migration implemented; MCP/JSON/docs/verification pending |
| `COMP-003` | P2 | `[x] implemented` | Runtime dependencies explicit; Store initialization only through composition |
| `COMP-003C2B2C2B` | P2 | `[x] implemented` | Read services, Graph и Store facade compatibility cleanup complete |
| `MCP-007` | P1 | `[ ] planned` | Index cancellation различает pre-commit rollback и post-commit durable success |
| `JSON-003` | P1 | `[-] in progress` | Repeat repository-wide schema scan и enforcement matrix выполнены |
| `DOC-001` | P3 | `[-] in progress` | Stale verification claims и removed paths удалены |
| `DOC-002` | P3 | `[ ] planned` | Pipeline current/target/history согласованы |
| `VERIFY-001` | P1 | `[!] blocked` | fmt/check/tests/Clippy/smoke выполнены на одном commit |

## 6. Реализованные архитектурные срезы

### Runtime composition

- [x] Process-global runtime owners физически удалены.
- [x] Installer APIs и legacy factory errors удалены.
- [x] Parallel isolated compositions покрывают Store/Search/Wiki/HTML factories.
- [x] Context normal/operation cores требуют mandatory composition.
- [x] Daemon state и lifecycle требуют mandatory composition.
- [x] Write services и projectors composition-only.
- [x] Search facade composition-only.
- [x] Read-service families composition-only.
- [x] Standard и RusTok Graph project execution composition-only.
- [x] Public Store initializer physically removed.

### Bounded ownership

- [x] Runtime, Docs engine, JSONL Store, Search, MCP и CLI монолиты декомпозированы.
- [x] ChangeMap, Overview, Capabilities, Impact и Coverage декомпозированы.
- [x] Check и API contracts декомпозированы.
- [x] Graph декомпозирован на conventional bounded owners.
- [x] Graph cooperative algorithms имеют single owners.
- [x] Repair execution консолидирован в одном owner.
- [x] Legacy Docs execution owner удалён.

### Protocols и publication

- [x] MCP bounded queues, in-flight limits и task reaping реализованы.
- [x] JSON-RPC `2.0`, error codes, id/null/session semantics реализованы.
- [x] Publication post-commit cleanup является best effort.
- [x] Pointer publication имеет один owner.
- [x] API snapshots остаются immutable, latest pointer обновляется отдельным commit point.
- [x] Repair latest не допускает rewind на неавторитетную generation.
- [x] Repair recovery сохраняет committed-pointer recovery semantics.

### JSON contracts

- [x] Adapter manifest использует versioned schema id.
- [x] Persisted registry и public report имеют разные schema owners.
- [x] Canonical re-export path для adapter contracts один.
- [-] Repeat adapter-boundary inventory и execution matrix pending.

## 7. Verification matrix

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
cargo test -p athanor-app --test read_service_composition_inventory --locked
cargo test -p athanor-app --test check_composition_inventory --locked
cargo test -p athanor-app --test api_composition_inventory --locked
cargo test -p athanor-app --test graph_composition_inventory --locked
cargo test -p athanor-app graph --locked
cargo test -p athanor-app graph_operation --locked
cargo test -p athanor-app rustok --locked
cargo test -p athanor-app repair_composition --locked
cargo test -p athanor-app docs --locked
cargo test -p athanor-app --test service_composition_inventory --locked
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

## 8. Последние изменения

### 2026-07-19 — Composition-only bounded Graph and Store cleanup

- Public Graph contracts вынесены в `graph/model.rs`.
- Standard export/GraphML/hubs и pure adapters вынесены в `graph/standard.rs`.
- RusTok pure audit/graph adapters вынесены в `graph/rustok.rs`.
- Traversal-heavy logic переиспользует canonical cooperative owners без duplicate algorithms.
- Bounded regressions добавлены в `graph/tests.rs`.
- `lib.rs` маршрутизирует Graph через `graph/mod.rs`.
- 4.8k-line `graph.rs` физически удалён.
- Все standard/RusTok no-composition async project loaders удалены.
- Final production `crate::store::init_store` caller удалён.
- Public `store::init_store` facade удалён; Store types сохранены.
- Добавлен `graph_composition_inventory`; legacy inventory обновлён.
- Статус — implemented, Rust/hosted verification pending.

### 2026-07-19 — Composition-only bounded Check and API contracts

- Check разделён на model/execution/diagnostics/affected/tests и требует composition.
- API contracts разделены на model/snapshot/diff/retention/tests.
- API Store loading принадлежит composition owner; publication/diff/cleanup pure.
- Status — implemented, verification pending.

### 2026-07-19 — Composition-only Repair, Docs and Graph operations

- Repair execution консолидирован в одном composition owner.
- Legacy Docs service физически удалён.
- Standard Graph operation entrypoints требуют composition и operation context.
- Status — implemented, verification pending.

## 9. Ранее закрытые этапы

- `COMP-003A/B1/B2/C1/C2A/C2B1/C2B2A/C2B2B1/C2B2B2A/C2B2B2B/C2B2C1/C2B2C2A`
  реализованы в `main`.
- `DS-RESOLVE-003` verified.
- Runtime, MCP, publication, JSONL Store, Docs engine и CLI modularity changes находятся в `main`.
