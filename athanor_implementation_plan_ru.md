# Athanor: консолидированный план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-20  
> Статус: active architecture audit

## 1. Правила статусов

- `[x] implemented` — изменение находится в `main`, но ещё не подтверждено execution evidence.
- `[x] verified` — реализация и regressions подтверждены successful matrix на одном exact commit.
- `[-] in progress` — полезные срезы находятся в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — подтверждённая работа ещё не начата.
- `[!] blocked` — matrix отсутствует или завершилась failure.

JSON является внешним контрактом. CLI, daemon и MCP варианты одной операции должны сериализовать
эквивалентный typed application payload. Documentation lifecycle metadata не заменяет current-commit
Rust execution evidence.

## 2. Текущее состояние

На уровне implementation завершены:

- `COMP-003` / `COMP-003C2B2C2B` — explicit runtime composition, bounded owners, Graph cleanup и
  удаление public Store initializer;
- `MCP-007` — transactional Index cancellation с pre-commit rollback и post-commit durable success;
- `JSON-003` — recursive Rust/adapter/automation schema lifecycle registries и transport parity;
- `DOC-001` / `DOC-002` — status hygiene и pipeline current/target/history;
- `MCP-004` — control-plane responsiveness при saturated request/response paths;
- `VERIFY-001A` — exact JSON evidence и `athanor/verification-matrix` status channels;
- `VERIFY-001B` — lifecycle-aware docs completeness и current `ath init` config;
- `VERIFY-001C` — repository-owned Rust 1.95 setup для CI, production и release;
- `VERIFY-001D` — fail-only artifacts для fmt, default feature compile и cargo-deny diagnostics;
- `VERIFY-001E` — canonical formatting, compile-owner cleanup и patched advisory lockfile.

Composition-only execution действует для Index, Generation, Wiki, HTML, Search, Context, Explain,
ChangeMap, API, Overview, Capabilities, Impact, Coverage, Check, Graph, Repair, Docs и daemon work.

### Transactional Index invariant

1. pre-commit boundaries проверяют cancellation/deadline до и после bounded work;
2. cancellation до commit откатывает prepared artifacts и aborts snapshot;
3. commit-race terminal errors сверяются exact snapshot identity;
4. exact committed snapshot сохраняет durable success;
5. MCP не применяет transport postflight после Index application future;
6. CLI, daemon и MCP сериализуют один `IndexReport`.

### Documentation and verification invariant

1. roadmap является compact current-state ledger;
2. pipeline guide разделяет current, target и history;
3. completeness lifecycle допускает `active`, `implemented`, `planned`, `draft`, `verified`;
4. snapshot freshness принадлежит `docs drift`, а не completeness;
5. workflow YAML является implementation evidence, но не execution evidence;
6. exact success требует successful status или valid JSON evidence для того же SHA;
7. failed matrix является diagnostic evidence и не повышает status до verified.

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

### 3.3 `JSON-003` — repeat contract inventory

- [x] Public, general non-public, adapter и automation registries unique и mutually disjoint.
- [x] Ordinary и qualified schema ids валидируются fail closed.
- [x] Production Rust sources сканируются рекурсивно.
- [x] Adapter legacy inputs нормализуются перед current write/response.
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
- [x] Saturation regressions и `control_plane_saturation_inventory` добавлены.

### 3.6 `VERIFY-001A` — exact CI evidence publication

- [x] JSON evidence и legacy status привязаны к exact workflow SHA.
- [x] Final CI job агрегирует security, quality, feature matrix и coverage через `always()`.
- [x] Status context — `athanor/verification-matrix`.
- [x] Success публикуется только когда все required results равны `success`.
- [x] Evidence-only JSON commit не создаёт recursive CI loop.

### 3.7 `VERIFY-001B` — lifecycle-aware docs gate

- [x] Default policy не требует `last_verified_snapshot` как completeness field.
- [x] Lifecycle vocabulary: `active`, `implemented`, `planned`, `draft`, `verified`.
- [x] Root config, defaults и `ath init` используют один current policy.
- [x] Golden config fixture и lifecycle source inventory обновлены.

### 3.8 `VERIFY-001C` — executable workflow toolchain setup

- [x] Run `29701756503` на SHA `9fda772436f50b55b2e4d9b11b18b7ec1e43a091` записал exact failure.
- [x] Quality, feature и coverage jobs остановились до compilation на toolchain-install step.
- [x] Pinned third-party action фактически выбирал Rust `1.100.0`, игнорируя неизвестный input `toolchain`.
- [x] `.github/actions/setup-rust/action.yml` устанавливает exact Rust `1.95.0` с retry/components/targets.
- [x] CI, production и release используют repository-owned setup.
- [x] `workflow_toolchain_inventory` запрещает возврат version-encoded action pin.

### 3.9 `VERIFY-001D` — exact failure diagnostics

- [x] Run `29721874834` на SHA `c0876bef110fad4a3dd105f7c46a45aeb01a98f8` подтвердил successful Rust 1.95 setup во всех jobs.
- [x] Quality jobs дошли до `cargo fmt --check` и завершились failure.
- [x] Feature и coverage jobs дошли до project compilation и завершились failure.
- [x] Cargo-deny artifact подтвердил advisories:
  - `ammonia 4.1.2` / `RUSTSEC-2026-0185`;
  - `anyhow 1.0.102` / `RUSTSEC-2026-0190`;
  - `crossbeam-epoch 0.9.18` / `RUSTSEC-2026-0204`.
