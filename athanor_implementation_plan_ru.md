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

Основные монолитные migration-пакеты завершены на уровне реализации. В реестре дефектов:

| Категория | Количество | Смысл |
| --- | ---: | --- |
| Implemented | 36 | Код и source/behavior regressions находятся в `main`; execution часто pending |
| In progress | 3 | Legacy API removal, schema enforcement и documentation claims |
| Planned | 2 | Transactional MCP cancellation и pipeline reconciliation |
| Blocked | 1 | Полный Rust verification недоступен в текущей среде |

Практически осталось **5 рабочих пакетов**:

1. `COMP-003C2` — удалить временные installer/no-composition shims и завершить parallel composition isolation matrix.
2. `MCP-007` — определить transactional Index cancellation до и после durable commit point.
3. `JSON-003` — повторить repository-wide schema scan и выполнить enforcement matrix.
4. `DOC-001/002` — убрать stale verification claims и согласовать pipeline current/target/history.
5. `VERIFY-001` — выполнить fmt/test/Clippy/smoke matrix и перевести подтверждённые пункты в `verified`.

`COMP-003A/B1/B2/C1` завершены на уровне implementation. Active entrypoints composition-only, process-global runtime state удалён физически, а test compatibility использует локальную composition. Оставшийся долг — breaking cleanup временных API shims и execution evidence.

## 3. Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | MCP lifecycle/composition, adapter contracts, publication, JSONL store, CLI root, Runtime, Docs, Store и Search owners декомпозированы; API cleanup и execution pending |
| `DS-JSON-001` | P1 | `[x] implemented` | Public registry 60; manifest, trust registry и public trust report имеют разные current owners; execution pending |
| `DS-JSON-002` | P1 | `[-] in progress` | 30 general + 4 adapter-specific non-public descriptors; repeat scan и execution pending |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed CLI/daemon/MCP/plugin payload parity реализована; execution pending |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |

## 4. Реестр подтверждённых дефектов и рисков

