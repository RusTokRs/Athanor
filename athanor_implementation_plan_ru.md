# Athanor: консолидированный план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-17  
> Статус: active architecture audit

## 1. Правила статусов

- `[x] implemented` — изменение находится в `main`, но не считается проверенным без execution evidence.
- `[x] verified` — реализация и regressions подтверждены выполненными проверками на соответствующем commit.
- `[-] in progress` — полезные срезы находятся в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — подтверждённая работа ещё не начата.
- `[!] blocked` — безопасное изменение или проверка временно недоступны.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id или protocol version. CLI, daemon и MCP варианты одной операции должны сохранять эквивалентные application payloads.

## 2. Состояние программы

Архитектурные миграции находятся в финальной трети.

| Категория | Количество | Смысл |
| --- | ---: | --- |
| Implemented | 26 | Код и source regressions находятся в `main`; execution часто pending |
| In progress | 5 | Остались ограниченные срезы или compatibility cleanup |
| Planned | 2 | Документационная декомпозиция и reconciliation |
| Blocked | 5 | Большой `runtime.rs`, Rust toolchain и connector limitations |

Практически осталось **7 рабочих пакетов**:

1. `RUNTIME-001` — декомпозиция `runtime.rs`, исправление `RUST-001/002`, закрытие `COMP-005/JSON-002`.
2. `COMP-003` — удаление оставшихся process-global compatibility factories после parity coverage.
3. `MCP-007` — решение transactional Index cancellation semantics.
4. `DOC-003` — декомпозиция большого Docs compatibility owner без textual include.
5. `JSON-003` — повторный repository-wide schema scan и enforcement execution.
6. `DOC-001/002` — reconciliation stale claims и pipeline current/target/history.
7. `VERIFY-001` — полный fmt/test/Clippy/smoke matrix и перевод подтверждённых пунктов в `verified`.

Из них крупной новой архитектурной разработки требуют в основном пункты 1–4. Остальное — cleanup, documentation и execution evidence.

## 3. Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | MCP lifecycle/composition, adapter contracts, publication hardening, JSONL store, bounded CLI root и direct Docs composition реализованы; runtime cleanup и execution pending |
| `DS-JSON-001` | P1 | `[-] in progress` | Public registry 60; current adapter owners разделены, legacy runtime emitters остаются |
| `DS-JSON-002` | P1 | `[-] in progress` | 30 general + 4 adapter-specific non-public descriptors; execution и repeat scan pending |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed CLI/daemon/MCP/plugin payload parity реализована; execution pending |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |

## 4. Реестр подтверждённых дефектов и рисков

