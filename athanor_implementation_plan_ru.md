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

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id или protocol
version. CLI, daemon и MCP варианты одной операции должны сохранять эквивалентные application payloads.

## 2. Текущее состояние

На уровне implementation завершены:

- `COMP-003` и `COMP-003C2B2C2B` — explicit runtime composition, bounded read owners, Graph cleanup и
  удаление public Store initializer;
- `MCP-007` — transactional Index cancellation с различением pre-commit rollback и post-commit
  durable success;
- `JSON-003` — repeat workspace-wide schema inventory, adapter/persistence lifecycle enforcement и
  typed transport payload parity.

Composition-only execution действует для Context, daemon, Search, Index, Generation, Wiki, HTML,
benchmark, Explain, ChangeMap, API, Overview, Capabilities, Impact, Coverage, Check, Graph, Repair и
Docs.

Conventional bounded owners без `include!` и forwarding compatibility facades реализованы для
ChangeMap, Overview, Capabilities, Impact, Coverage, Check, API contracts и Graph.

`crates/athanor-app/src/graph.rs` физически удалён. Public `store::init_store` удалён;
`store_facade.rs` экспортирует только `AthanorStore` и `StoreFactory`.

Transactional Index publication теперь имеет явную границу:

1. pre-commit adapter/Store boundaries проверяют `OperationContext::check_active()` до и после work;
2. cancellation/deadline до canonical commit приводит к rollback prepared artifacts и abort snapshot;
3. backend `Cancelled`/`DeadlineExceeded` на commit race сверяется с exact snapshot identity;
4. exact committed snapshot сохраняет durable success;
5. MCP регистрирует Index cancellation, но не выполняет transport postflight после application future;
6. CLI, daemon и MCP сериализуют один public `IndexReport` contract.

JSON contract inventory теперь имеет три независимых enforcement слоя:

1. public, general non-public и adapter registries валидируются на uniqueness/disjointness;
2. production Rust sources под `crates/*/src` и `apps/*/src` сканируются рекурсивно;
3. CLI/daemon/MCP equivalent operations обязаны сериализовать один typed public report.

## 3. Завершённые пакеты

### 3.1 `COMP-003C2B2C2B` — composition cleanup

- [x] Удалены no-composition Search, Context, ChangeMap, Graph и read-service wrappers.
- [x] Read и write services требуют mandatory `RuntimeComposition`.
- [x] ChangeMap, Overview, Capabilities, Impact, Coverage, Check и API contracts декомпозированы.
- [x] Graph contracts, standard builders, RusTok adapters и tests разделены на bounded owners.
- [x] Traversal-heavy Graph algorithms имеют canonical cooperative owners.
- [x] Legacy `graph.rs` физически удалён.
- [x] Repair execution консолидирован в composition owner.
- [x] Legacy Docs service физически удалён.
- [x] Public `store::init_store` и последний production caller удалены.
- [x] Source inventories запрещают возврат compatibility APIs и фиксируют line budgets.

### 3.2 `MCP-007` — transactional Index cancellation

- [x] Durable commit point определён как successful exact canonical atomic publication.
- [x] Pre-commit pipeline boundaries проверяют operation cancellation/deadline до и после work.
- [x] Pre-commit cancellation сохраняет terminal error, откатывает read model/index state и aborts
  uncommitted snapshot.
- [x] Commit-race `Cancelled` и `DeadlineExceeded` сверяются exact snapshot probe.
- [x] Exact committed snapshot преобразует terminal backend return в durable success.
- [x] Missing/uncommitted snapshot сохраняет rollback/error path; ambiguous probe не угадывается.
- [x] Post-commit cancellation не маскирует successful `IndexReport`.
- [x] MCP Index использует operation-aware application entrypoint.
- [x] MCP Index регистрируется для `notifications/cancelled` и disconnect cancellation.
- [x] MCP durable runner не применяет transport polling/postflight после application completion.
- [x] Daemon допускает `cancelling → succeeded`, если durable publication уже завершена.
- [x] Добавлены pre-commit, commit-race и post-commit publication regressions.
- [x] Source inventory фиксирует CLI/daemon/MCP `IndexReport` payload parity и line budgets.
- [x] `direct-operation-context.md` согласован с transactional contract.

### 3.3 `JSON-003` — repeat contract inventory