| ID | Priority | Status | Подтверждённая проблема | Критерий закрытия |
| --- | --- | --- | --- | --- |
| `MCP-001` | P1 | `[x] implemented` | Response path использовал unbounded channel | Bounded queue, single writer и выполненные MCP regressions |
| `MCP-002` | P1 | `[x] implemented` | Requests spawn-ились без лимита, completed tasks жили до EOF | In-flight cap, continuous reap и выполненные lifecycle tests |
| `MCP-003` | P1 | `[x] implemented` | Неверные JSON-RPC version/error/id/session semantics | Exact `2.0`, standard codes, distinct omitted/null id и session tests |
| `MCP-004` | P2 | `[x] implemented` | Saturation ordinary requests останавливала cancellation/control-plane input | Stdin читается независимо от request slots; notifications bypass saturation; overload возвращает retryable `-32001` |
| `MCP-005` | P1 | `[x] implemented` | MCP runtime использовал legacy dispatcher и process-global installation | Explicit `RuntimeComposition` через CLI/server/request/tool lifecycle |
| `MCP-006` | P1 | `[x] implemented` | MCP `server.rs` был монолитом примерно в 1000 строк | Старый файл удалён; lifecycle/operation/protocol/types/tests разделены и bounded |
| `MCP-007` | P1 | `[ ] planned` | Cancellation transactional Index не различает pre-commit rollback и post-commit durable success | Cancelability contract, непрерываемый post-commit cleanup и pre/post-commit regressions |
| `JSON-001` | P1 | `[x] implemented` | Current adapter manifest имел неверсионированный schema id | `athanor.adapter_manifest.v1`; legacy id только migration input |
| `JSON-002` | P1 | `[x] implemented` | `athanor.adapter_trust.v2` владел persisted registry и public report | Runtime emitters возвращают только `athanor.adapter_trust_report.v1`; persisted owner — `athanor.adapter_trust_registry.v2`; execution pending |
| `JSON-003` | P1 | `[-] in progress` | Inventory считался complete без adapter boundaries | Adapter registry/fixtures добавлены; repeat scan и enforcement execution остаются |
| `JSON-004` | P2 | `[x] implemented` | Adapter contract symbols re-exported двумя facade paths | Один canonical re-export path и Clippy без ambiguous re-export warnings |
| `RUST-001` | P1 | `[x] implemented` | Unix test `ProcessCommand` literal дважды задавал `clear_environment` | Fixture исправлен; source regression требует ровно одно поле; Rust execution pending |
| `RUST-002` | P1 | `[x] implemented` | Non-Windows helper не задавал обязательный `clear_environment` | Helper исправлен; source regression требует ровно одно поле; Rust execution pending |
| `RUNTIME-001` | P1 | `[x] implemented` | `runtime.rs` был 1846-line owner | Conventional bounded modules; tests разделены; no include; execution pending |
| `COMP-001` | P2 | `[x] implemented` | `OnceLock::set` conflicts молча игнорировались | Process-global runtime storage удалён; execution pending |
| `COMP-002` | P1 | `[x] implemented` | Отсутствующий adapter factory создавал empty registry | Explicit registry/composition; no hidden resolver fallback |
| `COMP-003` | P2 | `[-] in progress` | Store/Search/adapter/projector globals создавали hidden dependencies | `A/B1/B2/C1` implemented; final shim removal и isolation execution остаются |
| `COMP-003A` | P2 | `[x] implemented` | Compatibility perimeter не был зафиксирован | Caller inventory, source perimeter, удалённый Store bridge и private lookup helpers |
| `COMP-003B1` | P2 | `[x] implemented` | Adapter/projector globals компилировались в default build | Feature/test-only quarantine и CI/source enforcement |
| `COMP-003B2` | P2 | `[x] implemented` | Store/Search globals находились в implementation owners | Bounded Store/Search owners и isolated compatibility storage |
| `COMP-003C1` | P2 | `[x] implemented` | Feature/test-only globals и legacy factory errors сохраняли process state surface | `legacy-global-runtime`, all `OnceLock` owners, legacy errors и test installation удалены; execution pending |
| `COMP-003C2` | P2 | `[-] in progress` | State-free installer и no-composition shims сохраняют public compatibility surface | Breaking-change record, удаление shims/install API и full parallel isolation matrix |
| `COMP-004` | P2 | `[x] implemented` | `validate_changed` использовал hidden global adapter composition | Composition-aware path; production legacy wrapper fail-fast; execution pending |
| `COMP-005` | P2 | `[x] implemented` | Legacy trust functions возвращали report со старым shared schema | Runtime trust API возвращает versioned public report; execution pending |
| `COMP-006` | P2 | `[x] implemented` | Focused handlers устанавливали process-global runtime | Все active CLI families, включая MCP, используют explicit composition |
| `COMP-007` | P2 | `[x] implemented` | RusTok operations использовали global store/context APIs | Composition-aware audit/graph/context family и focused CLI в `main` |
| `COMP-008` | P2 | `[x] implemented` | API/Docs/Repair facades использовали task-local store composition | Direct paths используют `composition.init_store`; service bridge удалён |
| `PUB-001` | P1 | `[x] implemented` | Adapter trust writer мог вернуть failure после durable rename | Cleanup best effort после commit point |
| `PUB-002` | P1 | `[x] implemented` | Staged-replace helpers смешивали durable success и maintenance failure | Explicit commit point и non-fatal maintenance path |
| `PUB-003` | P2 | `[x] implemented` | Не было системных failure-injection regressions | Stage/build, rollback, cleanup, Drop, target-race и recovery matrix в `main` |
| `PUB-004` | P1 | `[x] implemented` | App finalizers возвращали cleanup failure после commit | Owners логируют warning и сохраняют successful publication |
| `PUB-005` | P1 | `[x] implemented` | JSONL latest/sequence имели duplicate replacement helpers | Один pointer-publication owner, invalid target fail-closed |
| `STORE-001` | P1 | `[x] implemented` | JSONL store состоял из compatibility includes и файлов до 1286 строк | Conventional bounded modules, public API preserved |
| `CLI-001` | P2 | `[x] implemented` | CLI root включал 4k-line monolith | Legacy root/include удалены, typed bounded command families |
| `CLI-002` | P2 | `[x] implemented` | Plugin commands использовали legacy trust-report schema | Focused dispatcher и versioned executable contract |
| `CLI-003` | P2 | `[x] implemented` | Read/Application Report/Repair использовали include wrappers | Explicit model/run/render modules и direct composition |
| `CLI-004` | P2 | `[x] implemented` | Graph/RusTok/Check зависели от legacy renderer bridges | Dedicated renderers и typed root command model |
| `CLI-005` | P2 | `[x] implemented` | Legacy-only grammars оставались в monolith | Dedicated families, root help/version/unknown behavior сохранены |
| `DOC-001` | P3 | `[-] in progress` | Документы содержат stale `verified` claims и удалённые source paths | Claims имеют current-HEAD evidence либо честный `implemented/pending` status |
| `DOC-002` | P3 | `[ ] planned` | Pipeline architecture содержит противоречия current/planned | Current, target и history sections согласованы |
| `DOC-003` | P2 | `[x] implemented` | Docs engine находился в 3282-line compatibility owner | Legacy file удалён; bounded conventional modules; execution pending |
| `VERIFY-001` | P1 | `[!] blocked` | В доступной среде нет Rust toolchain; hosted evidence отсутствует | fmt/tests/Clippy/workspace smoke выполнены и записаны на том же commit |
| `TOOL-001` | P2 | `[x] implemented` | Connector не предоставляет точечный patch больших Rust files | Runtime, Docs, Store и Search owners мигрированы атомарными Git trees |
| `TOOL-002` | P2 | `[x] implemented` | Contents connector не позволял безопасно менять CLI monolith | Команды перенесены полностью, старый monolith удалён |

