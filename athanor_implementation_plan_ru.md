# Athanor: консолидированный план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-17  
> Статус: active architecture audit

## Правила статусов

- `[x] implemented` — исправление находится в `main`, но ещё не считается проверенным без выполнения regressions.
- `[x] verified` — реализация и regressions подтверждены фактически выполненными проверками.
- `[-] in progress` — полезный срез находится в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — работа не начата.
- `[!] blocked` — требуется compatibility decision, безопасный способ изменения или недоступная платформенная проверка.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id или protocol version. Эквивалентные CLI, daemon и MCP операции не должны иметь разные application payload shapes.

## Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | MCP, adapter current contracts и первый composition slice реализованы; execution/cleanup pending |
| `DS-JSON-001` | P1 | `[-] in progress` | Public registry 60; current adapter owners разделены, legacy library API остаётся |
| `DS-JSON-002` | P1 | `[-] in progress` | 30 general + 4 adapter-specific non-public descriptors; execution и repeat scan pending |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed CLI/daemon/MCP/plugin payload parity реализована; execution pending |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |

## Реестр всех подтверждённых дефектов и рисков

| ID | Priority | Status | Подтверждённая проблема | Критерий закрытия |
| --- | --- | --- | --- | --- |
| `MCP-001` | P1 | `[x] implemented` | Response path использовал unbounded channel | Bounded queue, single writer и выполненные MCP regressions |
| `MCP-002` | P1 | `[x] implemented` | Requests spawn-ились без лимита, completed tasks жили до EOF | In-flight cap, continuous reap и выполненные load/lifecycle tests |
| `MCP-003` | P1 | `[x] implemented` | Неверные JSON-RPC version/error/id/session semantics | Exact `2.0`, standard codes, distinct omitted/null id, session-state tests |
| `MCP-004` | P2 | `[ ] planned` | При полной saturation ordinary request limit может задерживать cancellation notification/control-plane input | Отдельный control-plane path либо доказанная responsiveness regression |
| `JSON-001` | P1 | `[x] implemented` | Current adapter manifest имел неверсионированный schema id | `athanor.adapter_manifest.v1`; legacy id только migration input |
| `JSON-002` | P1 | `[-] in progress` | `athanor.adapter_trust.v2` владел persisted registry и public report | Current paths разделены; legacy library emitters удалены/deprecated |
| `JSON-003` | P1 | `[-] in progress` | Inventory ошибочно считался complete без adapter boundaries | Adapter registry/fixtures добавлены; repeat scan и execution завершены |
| `JSON-004` | P2 | `[x] implemented` | Adapter contract symbols были re-exported двумя facade paths | Один canonical re-export path и Clippy без ambiguous re-export warnings |
| `RUST-001` | P1 | `[!] blocked` | Unix test `ProcessCommand` literal дважды задаёт `clear_environment` | Полная безопасная замена `runtime.rs`, source guard и Rust compile |
| `RUST-002` | P1 | `[!] blocked` | Non-Windows helper не задаёт обязательный `clear_environment` | Полная безопасная замена `runtime.rs`, source guard и Rust compile |
| `COMP-001` | P2 | `[-] in progress` | `OnceLock::set` conflicts молча игнорировались | Adapter/projector исправлены; Store/Search получают typed conflict errors |
| `COMP-002` | P1 | `[x] implemented` | Отсутствующий adapter factory молча создавал empty registry | Legacy path fail-fast; explicit composition остаётся штатным path |
| `COMP-003` | P2 | `[-] in progress` | Store/Search/adapter/projector globals сохраняют process-wide hidden dependencies | Production entrypoints используют `RuntimeComposition`; globals удалены после parity |
| `COMP-004` | P2 | `[ ] planned` | `validate_changed` использует `RuntimeBuilder::new` и hidden global adapter composition | Добавлен composition-aware API и active CLI/daemon path переведён на него |
| `COMP-005` | P2 | `[ ] planned` | Legacy trust functions в `runtime.rs` ещё возвращают report со старым shared schema | API deprecated/removed либо возвращает только versioned facade type |
| `PUB-001` | P1 | `[x] implemented` | Adapter trust writer мог вернуть failure после durable rename из-за backup cleanup | Cleanup best-effort warning после commit point |
| `PUB-002` | P1 | `[ ] planned` | Другие staged-replace helpers могут смешивать durable success и post-commit maintenance failure | Полный inventory durability points и исправленные helpers |
| `PUB-003` | P2 | `[ ] planned` | Нет системных failure-injection regressions для stage/publish/rollback/cleanup | Tests для каждой фазы и repair-visible outcomes |
| `CLI-001` | P2 | `[-] in progress` | `entry.rs` включает monolithic `main.rs`; grammar/rendering дублируются | Один root command model; `include!("main.rs")` удалён |
| `CLI-002` | P2 | `[x] implemented` | Plugin commands использовали legacy shared trust-report schema | Focused dispatcher и executable parity regression |
| `DOC-001` | P3 | `[-] in progress` | Документы содержат stale `verified` claims без current-HEAD evidence | Все затронутые документы имеют честный status/evidence |
| `DOC-002` | P3 | `[ ] planned` | Pipeline architecture содержит противоречия между implemented и planned sections | Current/target/history sections согласованы |
| `VERIFY-001` | P1 | `[!] blocked` | В текущей среде нет Rust toolchain, hosted checks для direct-to-main commits не видны | Выполнены fmt/tests/Clippy/workspace smoke и результаты записаны |
| `TOOL-001` | P2 | `[!] blocked` | GitHub contents connector не поддерживает точечный patch большого `runtime.rs` | Получена проверяемая полная копия blob либо доступен безопасный patch path |

