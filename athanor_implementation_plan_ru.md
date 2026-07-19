# Athanor: консолидированный план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-19  
> Статус: active architecture audit

## 1. Правила статусов

- `[x] implemented` — изменение находится в `main`, но ещё не подтверждено execution evidence.
- `[x] verified` — реализация и regressions подтверждены одной выполненной matrix на одном commit.
- `[-] in progress` — полезные срезы находятся в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — подтверждённая работа ещё не начата.
- `[!] blocked` — безопасное изменение или проверка временно недоступны.

JSON является внешним контрактом. Несовместимое изменение требует нового major schema id или protocol
version. CLI, daemon и MCP варианты одной операции должны сериализовать эквивалентный typed
application payload.

Frontmatter `status` и `last_verified_snapshot` отдельных documentation pages являются metadata
содержания. Они не заменяют current-commit Rust execution evidence.

## 2. Текущее состояние

На уровне implementation завершены:

- `COMP-003` и `COMP-003C2B2C2B` — explicit runtime composition, bounded owners, Graph cleanup и
  удаление public Store initializer;
- `MCP-007` — transactional Index cancellation с различением pre-commit rollback и post-commit
  durable success;
- `JSON-003` — recursive workspace schema inventory, lifecycle enforcement и typed transport parity;
- `DOC-001` / `DOC-002` — status hygiene, active bounded owner paths и разделение pipeline
  current/target/history;
- `MCP-004` — control-plane responsiveness при одновременной saturation request slots и response
  queue.

Composition-only execution действует для Context, daemon, Search, Index, Generation, Wiki, HTML,
benchmark, Explain, ChangeMap, API, Overview, Capabilities, Impact, Coverage, Check, Graph, Repair и
Docs.

Conventional bounded owners без forwarding compatibility facades реализованы для ChangeMap,
Overview, Capabilities, Impact, Coverage, Check, API contracts и Graph.

### Transactional Index invariant

1. pre-commit adapter/Store boundaries проверяют `OperationContext::check_active()` до и после work;
2. cancellation/deadline до canonical commit приводит к rollback prepared artifacts и abort snapshot;
3. backend `Cancelled`/`DeadlineExceeded` на commit race сверяется с exact snapshot identity;
4. exact committed snapshot сохраняет durable success;
5. MCP регистрирует Index cancellation, но не применяет transport postflight после application future;
6. CLI, daemon и MCP сериализуют один public `IndexReport` contract.

### MCP control-plane invariant

1. stdin branch расположен перед request-task reaping в biased select loop;
2. notifications обрабатываются до ordinary request admission;
3. notification control input не потребляет request slot и не создаёт response;
4. inline parse/initialize/overload responses используют bounded nonblocking admission;
5. full response queue не завершает и не блокирует stdin reader;
6. ordinary request tasks сохраняют bounded `send().await` backpressure;
7. EOF отменяет registered operations до drain request tasks.

### Documentation invariant

1. `docs/development/roadmap-status.md` — compact current-state ledger;
2. `docs/architecture/pipeline.md` разделяет current, target и history;
3. aggregate status docs не используют stale snapshot metadata как current-commit evidence;
4. активные paths указывают на bounded Check/API/Graph и transactional Index owners;
5. `documentation_status_inventory` запрещает возврат удалённых paths и stale claims.

## 3. Завершённые пакеты

### 3.1 `COMP-003C2B2C2B` — composition cleanup

- [x] Удалены no-composition Search, Context, ChangeMap, Graph и read-service wrappers.
- [x] Read и write services требуют mandatory `RuntimeComposition`.
- [x] Check, API contracts и Graph разделены на conventional bounded modules.
- [x] Traversal-heavy Graph algorithms имеют canonical cooperative owners.
- [x] Legacy Graph monolith и legacy Docs execution owner физически удалены.
- [x] Repair execution консолидирован в composition owner.
- [x] Public `store::init_store` и последний production caller удалены.
- [x] Source inventories запрещают возврат compatibility APIs и фиксируют line budgets.

### 3.2 `MCP-007` — transactional Index cancellation

- [x] Durable commit point определён как successful exact canonical atomic publication.
- [x] Pre-commit boundaries проверяют operation cancellation/deadline до и после work.
- [x] Pre-commit termination откатывает read model/index state и aborts snapshot.
- [x] Commit-race terminal errors сверяются exact snapshot probe.
- [x] Exact committed snapshot сохраняет durable success.
- [x] Missing/uncommitted/mismatched/ambiguous probes fail closed.
- [x] MCP durable runner не применяет polling/postflight после application completion.
- [x] Daemon допускает `cancelling → succeeded` после durable publication.
- [x] Pre-commit, commit-race, post-commit и payload-parity regressions находятся в `main`.

### 3.3 `JSON-003` — repeat contract inventory

