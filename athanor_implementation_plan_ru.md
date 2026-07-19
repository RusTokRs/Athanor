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
- `[!] blocked` — execution evidence временно недоступно.

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
- `VERIFY-001A` — exact successful main-CI evidence publisher и automation-owned evidence contract;
- `VERIFY-001B` — lifecycle-aware docs completeness gate, valid `ath init` config и refreshed golden
  config contract.

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
4. default completeness не требует `last_verified_snapshot`; freshness принадлежит `docs drift`;
5. root `athanor.toml`, `ProjectConfig::default` и `ath init` используют один current policy;
6. workflow YAML является implementation evidence, но не execution evidence;
7. `athanor.verification_evidence.v1` принадлежит workflow-owned automation registry;
8. valid evidence содержит exact successful CI SHA и run identity.

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

- [x] `verification-evidence.yml` принимает только completed successful push-CI на `main`.
- [x] Evidence содержит schema, exact SHA, run id/URL, completion time и matrix.
- [x] Evidence-only path исключён из push-CI, предотвращая recursive loop.
- [x] Workflow коммитит только `docs/development/verification-evidence.json`.
- [x] `AUTOMATION_JSON_CONTRACTS` классифицирует evidence как persisted current workflow document.
- [x] `verification_evidence_inventory` связывает workflow, schema constant и required fields.

### 3.7 `VERIFY-001B` — lifecycle-aware docs gate

- [x] Default policy больше не требует `last_verified_snapshot` как completeness field.
- [x] Lifecycle vocabulary: `active`, `implemented`, `planned`, `draft`, `verified`.
- [x] Snapshot freshness остаётся отдельным `ath docs drift` contract.
- [x] Корневой `athanor.toml` явно задаёт repository policy.
- [x] `ath init` генерирует только поля текущего `ProjectConfig` и имеет parse regression.
- [x] `config_contracts.v1.json` обновлён под current default contract.
- [x] `docs_completeness_lifecycle_inventory` фиксирует default/init/root parity.

## 4. Следующие активные пакеты

### 4.1 `VERIFY-001` — execution matrix

- [!] Локальный checkout и `gh` недоступны в текущем runtime.
- [x] Self-recording exact evidence workflow находится в `main`.
- [ ] получить valid `docs/development/verification-evidence.json` от successful final push-CI;
- [ ] сверить recorded `head_sha` с architecture commit;
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
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Architecture packages implemented; exact evidence pending |
| `COMP-003` | P2 | `[x] implemented` | Runtime dependencies explicit |
| `COMP-003C2B2C2B` | P2 | `[x] implemented` | Read services, Graph и Store cleanup complete |
| `MCP-007` | P1 | `[x] implemented` | Transactional cancellation preserves durable success |
| `JSON-003` | P1 | `[x] implemented` | Rust, adapter and automation schemas are classified |
| `DOC-001` | P3 | `[x] implemented` | Stale verification and removed paths cleaned |
| `DOC-002` | P3 | `[x] implemented` | Pipeline/status docs aligned |
| `MCP-004` | P1 | `[x] implemented` | Control input remains observable under saturation |
| `VERIFY-001A` | P1 | `[x] implemented` | Successful main CI can publish classified exact evidence |
| `VERIFY-001B` | P1 | `[x] implemented` | Docs gate matches lifecycle semantics and current config |
| `VERIFY-001` | P1 | `[!] blocked` | Valid evidence JSON identifies one successful commit |

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

Статус остаётся `implemented`, пока valid evidence-файл не идентифицирует successful matrix для exact
architecture commit.

## 7. Последние изменения

### 2026-07-19 — Workflow-owned JSON evidence contract

- Verification evidence получил отдельный automation registry и exported schema constant.
- Public/general/adapter/automation sets взаимно disjoint и source-observable.
- Status — implemented; final CI evidence pending.

### 2026-07-19 — Documentation lifecycle gate repair

- Completeness lifecycle отделён от snapshot freshness.
- Root policy и `ath init` приведены к текущему `ProjectConfig`.
- Golden config fixture и source inventory обновлены.
- Status — implemented; final CI evidence pending.

## 8. Historical status

Closed packages and earlier historical evidence remain available in Git history. Current architecture
packages are not promoted to verified without fresh exact execution evidence.