## ARCH-AUDIT-001 — Repository-wide architecture audit

**Статус:** `[-] in progress`.

Рабочий audit tracker: GitHub issue `#16`.

### Порядок аудита

1. MCP transport lifecycle и JSON-RPC correctness.
2. Adapter manifest/trust schema migration и JSON inventory repair.
3. Process-global factories и final explicit-composition migration.
4. Publication, rollback и post-commit cleanup semantics.
5. CLI command model и удаление legacy monolith/include path.
6. Store/backend conformance и transaction boundaries.
7. Adapter correctness, incremental invalidation и resource limits.
8. Security/operations hardening.
9. Rust-enabled workspace verification и reconciliation документации.

### MCP transport hardening

#### Реализовано

- [x] Response queue ограничена 32 serialized responses.
- [x] Ordinary request concurrency ограничена 32 in-flight tasks.
- [x] Завершённые request tasks извлекаются из `JoinSet` во время input loop.
- [x] stdout имеет одного async writer owner; EOF cancels reads, drains tasks и joins writer.
- [x] JSON-RPC exact `2.0`, standard error categories и MCP protocol `2024-11-05` enforced.
- [x] Omitted id отделён от explicit `null`.
- [x] Session state: `AwaitingInitialize -> AwaitingInitialized -> Ready`.
- [x] Сохранены cancellation/deadlines и drained Search/Context cleanup.
- [x] Старый `lifecycle.rs` удалён; active implementation и regressions находятся в `server.rs`.

#### Осталось

- [ ] Выполнить MCP unit/integration tests, fmt и Clippy.
- [ ] Проверить interoperability с реальным MCP client.
- [ ] Решить `MCP-004`: cancellation/control-plane responsiveness при полной saturation.

### Adapter JSON boundary migration

#### Реализовано для current paths