- [x] Public registry сохраняет 60 unique current Rust/schema owners.
- [x] General non-public registry сохраняет 30 persisted/generated/interchange/embedded descriptors.
- [x] Adapter registry сохраняет два current и два legacy-input descriptors.
- [x] Public, general non-public и adapter schema sets проверяются на взаимную disjointness.
- [x] Qualified schema grammar поддерживает wire-compatible
  `athanor.index_state.v46-js-ts-precision-v1` и возвращает base major `46`.
- [x] Malformed qualified versions fail closed.
- [x] Старый path-list `json_contract_inventory` заменён recursive workspace production scan.
- [x] Unit-test source owners исключены из production emitter scan.
- [x] Любой новый quoted `athanor.*` production literal требует registration/classification в том же change.
- [x] Persistence/process source inventory больше не ссылается на удалённые monolith paths.
- [x] Adapter legacy manifest/trust inputs нормализуются в current owners перед current write/response.
- [x] Current persisted/generated/interchange fixtures сохраняют required-field coverage.
- [x] External process protocols остаются schema-less и сохраняют framing/type inventory.
- [x] CLI, daemon и MCP Index используют один typed `IndexReport` payload.
- [x] Daemon Index/Generation/HTML/Wiki regressions сравнивают transport result с direct serde shape.
- [x] `json-contract-inventory.md` отражает фактические registries, recursive scan и pending verification.

## 4. Следующие активные пакеты

### 4.1 `DOC-001` / `DOC-002` — documentation status hygiene

- [ ] убрать stale `verified` claims, не подтверждённые одним current commit;
- [ ] заменить ссылки на удалённые monolith paths актуальными bounded owners;
- [ ] согласовать pipeline current/target/history;
- [ ] синхронизировать roadmap, architecture guides и implementation plan.

### 4.2 `MCP-004` — control-plane responsiveness

- [ ] подтвердить обработку cancellation/control notifications при saturation ordinary request slots;
- [ ] исключить starvation control-plane сообщений;
- [ ] добавить bounded saturation/disconnect regressions.

### 4.3 `VERIFY-001` — execution matrix

- [!] Локальный checkout недоступен из текущего runtime из-за DNS-доступа к GitHub.
- [!] Hosted workflow runs для новых direct-to-main commits пока отсутствуют.
- [ ] выполнить fmt/check/test/Clippy/smoke matrix на одном commit;
- [ ] повысить только подтверждённые `[x] implemented` пункты до `[x] verified`.

## 5. Программа работ

| ID | Priority | Status | Результат / критерий закрытия |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Composition, MCP-007 и JSON-003 implemented; docs/control-plane/verification pending |
| `COMP-003` | P2 | `[x] implemented` | Runtime dependencies explicit; Store initialization only through composition |
| `COMP-003C2B2C2B` | P2 | `[x] implemented` | Read services, Graph и Store compatibility cleanup complete |
| `MCP-007` | P1 | `[x] implemented` | Pre-commit cancellation rolls back; post-commit durable success retained |
| `JSON-003` | P1 | `[x] implemented` | Workspace schema scan, lifecycle registries, fixtures и payload parity enforced |
| `DOC-001` | P3 | `[-] in progress` | Stale verification claims и removed paths удалены |
| `DOC-002` | P3 | `[ ] planned` | Pipeline current/target/history согласованы |
| `MCP-004` | P1 | `[ ] planned` | Control-plane остаётся responsive при request saturation |
| `VERIFY-001` | P1 | `[!] blocked` | fmt/check/tests/Clippy/smoke выполнены на одном commit |

## 6. Реализованные архитектурные срезы

### Runtime composition

- [x] Process-global runtime owners, installer APIs и legacy factory errors удалены.
- [x] Parallel isolated compositions покрывают Store/Search/Wiki/HTML factories.
- [x] Context, daemon, write services, projectors и read-service families composition-only.
- [x] Standard и RusTok Graph project execution composition-only.
- [x] Public Store initializer physically removed.

### Bounded ownership

- [x] Runtime, Docs engine, JSONL Store, Search, MCP и CLI монолиты декомпозированы.
- [x] ChangeMap, Overview, Capabilities, Impact, Coverage, Check и API contracts декомпозированы.
- [x] Graph декомпозирован; cooperative algorithms имеют single owners.
- [x] Repair execution консолидирован; legacy Docs execution owner удалён.

### Protocols и publication

