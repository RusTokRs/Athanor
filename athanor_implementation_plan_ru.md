# Athanor: консолидированный план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-17  
> Статус: active architecture audit

## Правила статусов

- `[x] verified` — реализация и regressions подтверждены фактически выполненными проверками.
- `[-] in progress` — полезный срез находится в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — работа не начата.
- `[!] blocked` — требуется compatibility decision или недоступная платформенная проверка.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id или protocol version. Эквивалентные CLI, daemon и MCP операции не должны иметь разные application payload shapes.

## Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | MCP, adapter current contracts и первый composition slice реализованы; execution/cleanup pending |
| `DS-JSON-001` | P1 | `[-] in progress` | Public registry 60; current adapter owners разделены, legacy library API остаётся |
| `DS-JSON-002` | P1 | `[-] in progress` | 30 general + 4 adapter-specific non-public descriptors; execution и repeat scan pending |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed CLI/daemon/MCP/plugin payload parity реализована; execution pending |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |

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
- [ ] Рассмотреть отдельный control-plane input path для cancellation notifications при полной saturation.

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

#### Осталось

- [ ] Deprecated/remove старые `runtime.rs` trust-report functions, которые ещё могут вернуть legacy shared schema.
- [ ] Выполнить adapter contract и executable CLI regressions.
- [ ] Повторить repository-wide schema scan после compatibility cleanup.

### Подтверждённый compile-level defect

- [!] Unix runtime test: один `ProcessCommand` задаёт `clear_environment` дважды.
- [!] Другой non-Windows helper не задаёт обязательный `clear_environment`.
- [ ] Исправить оба literals через гарантированную полную копию большого `runtime.rs`.
- [ ] Добавить source guard, исключающий возврат duplicate/missing field.

GitHub contents connector не поддерживает точечный patch большого файла. Небезопасная ручная замена 1800+ строк не выполняется без полной проверяемой копии исходного blob.

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

- [ ] Мигрировать `STORE_FACTORY` на explicit install/unavailable errors.
- [ ] Мигрировать search normal/operation factories.
- [ ] Инвентаризировать production calls legacy `init_store`, search/projector globals и `RuntimeBuilder::new`.
- [ ] Добавить composition-aware `validate_changed` и другие remaining entrypoints.
- [ ] Удалить legacy globals после parity coverage.

### Publication semantics

- [x] Adapter trust registry cleanup больше не превращает durable success в failure.
- [ ] Инвентаризировать остальные staged-replace helpers и durability points.
- [ ] Перевести post-commit cleanup в warning/repair или typed committed-with-warning result.
- [ ] Добавить failure-injection regressions для stage, publish, rollback и cleanup.

### CLI decomposition

- [x] Plugin command family вынесена в `direct_plugin_cli.rs` с executable parity regression.
- [ ] Инвентаризировать remaining legacy commands и parser duplication.
- [ ] Ввести один root command model и dispatch table.
- [ ] Переносить command families bounded slices с executable parity regressions.
- [ ] Удалить `include!("main.rs")` после завершения migration.

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

**Сейчас:** первый explicit-composition slice реализован для adapter registry и projectors; store/search globals и runtime compile literals остаются открыты.

**Следом:** store/search factory migration и безопасный repair большого `runtime.rs`, затем legacy trust API cleanup.

## Журнал

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