- [x] Current manifest: `athanor.adapter_manifest.v1`.
- [x] Legacy `athanor.adapter_manifest` нормализуется в памяти.
- [x] Current persisted trust registry: `athanor.adapter_trust_registry.v2`.
- [x] Legacy persisted `athanor.adapter_trust.v2` нормализуется перед current write.
- [x] Public report: `VersionedAdapterTrustReport` / `athanor.adapter_trust_report.v1`.
- [x] Public registry вырос с 59 до 60 owners.
- [x] `ADAPTER_NON_PUBLIC_JSON_CONTRACTS`: 2 current + 2 legacy inputs.
- [x] Добавлены fixture, migration, disjointness, lifecycle и source-observability regressions.
- [x] `ath plugins list/trust/untrust` используют focused current-schema dispatcher.
- [x] Existing grammar и human-readable rendering сохранены.
- [x] Trust registry backup cleanup после durable publish стал best-effort warning.
- [x] Architecture, RusTok FFA, FBA и Page Builder manifest examples переведены на current schema.
- [x] Документы с изменённым current-HEAD contract переведены из stale `verified` в `draft`.
- [x] Устранён duplicate facade re-export adapter contract symbols.

#### Осталось

- [ ] Закрыть `JSON-002`/`COMP-005`: deprecated/remove legacy `runtime.rs` trust-report functions.
- [ ] Выполнить adapter contract и executable CLI regressions.
- [ ] Повторить repository-wide schema scan после compatibility cleanup.

### Подтверждённые compile-level defects

- [!] `RUST-001`: Unix runtime test задаёт `clear_environment` дважды.
- [!] `RUST-002`: non-Windows helper не задаёт обязательный `clear_environment`.
- [ ] Исправить оба literals через гарантированную полную копию большого `runtime.rs`.
- [ ] Добавить source guard, исключающий duplicate/missing field.

Небезопасная ручная замена 1800+ строк через contents API не выполняется без полной проверяемой копии исходного blob.

### Explicit composition migration

#### Реализован первый bounded slice

- [x] Добавлены `LegacyFactoryInstallError` и `LegacyFactoryUnavailableError`.
- [x] Общий `install_once` превращает повторный `OnceLock::set` в explicit error.
- [x] Wiki/HTML projector factories получили `try_install_*` APIs.
- [x] Legacy projector wrappers больше не игнорируют conflicting install.
- [x] Adapter registry/resolver conflicting installs больше не игнорируются.
- [x] Отсутствующий default adapter factory больше не создаёт silent empty registry; legacy path fail-fast.
- [x] Source-level regression защищает migrated factories и явно перечисляет оставшиеся store/search globals.
- [x] Existing production `RuntimeComposition` path не изменён.

#### Осталось

- [ ] Закрыть `COMP-001`: мигрировать `STORE_FACTORY` на explicit install/unavailable errors.
- [ ] Закрыть `COMP-001`: мигрировать Search normal/operation factories.
- [ ] Инвентаризировать production calls legacy `init_store`, search/projector globals и `RuntimeBuilder::new`.
- [ ] Закрыть `COMP-004`: добавить composition-aware `validate_changed`.
- [ ] Удалить legacy globals после parity coverage.

### Publication semantics

- [x] `PUB-001`: Adapter trust registry cleanup больше не превращает durable success в failure.
- [ ] `PUB-002`: инвентаризировать остальные staged-replace helpers и durability points.
- [ ] Перевести post-commit cleanup в warning/repair или typed committed-with-warning result.
- [ ] `PUB-003`: добавить failure-injection regressions для stage, publish, rollback и cleanup.

### CLI decomposition

- [x] `CLI-002`: Plugin command family вынесена в `direct_plugin_cli.rs` с executable parity regression.
- [ ] `CLI-001`: инвентаризировать remaining legacy commands и parser duplication.
- [ ] Ввести один root command model и dispatch table.
- [ ] Переносить command families bounded slices с executable parity regressions.
- [ ] Удалить `include!("main.rs")` после завершения migration.

### Documentation reconciliation

- [x] Adapter contract examples и затронутые current-HEAD status claims исправлены.
- [ ] `DOC-001`: проверить остальные документы с `status: verified` против current evidence.
- [ ] `DOC-002`: согласовать current/target/history sections в pipeline architecture.
- [ ] Не переносить historical verification metadata в current-HEAD claims.

## DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`: current adapter ownership исправлена, compatibility API cleanup и execution pending.

- [x] Contract trait/validator и unique owner registry.
- [x] 60 current public Athanor owners.
- [x] Adapter trust public report имеет отдельный current owner.
- [x] Daemon current ownership и legacy input classification.
- [x] MCP native protocol остаётся без synthetic Athanor schemas.
- [x] Shared CLI/daemon report types для Index, Context, Generation, Wiki и HTML.
- [ ] Legacy adapter trust report API cleanup.
- [ ] Targeted/workspace verification.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

- [x] Four-entry MCP standard protocol registry.
- [x] Four schema-less process protocols.
- [x] 30-entry general non-public registry.
- [x] Four-entry adapter non-public registry: 2 current + 2 legacy inputs.
- [x] Adapter migration/disjointness/source-observability enforcement.
- [ ] Repeat repository-wide schema search after compatibility cleanup.
- [ ] Не заявлять inventory complete до execution всех enforcement tests.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`: implementation и regressions complete, execution pending.

- [x] Shared Index/Context/Generation/Wiki/HTML types across active boundaries.
- [x] Direct Plugin CLI uses `VersionedAdapterTrustReport`.
- [x] Executable regressions cover shared reports and plugin trust migration.
- [ ] Execute parity regressions.

## Verification

### MCP

```bash
cargo fmt --all -- --check
cargo test -p athanor-transport-mcp --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo clippy -p athanor-transport-mcp --all-targets --locked -- -D warnings
```

### Adapter contracts and CLI

```bash
cargo test -p athanor-app adapter_contract --locked
cargo test -p athanor-app --test adapter_contract_inventory --locked
cargo test -p ath --test direct_plugin_cli --locked
```

### Legacy composition slice

```bash
cargo test -p athanor-app legacy_factory --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
cargo clippy -p athanor-app --all-targets --locked -- -D warnings
```

### JSON targeted

```bash
cargo test -p athanor-app --test daemon_transport_contracts --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test remaining_application_contracts --locked
cargo test -p athanor-app --test repair_contracts --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p ath --test executable_shared_report_cli --locked
```

### Workspace Definition of Done

```bash
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
```

Локальный Rust toolchain отсутствует. Hosted status для последних direct-to-main commits через доступный connector не отображается. Новые пакеты не помечаются `[x] verified` без execution evidence.

## Активный рабочий пакет

**Сейчас:** все найденные проблемы сведены в единый реестр. Первый explicit-composition slice реализован для adapter registry и projectors.

**Следом:** Store/Search factory migration, composition-aware `validate_changed`, затем безопасный repair большого `runtime.rs` и legacy trust API cleanup.

## Журнал

### 2026-07-17 — Consolidated defect registry

- Все подтверждённые дефекты, открытые риски, tool limitations и verification blockers получили отдельные ID.
- Реализованные исправления отделены от verified результатов.
- Store/Search composition, runtime compile defects, publication semantics, CLI monolith и docs reconciliation остаются явно открытыми.

### 2026-07-17 — ARCH-AUDIT-001 first composition slice

- Silent conflicting installs удалены для adapter registry/resolver и projectors.
- Empty-registry fallback заменён fail-fast behavior.
- Добавлены typed legacy factory errors и source-level migration regression.
- Store/search globals оставлены явно видимыми до следующего bounded slice.

### 2026-07-17 — ARCH-AUDIT-001 adapter contract migration

- Разделены current manifest, persisted trust registry и public trust report owners.
- Plugin CLI переведён на current public report; legacy inputs нормализуются.
- Current manifest examples обновлены; stale verification metadata снята.

### 2026-07-17 — ARCH-AUDIT-001 MCP hardening

- Bounded lifecycle, standard JSON-RPC semantics и initialization state реализованы.
- Execution remains pending.
