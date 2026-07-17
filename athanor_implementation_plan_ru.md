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
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | MCP hardening и adapter current-path schema migration реализованы; execution/compatibility cleanup pending |
| `DS-JSON-001` | P1 | `[-] in progress` | Public registry вырос до 60; adapter current owners разделены, legacy library API остаётся |
| `DS-JSON-002` | P1 | `[-] in progress` | 30 general + 4 adapter-specific non-public descriptors; execution и повторный scan pending |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed CLI/daemon/MCP/plugin payload parity реализована; execution pending |
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
- [x] Завершённые request tasks извлекаются из `JoinSet` во время input loop.
- [x] stdout имеет одного async writer owner и сохраняет one-JSON-document-per-line framing.
- [x] EOF отменяет зарегистрированные reads, drains request tasks и joins response writer.
- [x] JSON-RPC request version проверяется на exact `2.0`.
- [x] Стандартные error categories разделены: `-32700`, `-32600`, `-32601`, `-32602`, `-32002`.
- [x] Omitted request id отделён от explicit `null` id.
- [x] MCP session проходит `AwaitingInitialize -> AwaitingInitialized -> Ready`.
- [x] Initialize protocol проверяется на `2024-11-05`.
- [x] Сохранены request-scoped cancellation, deadlines и drained Search/Context cleanup.
- [x] Старый `lifecycle.rs` удалён после переноса regressions в единый active server module.
- [x] Добавлены unit/in-memory stdio regressions и source-level bounded architecture guards.

#### Осталось

- [ ] Выполнить MCP unit/integration tests, fmt и Clippy.
- [ ] Проверить interoperability с реальным MCP client.
- [ ] Рассмотреть отдельный control-plane read path для cancellation notifications при полной saturation.

### Adapter JSON boundary migration

#### Реализовано для current paths

- [x] Current manifest получил canonical schema `athanor.adapter_manifest.v1`.
- [x] Legacy `athanor.adapter_manifest` принимается только как migration input и нормализуется в памяти.
- [x] Persisted trust registry получил отдельный schema owner `athanor.adapter_trust_registry.v2`.
- [x] Legacy persisted `athanor.adapter_trust.v2` читается и нормализуется перед current write.
- [x] Public trust response получил отдельный owner `VersionedAdapterTrustReport` / `athanor.adapter_trust_report.v1`.
- [x] Public registry вырос с 59 до 60 current owners.
- [x] Добавлен specialized `ADAPTER_NON_PUBLIC_JSON_CONTRACTS`: 2 current + 2 legacy inputs.
- [x] Добавлены fixture, lifecycle/disjointness/source-observability и migration regressions.
- [x] `ath plugins list/trust/untrust` переведены на focused dispatcher с current public schema.
- [x] Existing command grammar и human-readable output сохранены.
- [x] Trust-store write после legacy input выдаёт только current persisted schema.
- [x] Post-commit backup cleanup для trust registry стал best-effort warning, а не ложным failure после durable success.

#### Осталось

- [ ] Удалить или явно deprecated-mark старые `runtime.rs` trust-report functions, которые ещё могут вернуть legacy shared schema для library compatibility.
- [ ] Обновить оставшиеся adapter documentation examples на current manifest schema.
- [ ] Выполнить adapter contract и executable CLI regressions.
- [ ] Повторить repository-wide schema scan после compatibility cleanup.

### Подтверждённый compile-level defect

- [!] В Unix-only runtime test один `ProcessCommand` задаёт `clear_environment` дважды.
- [!] В другом non-Windows helper `ProcessCommand` не задаёт обязательный `clear_environment`.
- [ ] Исправить оба struct literals до первого Rust-enabled test run.

Этот дефект находится в большом legacy `runtime.rs`; его исправление объединяется с bounded runtime facade/decomposition slice, чтобы не переписывать 1800+ строк через contents API без дополнительной защиты.

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

Подтверждённая проблема: некоторые staged-replace helpers публикуют новый durable файл, а затем возвращают ошибку при cleanup backup.

- [x] Adapter trust registry cleanup переведён в best-effort warning после durable publish.
- [ ] Инвентаризировать остальные staged-replace helpers.
- [ ] Зафиксировать durability point каждого protocol.
- [ ] Перевести оставшиеся post-commit cleanup paths в warning/repair либо typed committed-with-warning result.
- [ ] Добавить failure-injection regressions для stage, publish, rollback и cleanup.