- [x] Public registry сохраняет 60 unique current Rust/schema owners.
- [x] General non-public registry сохраняет 30 boundary descriptors.
- [x] Adapter registry сохраняет два current и два legacy-input descriptors.
- [x] Public/general/adapter schema sets взаимно disjoint.
- [x] Ordinary и strict qualified schema forms валидируются fail closed.
- [x] Feature wire id сохраняет compatibility major `46`.
- [x] Production Rust sources сканируются рекурсивно.
- [x] Новый quoted `athanor.*` literal требует registration/classification.
- [x] Adapter legacy inputs нормализуются перед current write/response.
- [x] Current boundary fixtures и schema-less process protocols инвентаризированы.
- [x] CLI/daemon/MCP Index используют один typed public report.

### 3.4 `DOC-001` / `DOC-002` — documentation status hygiene

- [x] Snapshot-era roadmap заменён compact current-state ledger.
- [x] Aggregate stale verification claims удалены.
- [x] Removed Check/API/Graph monolith references заменены bounded paths.
- [x] Pipeline guide переписан вокруг explicit composition и bounded phase owners.
- [x] Current publication sequence согласован с journals и exact commit probe.
- [x] Pipeline target work отделён от current behavior и history.
- [x] Documentation map сокращён и переведён на active status semantics.
- [x] `documentation_status_inventory` фиксирует status/path/alignment/line budgets.

### 3.5 `MCP-004` — control-plane responsiveness

- [x] Stdin input имеет приоритет перед ordinary task reaping в biased select loop.
- [x] Notifications bypass ordinary request-limit admission.
- [x] Parse, initialize и overload inline responses используют `try_send` admission.
- [x] Full/closed response queue больше не завершает reader loop.
- [x] Ordinary request task responses сохраняют bounded async backpressure.
- [x] `notifications/cancelled` остаётся observable при saturated requests и responses.
- [x] EOF вызывает `cancel_all` до request-task drain.
- [x] Добавлены saturation, protocol-error, overload и disconnect regressions.
- [x] `control_plane_saturation_inventory` фиксирует routing и line budgets.
- [x] `direct-operation-context.md` согласован с фактическим behavior.

## 4. Следующие активные пакеты

### 4.1 `VERIFY-001` — execution matrix

- [!] Локальный checkout недоступен из текущего runtime из-за DNS-доступа к GitHub.
- [!] Hosted workflow runs для новых direct-to-main commits пока отсутствуют.
- [ ] выполнить fmt/check/test/Clippy/smoke matrix на одном commit;
- [ ] повысить только подтверждённые `[x] implemented` пункты до `[x] verified`.

### 4.2 Product backlog

- [ ] deeper GraphQL/cross-protocol API consistency;
- [ ] broader relationship/framework adapters;
- [ ] richer analysis completeness reporting;
- [ ] evidence-backed documentation generation;
- [ ] release-readiness consolidation;
- [ ] i18n/concept mapping;
- [ ] optional semantic/vector retrieval;
- [ ] additional Rustok/community-module/language integrations.

## 5. Программа работ

| ID | Priority | Status | Результат / критерий закрытия |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Architecture packages implemented; one-commit verification pending |
| `COMP-003` | P2 | `[x] implemented` | Runtime dependencies explicit; Store initialization only through composition |
| `COMP-003C2B2C2B` | P2 | `[x] implemented` | Read services, Graph и Store compatibility cleanup complete |
| `MCP-007` | P1 | `[x] implemented` | Pre-commit cancellation rolls back; post-commit durable success retained |
| `JSON-003` | P1 | `[x] implemented` | Schema scan, lifecycle registries, fixtures и payload parity enforced |
| `DOC-001` | P3 | `[x] implemented` | Aggregate stale verification claims и removed owner references удалены |
| `DOC-002` | P3 | `[x] implemented` | Pipeline current/target/history и status docs согласованы |
| `MCP-004` | P1 | `[x] implemented` | Control input remains observable under request/response saturation |
| `VERIFY-001` | P1 | `[!] blocked` | fmt/check/tests/Clippy/smoke выполнены на одном commit |

## 6. Verification matrix

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test -p athanor-app pipeline --locked
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
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p athanor-transport-mcp server::operation --locked
cargo test -p athanor-transport-mcp server::tests --locked
cargo test -p athanor-transport-mcp --test index_publication_cancellation_inventory --locked
cargo test -p athanor-transport-mcp --test control_plane_saturation_inventory --locked
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

## 7. Последние изменения

### 2026-07-19 — MCP control-plane saturation

- Reader loop prioritizes stdin before ordinary task reaping.
- Notifications bypass ordinary request admission.
- Inline reader-loop responses use nonblocking bounded admission.
- Full response queues no longer block or terminate control input processing.
- EOF cancels registered operations before saturated task drain.
- Saturation/disconnect tests and source inventory added.
- Status — implemented; Rust/hosted verification pending.

### 2026-07-19 — Documentation status and pipeline alignment

- Roadmap, pipeline and documentation map aligned with current owners and evidence semantics.
- Status — implemented; Rust/hosted verification pending.

### 2026-07-19 — JSON and transactional Index packages

- Recursive JSON contract inventory and transactional Index cancellation are implemented.
- Status — implemented; Rust/hosted verification pending.

## 8. Historical status

Closed composition subpackages and earlier verified historical slices remain available in Git history.
Current architecture packages are not promoted to verified without one fresh execution matrix on the
same commit.
