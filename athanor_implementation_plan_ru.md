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
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Repository-wide audit открыт; MCP P1 implementation завершена, execution pending |
| `DS-JSON-001` | P1 | `[-] in progress` | 59 public/current owners сохранены; adapter manifest/trust boundaries требуют миграции |
| `DS-JSON-002` | P1 | `[-] in progress` | Process/persistence inventory больше не считается полным до adapter boundary migration |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed CLI/daemon/MCP payload parity реализована; execution pending |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |

## ARCH-AUDIT-001 — Repository-wide architecture audit

**Статус:** `[-] in progress`.

Рабочий audit tracker: GitHub issue `#16`.

### Порядок аудита

1. MCP transport lifecycle и JSON-RPC correctness.
2. Adapter manifest/trust schema migration и восстановление JSON inventory.
3. Process-global factories и final explicit-composition migration.
4. Publication, rollback и post-commit cleanup semantics.
5. CLI command model и удаление legacy monolith/include path.
6. Store/backend conformance и transaction boundaries.
7. Adapter correctness, incremental invalidation и resource limits.
8. Security/operations hardening.
9. Rust-enabled workspace verification и reconciliation документации.

### MCP transport hardening

#### Реализовано

- [x] Активный stdio server больше не использует `mpsc::unbounded_channel`.
- [x] Response queue ограничена 32 serialized responses.
- [x] Ordinary request concurrency ограничена 32 in-flight tasks.
- [x] Завершённые request tasks извлекаются из `JoinSet` во время input loop, а не только после EOF.
- [x] stdout имеет одного async writer owner и сохраняет one-JSON-document-per-line framing.
- [x] EOF отменяет зарегистрированные reads, drains request tasks и joins response writer.
- [x] JSON-RPC request version проверяется на exact `2.0`.
- [x] Добавлены стандартные error categories: parse `-32700`, invalid request `-32600`, method not found `-32601`, invalid params `-32602`, server not initialized `-32002`.
- [x] Omitted request id отделён от explicit `null` id.
- [x] MCP session проходит `AwaitingInitialize -> AwaitingInitialized -> Ready`.
- [x] Initialize protocol проверяется на `2024-11-05`.
- [x] Сохранены request-scoped cancellation, deadlines и drained Search/Context cleanup.
- [x] Старый `lifecycle.rs` удалён после переноса regressions в единый active server module.
- [x] Добавлены unit/in-memory stdio regressions и source-level bounded architecture guards.

#### Осталось

- [ ] Выполнить `cargo test -p athanor-transport-mcp --locked`.
- [ ] Выполнить MCP contract integration test, fmt и Clippy.
- [ ] Исправить только фактически обнаруженные compile/fmt/Clippy замечания.
- [ ] После зелёного verification отметить MCP-подпакет `[x] verified` в issue `#16`.

### Adapter JSON boundary migration

Подтверждённые проблемы:

- `AdapterPluginManifest` использует неверсионированный `athanor.adapter_manifest`.
- `athanor.adapter_trust.v2` владеет двумя разными top-level shapes: persisted registry и public report.
- Эти boundaries отсутствуют в ранее объявленном complete inventory.

План:

- [ ] Ввести canonical versioned schema для current adapter manifest.
- [ ] Сохранить legacy unversioned manifest только как accepted migration input.
- [ ] Разделить persisted trust registry schema и public trust report schema.
- [ ] Добавить fixtures, lifecycle classification и source-observability enforcement.
- [ ] Обновить public/non-public counts после фактической регистрации owners.
- [ ] Выполнить targeted и workspace verification.

### Explicit composition migration

Подтверждённые проблемы:

- Store, search, projector и adapter factories сохраняют `OnceLock` global paths.
- Повторный `set` молча игнорируется.
- Legacy adapter registry может молча вернуть empty registry.

План:

- [ ] Инвентаризировать production call sites legacy installers/init functions.
- [ ] Сделать failed installation явной typed error.
- [ ] Удалить empty-registry fallback из production behavior.
- [ ] Перевести оставшиеся entrypoints на explicit `RuntimeComposition`.
- [ ] Удалить legacy globals после parity coverage.

### Publication semantics

Подтверждённая проблема: некоторые staged-replace helpers публикуют новый durable файл, а затем возвращают ошибку при cleanup backup. Это смешивает commit outcome и post-commit maintenance.

План:

- [ ] Инвентаризировать все staged-replace helpers.
- [ ] Зафиксировать durability point каждого protocol.
- [ ] Перевести post-commit cleanup в best-effort warning/repair path либо typed committed-with-warning result.
- [ ] Добавить failure-injection regressions для stage, publish, rollback и cleanup.