## 5. Реализованные архитектурные срезы

### Explicit runtime composition

- [x] Active CLI/MCP/API/Docs/Repair/Search/Generation/Index paths используют explicit composition.
- [x] `legacy-global-runtime` удалён из обоих Cargo crates и CI.
- [x] Adapter, Projector, Store и Search global owner files удалены.
- [x] `LegacyFactoryInstallError` и `LegacyFactoryUnavailableError` удалены.
- [x] Unit tests создают локальную `test_runtime::composition()` без installation.
- [x] `RuntimeBuilder::new` не использует hidden production registry/resolver.
- [x] Store, Search, Projection и changed-validation production legacy paths fail fast.

### Runtime, Docs, Store и Search decomposition

- [x] Runtime, Docs, Store и Search разделены на bounded owners.
- [x] 3282-line Docs legacy owner удалён.
- [x] Store trait implementations разделены на knowledge/publication/canonical.
- [x] Search разделён на orchestration/contracts/index lifecycle.
- [x] Source inventories ограничивают module sizes и запрещают global state.

### MCP, publication и CLI

- [x] Bounded MCP response/request lifecycle и correct JSON-RPC semantics.
- [x] Publication commit-point и best-effort post-commit cleanup.
- [x] JSONL store/pointer writers декомпозированы.
- [x] CLI 4k-line monolith удалён.

## 6. Verification

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test -p athanor-app runtime --locked
cargo test -p athanor-app docs --locked
cargo test -p athanor-app adapter_contract --locked
cargo test -p athanor-app --test runtime_modularity_inventory --locked
cargo test -p athanor-app --test service_composition_inventory --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
cargo test -p athanor-app --test adapter_contract_inventory --locked
cargo test -p athanor-app --test mcp_runtime_bootstrap_inventory --locked
cargo test -p athanor-transport-mcp --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
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
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo run -p ath --quiet --locked -- --help
cargo run -p ath --quiet --locked -- index .
```

До выполнения этого набора новые пункты сохраняют статус `implemented`, а не `verified`.

## 7. Активный рабочий пакет

**Сейчас:** `COMP-003C2` — удалить временные state-free compatibility shims и завершить breaking migration на composition-only public API.

Требования:

- записать breaking-change surface для external embedders;
- удалить `athanor_runtime_defaults::install()` и все `install_*` symbols;
- удалить production no-composition Index/Search/Generation/Wiki и service wrappers;
- обновить stable re-export modules на composition-only signatures;
- добавить parallel isolation tests с различными Store/Search/Projector factories;
- выполнить targeted/default/all-features tests и Clippy.

**После него:** `MCP-007`, затем `JSON-003`, documentation reconciliation и полный verification.

## 8. Журнал актуализаций

### 2026-07-17 — Process-global runtime removal

- Реализован `COMP-003C1`.
- Удалён `legacy-global-runtime` из Cargo и CI.
- Физически удалены adapter/projector/Store/Search global owner files.
- Удалены legacy factory error types и test installation bootstrap.
- Unit-test compatibility wrappers используют fresh local composition.
- Production no-composition Store/Search/Projection/Validate Changed paths fail fast.
- Installer symbols временно сохранены только как state-free panic shims до `COMP-003C2`.
- Статус — implemented, execution pending.

### 2026-07-17 — Store and Search feature quarantine

- Реализован `COMP-003B2`.
- Store и Search декомпозированы; globals были изолированы перед физическим удалением.

### 2026-07-17 — Adapter and projector feature quarantine

- Реализован `COMP-003B1`.
- Adapter и projector globals были изолированы за opt-in feature перед удалением.

### 2026-07-17 — Legacy runtime compatibility perimeter

- Завершён `COMP-003A` caller inventory.
- Удалены task-local Store bridge и public factory-introspection helpers.