| ID | Priority | Status | Подтверждённая проблема | Критерий закрытия |
| --- | --- | --- | --- | --- |
| `MCP-001` | P1 | `[x] implemented` | Response path использовал unbounded channel | Bounded queue, single writer и выполненные MCP regressions |
| `MCP-002` | P1 | `[x] implemented` | Requests spawn-ились без лимита, completed tasks жили до EOF | In-flight cap, continuous reap и выполненные lifecycle tests |
| `MCP-003` | P1 | `[x] implemented` | Неверные JSON-RPC version/error/id/session semantics | Exact `2.0`, standard codes, distinct omitted/null id и session tests |
| `MCP-004` | P2 | `[x] implemented` | Полная saturation ordinary requests останавливала чтение cancellation/control-plane input | Stdin читается независимо от request slots; notifications bypass saturation; overloaded requests получают retryable `-32001`; execution pending |
| `MCP-005` | P1 | `[x] implemented` | MCP runtime использовал legacy dispatcher и process-global installation | Explicit `RuntimeComposition` через CLI/server/request/tool lifecycle; legacy dispatcher/include/install удалены |
| `MCP-006` | P2 | `[x] implemented` | MCP `server.rs` был монолитом примерно в 1000 строк | Старый файл удалён; lifecycle/operation/protocol/types/tests разделены; production modules bounded; execution pending |
| `MCP-007` | P1 | `[ ] planned` | Cancellation notification для transactional Index требует согласования rollback и durable-success semantics | Определён cancelability contract, cleanup не прерывается после commit point, regressions покрывают pre/post-commit cancellation |
| `JSON-001` | P1 | `[x] implemented` | Current adapter manifest имел неверсионированный schema id | `athanor.adapter_manifest.v1`; legacy id только migration input |
| `JSON-002` | P1 | `[-] in progress` | `athanor.adapter_trust.v2` владел persisted registry и public report | Current owners разделены; legacy runtime emitters удалены либо version-limited |
| `JSON-003` | P1 | `[-] in progress` | Inventory считался complete без adapter boundaries | Adapter registry/fixtures добавлены; repeat scan и enforcement execution завершены |
| `JSON-004` | P2 | `[x] implemented` | Adapter contract symbols re-exported двумя facade paths | Один canonical re-export path и Clippy без ambiguous re-export warnings |
| `RUST-001` | P1 | `[!] blocked` | Unix test `ProcessCommand` literal дважды задаёт `clear_environment` | Исправлено внутри `RUNTIME-001`, source guard и Rust compile |
| `RUST-002` | P1 | `[!] blocked` | Non-Windows helper не задаёт обязательный `clear_environment` | Исправлено внутри `RUNTIME-001`, source guard и Rust compile |
| `RUNTIME-001` | P1 | `[-] in progress` | `runtime.rs` остаётся крупным owner для registry, trust API, builder и process tests; точечные дефекты нельзя безопасно лечить косметически | Conventional bounded modules, без textual include; `RUST-001/002` и `COMP-005` закрыты; public compatibility явно deprecated/versioned; tests/Clippy выполнены |
| `COMP-001` | P2 | `[x] implemented` | `OnceLock::set` conflicts молча игнорировались | Adapter/projector/Store/Search guarded APIs находятся в `main`; execution pending |
| `COMP-002` | P1 | `[x] implemented` | Отсутствующий adapter factory молча создавал empty registry | Legacy path fail-fast; explicit composition — штатный path |
| `COMP-003` | P2 | `[-] in progress` | Store/Search/adapter/projector globals сохраняют process-wide hidden dependencies | Legacy globals удалены после parity и compatibility window |
| `COMP-004` | P2 | `[x] implemented` | `validate_changed` использовал hidden global adapter composition | Composition-aware API, focused CLI и executable regression в `main`; execution pending |
| `COMP-005` | P2 | `[!] blocked` | Legacy trust functions в `runtime.rs` возвращают report со старым shared schema | Закрывается `RUNTIME-001`: API removed/deprecated либо возвращает versioned public report |
| `COMP-006` | P2 | `[x] implemented` | Focused handlers устанавливали process-global runtime | Все active CLI families, включая MCP, используют explicit composition; execution pending |
| `COMP-007` | P2 | `[x] implemented` | RusTok operations использовали global store/context APIs | Composition-aware audit/graph/context family и focused CLI в `main`; execution pending |
| `COMP-008` | P2 | `[x] implemented` | API/Docs/Repair facades использовали task-local store composition | API/Docs/Repair direct paths используют `composition.init_store`; последний service bridge удалён |
| `PUB-001` | P1 | `[x] implemented` | Adapter trust writer мог вернуть failure после durable rename из-за cleanup | Cleanup best-effort warning после commit point |
| `PUB-002` | P1 | `[x] implemented` | Staged-replace helpers смешивали durable success и maintenance failure | Writers имеют explicit commit point и non-fatal maintenance path; execution pending |
| `PUB-003` | P2 | `[x] implemented` | Не было системных failure-injection regressions | Stage/build, rollback, cleanup, Drop, target-race и recovery matrix в `main`; execution pending |
| `PUB-004` | P1 | `[x] implemented` | App finalizers возвращали backup cleanup failure после commit point | Owners логируют warning и сохраняют successful publication; execution pending |
| `PUB-005` | P1 | `[x] implemented` | JSONL latest/sequence имели duplicate replacement helpers | Один pointer-publication owner, invalid target fail-closed; execution pending |
| `STORE-001` | P1 | `[x] implemented` | JSONL store состоял из compatibility includes и файлов до 1286 строк | Conventional bounded modules, public API preserved; execution pending |
| `CLI-001` | P2 | `[x] implemented` | CLI root включал 4k-line monolith | Legacy `main.rs`/include удалены, typed bounded command families; execution pending |
| `CLI-002` | P2 | `[x] implemented` | Plugin commands использовали legacy trust-report schema | Focused dispatcher и versioned executable contract |
| `CLI-003` | P2 | `[x] implemented` | Read/Application Report/Repair использовали include wrappers и shadowing | Explicit model/run/render modules и direct composition; execution pending |
| `CLI-004` | P2 | `[x] implemented` | Graph/RusTok/Check зависели от legacy renderer bridges | Dedicated renderers и typed root command model; execution pending |
| `CLI-005` | P2 | `[x] implemented` | Legacy-only grammars оставались в удаляемом monolith | Dedicated families, root help/version/unknown behavior сохранены; execution pending |
| `DOC-001` | P3 | `[-] in progress` | Документы содержат stale `verified` claims и удалённые source paths | Все claims имеют current-HEAD evidence либо честный `implemented/pending` status |
| `DOC-002` | P3 | `[ ] planned` | Pipeline architecture содержит противоречия current/planned | Current, target и history sections согласованы |
| `DOC-003` | P2 | `[ ] planned` | Docs engine остаётся в 3k-line compatibility owner через textual include | Shared docs builders/helpers разделены на conventional bounded modules, include удалён без JSON/API drift |
| `VERIFY-001` | P1 | `[!] blocked` | В доступной среде нет Rust toolchain; hosted evidence неполно | Выполнены fmt/tests/Clippy/workspace smoke и результаты записаны на том же commit |
| `TOOL-001` | P2 | `[!] blocked` | Connector не предоставляет точечный patch большого `runtime.rs` | Ограничение обходится настоящей модульной миграцией через атомарный Git tree, не compatibility include |
| `TOOL-002` | P2 | `[x] implemented` | Contents connector не позволял безопасно менять CLI monolith | Команды перенесены полностью, старый monolith удалён |