### CLI decomposition

Подтверждённая проблема: `entry.rs` перехватывает часть команд focused parsers, затем включает monolithic legacy `main.rs`. Это создаёт несколько источников истины для grammar/help/rendering/exit codes.

План:

- [ ] Инвентаризировать remaining legacy commands и parser duplication.
- [ ] Ввести один root command model и dispatch table.
- [ ] Переносить command families bounded slices с executable parity regressions.
- [ ] Удалить `include!("main.rs")` после завершения migration.

## DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`: application/daemon/repair owners реализованы, но repository-wide inventory переоткрыт после обнаружения adapter plugin/trust boundaries.

### Сохранённый foundation

- [x] App-layer `json_contract` facade и trait `VersionedJsonContract`.
- [x] Canonical schema-id validator и typed `JsonContractError`.
- [x] Registry отклоняет duplicate schema id и duplicate Rust owner.
- [x] Standard, schema-less и non-public protocol registries отделены от public Athanor registry.
- [x] 59 current public Athanor owners остаются зарегистрированными.
- [x] Daemon current request/response/jobs ownership и legacy input classification.
- [x] MCP native JSON-RPC/MCP registry не получает synthetic `athanor.*` schemas.
- [x] CLI/daemon common Index, Generation, Wiki, HTML и Context payloads используют shared report types.

### Переоткрытый scope

- [ ] Adapter manifest current/legacy ownership.
- [ ] Adapter trust persisted registry ownership.
- [ ] Adapter trust public report ownership.
- [ ] Обновлённые inventory counts и fixtures.
- [ ] Полный targeted/workspace verification.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

Ранее реализованы:

- [x] Four-entry MCP standard protocol registry.
- [x] Four schema-less process protocols.
- [x] 30-entry non-public registry для ранее audited persisted/generated/interchange/embedded boundaries.
- [x] Public/non-public disjointness и runtime source assertions.
- [x] Process framing и representative fixtures.

Осталось:

- [ ] Добавить adapter manifest/trust boundaries.
- [ ] Повторить repository-wide schema search после migration.
- [ ] Не заявлять inventory complete до execution всех enforcement tests.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`: implementation и regressions complete, execution pending.

- [x] Direct CLI Index/Update и daemon index используют `IndexReport`.
- [x] Direct CLI Generate и daemon generation используют `GenerationReport`.
- [x] Direct Wiki/HTML JSON и daemon jobs используют `WikiReport`/`HtmlReport`.
- [x] Direct CLI и active MCP Context используют `ContextReport`.
- [x] Executable regression покрывает Index/Context/Generation/Wiki/HTML schemas и shared snapshot.
- [ ] Фактически выполнить executable parity regression.

## Verification

### MCP active package

```bash
cargo fmt --all -- --check
cargo test -p athanor-transport-mcp --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo clippy -p athanor-transport-mcp --all-targets --locked -- -D warnings
```

### JSON targeted package

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

Локальный Rust toolchain в текущей среде отсутствует. Hosted status для последних direct-to-main commits через доступный connector не отображается. Поэтому ни один новый пакет не помечается `[x] verified` без внешнего execution evidence.

## Активный рабочий пакет

**Сейчас:** MCP transport hardening реализован в source и ожидает Rust-enabled verification.

**Следом:** adapter manifest/trust schema migration, затем повторный repository-wide JSON inventory scan.

## Журнал

### 2026-07-17 — ARCH-AUDIT-001 MCP transport hardening

- Открыт repository-wide architecture audit issue `#16`.
- Неограниченные response channel/request spawning заменены bounded lifecycle.
- JSON-RPC parsing отделяет parse error, invalid request, method not found и invalid params.
- Добавлена MCP initialization state machine и protocol negotiation.
- Omitted id отделён от explicit null.
- Старый lifecycle source удалён; active implementation и regressions находятся в `server.rs`.
- Документация переведена из stale verified claim в draft/current-HEAD execution-pending status.

### 2026-07-17 — DS-JSON executable shared-report parity

- Добавлен direct dispatcher для Generate, Wiki и Report Html с additive `--json`.
- Добавлен executable regression для Index, Context, Generation, Wiki и HTML.
- Execution не заявляется из-за отсутствия Rust toolchain и hosted checks.

### 2026-07-17 — DS-JSON process/persistence inventory

- Добавлены public, non-public, MCP и process registries/fixtures для ранее audited scope.
- После repository-wide architecture audit completion claim переоткрыт из-за пропущенных adapter plugin/trust boundaries.