- [x] MCP bounded queues, in-flight limits и task reaping реализованы.
- [x] JSON-RPC `2.0`, error codes, id/null/session semantics реализованы.
- [x] Read operations reject late success after cancellation.
- [x] Transactional Index distinguishes pre-commit termination from post-commit success.
- [x] MCP Index cancellation is registered without a transport post-commit check.
- [x] Canonical commit-race terminal errors use exact identity reconciliation.
- [x] Publication post-commit cleanup remains best effort and journal-recoverable.
- [x] Pointer publication has one owner.
- [x] API snapshots remain immutable.
- [x] Repair latest/recovery preserve authoritative generation semantics.

### JSON contracts

- [x] Public/current, non-public/current, legacy-input и historical lifecycles разделены.
- [x] Adapter manifest/trust persisted state и public trust report имеют разные owners.
- [x] Ordinary и qualified versioned schema IDs валидируются fail-closed.
- [x] Production schema literals сканируются рекурсивно по workspace source tree.
- [x] Removed monolith paths не участвуют в inventory.
- [x] Persisted/generated/interchange fixtures покрывают required current fields.
- [x] Process protocols сохраняют schema-less typed framing.
- [x] Typed CLI/daemon/MCP payload parity закреплена source и serde regressions.

## 7. Verification matrix

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test -p athanor-app pipeline_support --locked
cargo test -p athanor-app store_publication_cancellation --locked
cargo test -p athanor-app index_publication_fault_tests --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
cargo test -p athanor-app --test read_service_composition_inventory --locked
cargo test -p athanor-app --test check_composition_inventory --locked
cargo test -p athanor-app --test api_composition_inventory --locked
cargo test -p athanor-app --test graph_composition_inventory --locked
cargo test -p athanor-app --test service_composition_inventory --locked
cargo test -p athanor-app --test composition_isolation --locked
cargo test -p athanor-app --test context_composition_inventory --locked
cargo test -p athanor-app --test daemon_composition_inventory --locked
cargo test -p athanor-app --test write_service_composition_inventory --locked
cargo test -p athanor-app --test runtime_modularity_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test adapter_contract_inventory --locked
cargo test -p athanor-app --test public_report_transport_inventory --locked
cargo test -p athanor-transport-mcp server::operation --locked
cargo test -p athanor-transport-mcp --test index_publication_cancellation_inventory --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo test -p athanor-transport-mcp --locked
cargo test -p athanor-store-jsonl --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo run -p ath --quiet --locked -- --help
cargo run -p ath --quiet --locked -- index .
```

Новые срезы сохраняют статус `implemented`, пока этот набор не выполнен на одном commit.

## 8. Последние изменения

### 2026-07-19 — Repeat workspace JSON contract inventory

- Schema validator поддерживает ordinary и strict qualified version forms.
- Qualified JS/TS precision IndexState wire id сохраняется без breaking migration.
- JSON contract inventory рекурсивно сканирует production Rust sources вместо stale path list.
- Persistence/process inventory больше не зависит от удалённых API/Graph/Check monolith paths.
- Public/general/adapter registries проверяются на uniqueness, disjointness и source observability.
- Adapter legacy input normalization и current fixture rules сохранены.
- CLI/daemon/MCP typed `IndexReport` parity и daemon write-report parity закреплены source guards.
- Status — implemented; Rust/hosted verification pending.

### 2026-07-19 — Transactional MCP Index cancellation

- Pre-commit pipeline boundary helper переведён с deadline-only на active cancellation/deadline checks.
- `AthanorStore` reconciles terminal commit-race errors through exact snapshot identity.
- Full publication regressions проверяют abort/rollback, committed terminal races, current pointer и
  journal cleanup.
- MCP dispatcher использует operation-aware Index API.
- MCP server регистрирует Index cancellation, но awaits application future without external
  polling/postflight.
- CLI/daemon/MCP `IndexReport` parity закреплена source inventory.
- Status — implemented; Rust/hosted verification pending.

### 2026-07-19 — Composition-only bounded Graph and Store cleanup

- Graph model/standard/RusTok/tests split implemented; legacy 4.8k-line owner deleted.
- Public Store initializer and final hidden Store caller deleted.
- Status — implemented; verification pending.

## 9. Ранее закрытые этапы

- `COMP-003A/B1/B2/C1/C2A/C2B1/C2B2A/C2B2B1/C2B2B2A/C2B2B2B/C2B2C1/C2B2C2A`
  реализованы в `main`.
- `DS-RESOLVE-003` verified.
- Runtime, MCP, publication, JSONL Store, Docs engine и CLI modularity changes находятся в `main`.