- [x] CI сохраняет `cargo-fmt-diagnostics`, `default-feature-diagnostics` и `cargo-deny-diagnostics` только при failure.

### 3.10 `VERIFY-001E` — project-level remediation

- [x] Run `29722608851` предоставил exact formatting diff и compile diagnostics.
- [x] MCP visibility, transport helper shadowing, request-task ownership и extraction queue ownership исправлены.
- [x] Canonical `cargo fmt` применён к workspace; run `29727776673` подтвердил formatting success на Linux, macOS и Windows.
- [x] Неиспользуемый duplicate `docs/service.rs` удалён; единственный composition owner остаётся в `application_report_composition/docs_direct`.
- [x] Production-only warnings устранены: Store trait import удалён, adapter report type ограничен `cfg(test)`.
- [x] Cargo 1.95 сгенерировал и проверил patched lockfile:
  - `anyhow 1.0.103`;
  - `ammonia 4.1.3` и согласованный HTML/CSS dependency graph;
  - `crossbeam-epoch 0.9.20`.
- [x] Bot commit `b82eff9dee595394cf5b36a8ad8aa983e944db6c` изменил только `Cargo.lock` после `cargo check --workspace --locked` и cargo-deny success.
- [x] Одноцелевой lock remediation workflow удалён после применения.

### 3.11 `VERIFY-001F` — validated execution remediation V10

- [x] Run `29739854537` подтвердил Rust 1.95 setup, formatting и cargo-deny; workspace/feature execution остановилась на source blockers.
- [x] Run `29741168520` повторно подтвердил formatting и security, сохранив exact failed matrix evidence для оставшегося compile path.
- [x] Run `29744943097` подтвердил OXC family 0.139 с `--all-features`, executable CLI JSON regression и прохождение Repair cleanup до Clippy.
- [x] Run `29754747369` подтвердил targeted MCP suite, default Clippy и all-features Clippy; source publication остановилась только на дефектном evidence anchor.
- [x] Run `29755384879` повторно подтвердил remediation и успешно записал plan evidence; publication остановилась только на overly strict path equality gate.
- [x] `RequestRuntime` владеет MCP lifecycle dependencies; `process_line` больше не имеет восемь аргументов.
- [x] Tool dispatch принимает `&Path`; `ProtocolFailure` хранит boxed `RpcError` без изменения parser signature и JSON-RPC поведения.
- [x] Targeted MCP suite, default/all-features check и Clippy повторно выполнены успешно в publication run `29756009418`.
- [x] Временные execution/source-warning remediation workflows физически удалены из validated source commit.
- [ ] Полная `athanor/verification-matrix` должна стать successful на опубликованном architecture commit до повышения пакетов до `[x] verified`.

## 4. Следующие активные пакеты

### 4.1 `VERIFY-001` — execution matrix

- [x] Toolchain infrastructure failure устранён.
- [x] Canonical formatting применён и подтверждён на трёх ОС.
- [x] Известные project compile blockers и warnings устранены.
- [x] Advisory lockfile обновлён и проверен cargo-deny.
- [ ] получить exact matrix result для текущего HEAD;
- [ ] разобрать remaining tests/Clippy/coverage/smoke failures, если они останутся;
- [ ] сверить successful evidence SHA с architecture commit;
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
| `JSON-003` | P1 | `[x] implemented` | Schema lifecycle and payload parity enforced |
| `DOC-001` | P3 | `[x] implemented` | Stale verification and removed paths cleaned |
| `DOC-002` | P3 | `[x] implemented` | Pipeline/status docs aligned |
| `MCP-004` | P1 | `[x] implemented` | Control input remains observable under saturation |
| `VERIFY-001A` | P1 | `[x] implemented` | Exact JSON/status evidence channels |
| `VERIFY-001B` | P1 | `[x] implemented` | Docs gate matches lifecycle semantics |
| `VERIFY-001C` | P1 | `[x] implemented` | Workflow toolchain matches Rust 1.95 |
| `VERIFY-001D` | P1 | `[x] implemented` | Failed fmt/compile/security output is retrievable |
| `VERIFY-001E` | P1 | `[x] implemented` | Fmt, compile owners and advisory lockfile remediated |
| `VERIFY-001F` | P1 | `[x] implemented` | Structural MCP and execution blockers closed by validated V10 |
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

### 2026-07-20 — Validated execution remediation V10

- Diagnostic runs `29739854537`, `29741168520` и `29744943097` сведены в один source remediation.
- Run `29754747369` доказал successful targeted MCP suite и оба Clippy; publication была заблокирована только evidence writer.
- Run `29755384879` повторно подтвердил source и plan evidence; commit был заблокирован только overly strict equality gate.
- MCP lifecycle, path ownership, protocol error size и физическая Repair migration исправлены структурно, без lint suppressions.
- Publication run `29756009418` повторно проверил source patch до прямой публикации в `main`.
- Временные remediation workflows удалены.
- Status — implemented; полная exact verification matrix остаётся обязательной.

### 2026-07-20 — Project-level CI remediation

- Canonical formatting применён и подтверждён на Linux, macOS и Windows.
- Duplicate Docs service owner и known compile warnings удалены.
- Advisory dependency graph обновлён exact Cargo 1.95 resolver-ом и проверен cargo-deny.
- Status — implemented; final exact matrix pending.

## 8. Historical status

Closed packages and earlier historical evidence remain available in Git history. Current architecture
packages are not promoted to verified without fresh exact execution evidence.
