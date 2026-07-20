# Athanor: консолидированный план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-20  
> Статус: active architecture audit

## 1. Правила статусов

- `[x] implemented` — изменение находится в `main`, но ещё не подтверждено execution evidence.
- `[x] verified` — реализация и regressions подтверждены одной выполненной matrix на одном commit.
- `[-] in progress` — полезные срезы находятся в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — подтверждённая работа ещё не начата.
- `[!] blocked` — execution evidence временно недоступно или matrix завершилась failure.

JSON является внешним контрактом. CLI, daemon и MCP варианты одной операции должны сериализовать
эквивалентный typed application payload. Documentation lifecycle status и snapshot metadata не
заменяют current-commit Rust execution evidence.

## 2. Текущее состояние

На уровне implementation завершены:

- `COMP-003` / `COMP-003C2B2C2B` — explicit runtime composition, bounded owners, Graph cleanup и
  удаление public Store initializer;
- `MCP-007` — transactional Index cancellation с pre-commit rollback и post-commit durable success;
- `JSON-003` — recursive schema inventory, Rust/adapter/automation lifecycle registries и typed
  transport parity;
- `DOC-001` / `DOC-002` — status hygiene и pipeline current/target/history;
- `MCP-004` — control-plane responsiveness при saturation request slots и response queue;
- `VERIFY-001A` — exact successful main-CI JSON evidence и commit-status channels;
- `VERIFY-001B` — lifecycle-aware docs completeness gate и current `ath init` config;
- `VERIFY-001C` — repository-owned Rust 1.95 setup для CI, production и release.

Composition-only execution действует для Index, Generation, Wiki, HTML, Search, Context, Explain,
ChangeMap, API, Overview, Capabilities, Impact, Coverage, Check, Graph, Repair, Docs и daemon work.

### Transactional Index invariant

1. pre-commit boundaries проверяют cancellation/deadline до и после bounded work;
2. cancellation до commit откатывает prepared artifacts и aborts snapshot;
3. commit-race terminal errors сверяются exact snapshot identity;
4. exact committed snapshot сохраняет durable success;
5. MCP не применяет transport postflight после Index application future;
6. CLI, daemon и MCP сериализуют один `IndexReport`.

### MCP control-plane invariant

1. stdin расположен перед request-task reaping в biased select;
2. notifications обрабатываются до ordinary admission;
3. inline parse/initialize/overload responses используют nonblocking bounded admission;
4. full response queue не блокирует stdin reader;
5. ordinary tasks сохраняют bounded `send().await` backpressure;
6. EOF вызывает cancellation до task drain.

### Documentation and verification invariant

1. roadmap является compact current-state ledger;
2. pipeline guide разделяет current, target и history;
3. completeness lifecycle допускает `active`, `implemented`, `planned`, `draft`, `verified`;
4. snapshot freshness принадлежит `docs drift`, а не completeness;
5. workflow YAML является implementation evidence, но не execution evidence;
6. `athanor.verification_evidence.v1` принадлежит workflow-owned automation registry;
7. `athanor/verification-matrix` публикуется на exact `GITHUB_SHA` после aggregation required jobs;
8. verified claim требует successful status или valid JSON evidence для того же SHA.

## 3. Завершённые пакеты

### 3.1 `COMP-003C2B2C2B` — composition cleanup

- [x] Read и write services требуют mandatory `RuntimeComposition`.
- [x] Check, API contracts и Graph разделены на conventional bounded modules.
- [x] Legacy Graph/Docs execution owners и public `store::init_store` удалены.
- [x] Source inventories запрещают возврат compatibility APIs.

### 3.2 `MCP-007` — transactional Index cancellation

- [x] Durable commit point определён exact canonical atomic publication.
- [x] Pre-commit termination откатывает read model/index state и aborts snapshot.
- [x] Commit-race errors сверяются exact snapshot probe и fail closed при ambiguity.
- [x] Post-commit cancellation не маскирует successful `IndexReport`.
- [x] MCP и daemon сохраняют durable-success semantics.

### 3.3 `JSON-003` — repeat contract inventory

- [x] Public, general non-public, adapter и automation registries unique и mutually disjoint.
- [x] Ordinary и qualified schema ids валидируются fail closed.
- [x] Production Rust sources сканируются рекурсивно.
- [x] Adapter legacy inputs нормализуются перед current write/response.
- [x] Workflow-owned persisted JSON имеет explicit owner и required fields.
- [x] CLI/daemon/MCP Index используют один typed public report.

### 3.4 `DOC-001` / `DOC-002` — documentation status hygiene

- [x] Snapshot-era roadmap заменён compact current-state ledger.
- [x] Aggregate stale verification claims удалены.
- [x] Removed Check/API/Graph paths заменены bounded owners.
- [x] Pipeline current/target/history разделены.
- [x] Documentation map, roadmap и implementation plan синхронизированы.
- [x] `documentation_status_inventory` фиксирует status/path/alignment/line budgets.

### 3.5 `MCP-004` — control-plane responsiveness

- [x] Notifications bypass ordinary request admission.
- [x] Inline reader-loop responses используют `try_send`.
- [x] Full/closed response queue не завершает reader loop.
- [x] EOF вызывает `cancel_all` до request-task drain.
- [x] Saturation, protocol-error, overload и disconnect regressions добавлены.
- [x] `control_plane_saturation_inventory` фиксирует routing и line budgets.