### CLI decomposition

Подтверждённая проблема: `entry.rs` перехватывает часть команд focused parsers, затем включает monolithic legacy `main.rs`.

- [x] Plugin command family вынесена в `direct_plugin_cli.rs` с executable parity regression.
- [ ] Инвентаризировать remaining legacy commands и parser duplication.
- [ ] Ввести один root command model и dispatch table.
- [ ] Переносить command families bounded slices с executable parity regressions.
- [ ] Удалить `include!("main.rs")` после завершения migration.

## DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`: current adapter ownership исправлена, compatibility API cleanup и execution pending.

### Foundation и current owners

- [x] App-layer `json_contract` facade и trait `VersionedJsonContract`.
- [x] Canonical schema-id validator и typed `JsonContractError`.
- [x] Registry отклоняет duplicate schema id и duplicate Rust owner.
- [x] Standard, schema-less и non-public protocol registries отделены от public Athanor registry.
- [x] 60 current public Athanor owners зарегистрированы.
- [x] Adapter trust public report имеет отдельный current owner.
- [x] Daemon current request/response/jobs ownership и legacy input classification.
- [x] MCP native registry не получает synthetic `athanor.*` schemas.
- [x] CLI/daemon common Index, Generation, Wiki, HTML и Context payloads используют shared report types.

### Осталось

- [ ] Legacy adapter trust report API cleanup.
- [ ] Полный targeted/workspace verification.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

- [x] Four-entry MCP standard protocol registry.
- [x] Four schema-less process protocols.
- [x] 30-entry general non-public registry.
- [x] Four-entry adapter non-public registry: 2 current + 2 legacy inputs.
- [x] Adapter fixture, migration, disjointness и source-observability enforcement.
- [ ] Повторить repository-wide schema search после compatibility cleanup.
- [ ] Не заявлять inventory complete до execution всех enforcement tests.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`: implementation и regressions complete, execution pending.

- [x] Direct CLI Index/Update и daemon index используют `IndexReport`.
- [x] Direct CLI Generate и daemon generation используют `GenerationReport`.
- [x] Direct Wiki/HTML JSON и daemon jobs используют `WikiReport`/`HtmlReport`.
- [x] Direct CLI и active MCP Context используют `ContextReport`.
- [x] Direct Plugin CLI использует `VersionedAdapterTrustReport`.
- [x] Executable regressions покрывают shared reports и plugin trust migration.
- [ ] Фактически выполнить executable parity regressions.

## Verification

### MCP package

```bash
cargo fmt --all -- --check
cargo test -p athanor-transport-mcp --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo clippy -p athanor-transport-mcp --all-targets --locked -- -D warnings
```

### Adapter contract package

```bash
cargo test -p athanor-app adapter_contract --locked
cargo test -p athanor-app --test adapter_contract_inventory --locked
cargo test -p ath --test direct_plugin_cli --locked
cargo clippy -p athanor-app --all-targets --locked -- -D warnings
cargo clippy -p ath --all-targets --locked -- -D warnings
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

**Сейчас:** adapter current-path schema migration реализована; compatibility cleanup и compile defect остаются открыты.

**Следом:** bounded runtime facade/explicit-composition inventory, включающий безопасное исправление legacy runtime test literals и deprecated trust API cleanup.

## Журнал

### 2026-07-17 — ARCH-AUDIT-001 adapter contract migration

- Разделены current manifest, persisted trust registry и public trust report schema owners.
- Legacy manifest/trust registry inputs нормализуются перед current use/write.
- Добавлены adapter-specific registry, fixture и migration regressions.
- Plugin CLI вынесен в focused dispatcher и возвращает current public report.
- Trust registry post-commit cleanup больше не превращает durable success в failure.
- Старый library report API и Rust-enabled execution остаются открытыми.

### 2026-07-17 — ARCH-AUDIT-001 MCP transport hardening

- Неограниченные response channel/request spawning заменены bounded lifecycle.
- JSON-RPC parsing и initialization state разделены по standard categories.
- Старый lifecycle source удалён; active implementation и regressions находятся в `server.rs`.

### 2026-07-17 — DS-JSON executable shared-report parity

- Добавлены direct JSON dispatchers и executable regression для Index, Context, Generation, Wiki и HTML.
- Execution не заявляется из-за отсутствия Rust toolchain и hosted checks.