## 5. Реализованные архитектурные срезы

### MCP transport

- [x] Bounded response queue и request concurrency.
- [x] Continuous task reap, EOF drain и one-writer stdout.
- [x] Correct JSON-RPC/MCP session semantics.
- [x] Explicit runtime composition end to end.
- [x] `server.rs` удалён; production logic разделена на `server/{lifecycle,operation,protocol,types}.rs`.
- [x] Cancellation notifications обрабатываются даже при заполненном ordinary request limit.
- [x] Overloaded ordinary requests получают bounded retryable server-busy error.
- [x] Unit/source regressions фиксируют saturation behavior и module size limits.

Осталось: выполнить MCP test matrix и закрыть `MCP-007`.

### Explicit composition

- [x] Guarded legacy factories с typed install/unavailable errors.
- [x] Active CLI, MCP, RusTok, Graph, ValidateChanged, API, Docs и Repair используют explicit composition.
- [x] API/Docs/Repair direct services не используют task-local/global Store lookup.
- [x] Executable regression для focused `validate-changed` добавлен.

Осталось: `RUNTIME-001`, затем удаление legacy globals (`COMP-003`).

### Publication и storage

- [x] Commit-point semantics и best-effort post-commit cleanup.
- [x] Failure-injection/source matrices добавлены.
- [x] JSONL store и pointer writers декомпозированы.

Осталось: execution всех conformance/failure tests.

### CLI

- [x] 4k-line root monolith удалён.
- [x] Command grammar, run и render owners разделены.
- [x] MCP CLI является composition root, а не global bootstrap.

Осталось: executable parity execution.

## 6. Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-transport-mcp --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo test -p athanor-app --test mcp_runtime_bootstrap_inventory --locked
cargo test -p athanor-app --test service_composition_inventory --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
cargo test -p athanor-app adapter_contract --locked
cargo test -p athanor-app --test adapter_contract_inventory --locked
cargo test -p ath --test direct_validate_changed_cli --locked
cargo test -p ath --test direct_plugin_cli --locked
cargo test -p ath --test direct_graph_cli --locked
cargo test -p athanor-app --test publication_semantics_inventory --locked
cargo test -p athanor-app --test publication_recovery_matrix --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p ath --test executable_shared_report_cli --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- --help
cargo run -p ath --quiet --locked -- index .
```

До выполнения этого набора новые пункты сохраняют статус `implemented`, а не `verified`.

## 7. Активный рабочий пакет

**Сейчас:** `RUNTIME-001` — разнести большой `crates/athanor-app/src/runtime.rs` на conventional bounded owners. Это не косметическая правка: миграция должна одновременно закрыть `RUST-001`, `RUST-002`, `COMP-005` и оставшуюся часть `JSON-002`.

Целевая структура:

```text
crates/athanor-app/src/runtime/
  mod.rs
  registry.rs
  builder.rs
  trust.rs
  process.rs
  process_tests.rs
```

Требования:

- без `include!` compatibility слоя;
- public API либо сохранён через явный facade, либо deprecated с documented migration;
- current public trust report всегда `athanor.adapter_trust_report.v1`;
- persisted trust registry всегда `athanor.adapter_trust_registry.v2`;
- `clear_environment` задаётся ровно один раз во всех platform helpers;
- source regression ограничивает размеры production modules;
- после migration выполняются targeted runtime/adapter tests и Clippy.

**После него:** `COMP-003`, затем `MCP-007`, `DOC-003` и verification/reconciliation.

## 8. Журнал актуализаций

### 2026-07-17 — MCP server decomposition and saturation control

- Добавлен executable regression для focused `validate-changed`.
- Удалён 1000-line `crates/athanor-transport-mcp/src/server.rs`.
- Lifecycle, protocol/session, operations/cancellation и shared types разделены на bounded modules.
- Main loop продолжает читать stdin независимо от количества обычных request tasks.
- Cancellation notifications bypass request saturation.
- Overloaded ordinary requests получают JSON-RPC `-32001` с `retryable: true` и фактическим limit.
- `MCP-004` и `MCP-006` отмечены `[x] implemented`; execution pending.
- Следующий крупный пакет оформлен как `RUNTIME-001`, поскольку большие файлы должны рефакториться, а не лататься.

### 2026-07-17 — Direct Docs composition

- Docs check/drift/propose/apply используют direct composition.
- `COMP-008` отмечен `[x] implemented`.
- Compatibility Docs owner вынесен в отдельный долг `DOC-003`.

### 2026-07-17 — MCP explicit composition

- `RuntimeComposition` проходит через MCP server/request/tool lifecycle.
- Legacy dispatcher/include/global install удалены.
- `MCP-005` и `COMP-006` отмечены `[x] implemented`.

### 2026-07-17 — CLI, storage and publication migration

- CLI monolith удалён, command families bounded.
- JSONL store переведён на conventional modules.
- Publication commit-point и post-commit cleanup semantics исправлены.