### 3.6 `VERIFY-001A` — exact CI evidence publication

- [x] JSON evidence и legacy status привязаны к exact workflow SHA.
- [x] Final CI job зависит от security, quality, feature matrix и coverage через `always()`.
- [x] Exact status context — `athanor/verification-matrix`.
- [x] Status публикует success только когда все required results равны `success`.
- [x] Evidence-only JSON commit не создаёт recursive CI loop.
- [x] `verification_evidence_inventory` source-enforces оба evidence channels.

### 3.7 `VERIFY-001B` — lifecycle-aware docs gate

- [x] Default policy не требует `last_verified_snapshot` как completeness field.
- [x] Lifecycle vocabulary: `active`, `implemented`, `planned`, `draft`, `verified`.
- [x] Snapshot freshness остаётся отдельным `ath docs drift` contract.
- [x] Root config, defaults и `ath init` используют один current policy.
- [x] Golden config fixture и lifecycle source inventory обновлены.

### 3.8 `VERIFY-001C` — executable workflow toolchain setup

- [x] Run `29701756503` на SHA `9fda772436f50b55b2e4d9b11b18b7ec1e43a091` записал exact failure.
- [x] Quality, feature и coverage jobs завершились до compilation на toolchain-install step.
- [x] Причина: pinned `dtolnay/rust-toolchain` commit жёстко выбирал `1.100.0`; workflow input
  `toolchain: 1.95.0` не являлся input этого action и игнорировался.
- [x] Добавлен `.github/actions/setup-rust/action.yml` с explicit Rust `1.95.0`, components, targets и retry.
- [x] CI, production и release используют только repository-owned setup.
- [x] Cargo-deny запускается без Docker build noise; failure log сохраняется artifact-ом.
- [x] `workflow_toolchain_inventory` запрещает возврат version-encoded action pin.

## 4. Следующие активные пакеты

### 4.1 `VERIFY-001` — execution matrix

- [x] Failure run `29701756503` получен и классифицирован; он не является verification success.
- [x] Toolchain root cause устранён в `main`.
- [ ] получить новый successful `athanor/verification-matrix` или valid evidence JSON;
- [ ] при remaining failure разобрать exact failed job/log/artifact;
- [ ] сверить evidence SHA с architecture commit;
- [ ] повысить только доказанные packages до `[x] verified`.

### 4.2 Product backlog

- [ ] deeper GraphQL/cross-protocol API consistency;
- [ ] broader relationship/framework adapters;
- [ ] richer analysis completeness reporting;
- [ ] evidence-backed documentation generation;
- [ ] release-readiness consolidation;
- [ ] i18n/concept mapping и optional semantic/vector retrieval.

## 5. Программа работ

| ID | Priority | Status | Результат / критерий закрытия |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Architecture packages implemented; exact success pending |
| `COMP-003` | P2 | `[x] implemented` | Runtime dependencies explicit |
| `COMP-003C2B2C2B` | P2 | `[x] implemented` | Read services, Graph и Store cleanup complete |
| `MCP-007` | P1 | `[x] implemented` | Transactional cancellation preserves durable success |
| `JSON-003` | P1 | `[x] implemented` | Rust, adapter and automation schemas are classified |
| `DOC-001` | P3 | `[x] implemented` | Stale verification and removed paths cleaned |
| `DOC-002` | P3 | `[x] implemented` | Pipeline/status docs aligned |
| `MCP-004` | P1 | `[x] implemented` | Control input remains observable under saturation |
| `VERIFY-001A` | P1 | `[x] implemented` | CI publishes exact JSON/status evidence channels |
| `VERIFY-001B` | P1 | `[x] implemented` | Docs gate matches lifecycle semantics and current config |
| `VERIFY-001C` | P1 | `[x] implemented` | Workflow toolchain setup matches workspace Rust 1.95 |
| `VERIFY-001` | P1 | `[!] blocked` | Exact successful status or JSON evidence identifies one commit |

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
cargo test -p athanor-app --test verification_evidence_inventory --locked
cargo test -p athanor-app --test docs_completeness_lifecycle_inventory --locked
cargo test -p athanor-app --test workflow_toolchain_inventory --locked
cargo test -p athanor-app --test config_contracts --locked
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
cargo run -p ath --quiet --locked -- docs check
```

Статус остаётся `implemented`, пока exact successful status или valid evidence JSON не идентифицирует
matrix для architecture commit.

## 7. Последние изменения

### 2026-07-20 — Workflow toolchain root-cause remediation

- Exact failure run показал, что проектный Rust код ещё не запускался.
- Version-encoded third-party setup заменён repository-owned Rust 1.95 action во всех active workflows.
- Cargo-deny diagnostics стали краткими и retrievable через workflow artifact.
- Status — implemented; новый exact CI result pending.

### 2026-07-19 — Connector-visible exact commit status

- Final CI job aggregates required results и публикует `athanor/verification-matrix`.
- JSON evidence остаётся persisted secondary channel.
- Status — implemented.

## 8. Historical status

Closed packages and earlier historical evidence remain available in Git history. Current architecture
packages are not promoted to verified without fresh exact execution evidence.
