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
| Implemented | 33 | Код и source/behavior regressions находятся в `main`; execution часто pending |
| In progress | 3 | Legacy globals, schema enforcement и documentation claims |
| Planned | 2 | Transactional MCP cancellation и pipeline reconciliation |
| Blocked | 1 | Полный Rust verification недоступен в текущей среде |

Практически осталось **5 рабочих пакетов**:

1. `COMP-003B/C` — исключить compatibility globals из default production build, затем удалить no-composition bootstrap APIs.
2. `MCP-007` — определить transactional Index cancellation до и после durable commit point.
3. `JSON-003` — повторить repository-wide schema scan и выполнить enforcement matrix.
4. `DOC-001/002` — убрать stale verification claims и согласовать pipeline current/target/history.
5. `VERIFY-001` — выполнить fmt/test/Clippy/smoke matrix и перевести подтверждённые пункты в `verified`.

`COMP-003A` завершён: caller inventory зафиксирован, active entrypoints подтверждены composition-only, неиспользуемый task-local Store bridge и public factory-introspection helpers удалены. Крупная новая архитектурная работа теперь сосредоточена в feature quarantine и `MCP-007`.

## 3. Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | MCP lifecycle/composition, adapter contracts, publication, JSONL store, CLI root, Runtime и Docs engine декомпозированы; legacy cleanup и execution pending |
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
| `MCP-006` | P2 | `[x] implemented` | MCP `server.rs` был монолитом примерно в 1000 строк | Старый файл удалён; lifecycle/operation/protocol/types/tests разделены и bounded |
| `MCP-007` | P1 | `[ ] planned` | Cancellation transactional Index не различает pre-commit rollback и post-commit durable success | Cancelability contract, непрерываемый post-commit cleanup и pre/post-commit regressions |
| `JSON-001` | P1 | `[x] implemented` | Current adapter manifest имел неверсионированный schema id | `athanor.adapter_manifest.v1`; legacy id только migration input |
| `JSON-002` | P1 | `[x] implemented` | `athanor.adapter_trust.v2` владел persisted registry и public report | Runtime emitters возвращают только `athanor.adapter_trust_report.v1`; persisted owner — `athanor.adapter_trust_registry.v2`; execution pending |
| `JSON-003` | P1 | `[-] in progress` | Inventory считался complete без adapter boundaries | Adapter registry/fixtures добавлены; repeat scan и enforcement execution завершены |
| `JSON-004` | P2 | `[x] implemented` | Adapter contract symbols re-exported двумя facade paths | Один canonical re-export path и Clippy без ambiguous re-export warnings |
| `RUST-001` | P1 | `[x] implemented` | Unix test `ProcessCommand` literal дважды задавал `clear_environment` | Fixture исправлен; source regression требует ровно одно поле; Rust execution pending |
| `RUST-002` | P1 | `[x] implemented` | Non-Windows helper не задавал обязательный `clear_environment` | Helper исправлен; source regression требует ровно одно поле; Rust execution pending |
| `RUNTIME-001` | P1 | `[x] implemented` | `runtime.rs` был 1846-line owner для models, registry, builder, trust, process scope и tests | 69-line root и conventional bounded modules; tests разделены; no include; execution pending |
| `COMP-001` | P2 | `[x] implemented` | `OnceLock::set` conflicts молча игнорировались | Adapter/projector/Store/Search guarded APIs в `main`; execution pending |
| `COMP-002` | P1 | `[x] implemented` | Отсутствующий adapter factory создавал empty registry | Legacy path fail-fast; explicit composition — штатный path |
| `COMP-003` | P2 | `[-] in progress` | Store/Search/adapter/projector globals сохраняют process-wide hidden dependencies | `COMP-003A` завершён: perimeter documented, task-local Store bridge/introspection удалены; далее default-build feature quarantine, parallel isolation и удаление installers |
| `COMP-004` | P2 | `[x] implemented` | `validate_changed` использовал hidden global adapter composition | Composition-aware API, focused CLI и executable regression в `main`; execution pending |
| `COMP-005` | P2 | `[x] implemented` | Legacy trust functions возвращали report со старым shared schema | Runtime trust API возвращает versioned public report; compatibility alias не меняет schema; execution pending |
| `COMP-006` | P2 | `[x] implemented` | Focused handlers устанавливали process-global runtime | Все active CLI families, включая MCP, используют explicit composition |
| `COMP-007` | P2 | `[x] implemented` | RusTok operations использовали global store/context APIs | Composition-aware audit/graph/context family и focused CLI в `main` |
| `COMP-008` | P2 | `[x] implemented` | API/Docs/Repair facades использовали task-local store composition | Direct paths используют `composition.init_store`; task-local Store bridge удалён из facade |
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
| `DOC-003` | P2 | `[x] implemented` | Docs engine находился в 3282-line compatibility owner через textual include | Legacy file удалён; contract/check/service/proposal/API/operations/tests — conventional bounded modules; execution pending |
| `VERIFY-001` | P1 | `[!] blocked` | В доступной среде нет Rust toolchain; hosted evidence отсутствует | fmt/tests/Clippy/workspace smoke выполнены и записаны на том же commit |
| `TOOL-001` | P2 | `[x] implemented` | Connector не предоставляет точечный patch больших Rust files | Runtime и Docs owners мигрированы атомарными Git trees без compatibility include |
| `TOOL-002` | P2 | `[x] implemented` | Contents connector не позволял безопасно менять CLI monolith | Команды перенесены полностью, старый monolith удалён |

## 5. Реализованные архитектурные срезы

### Runtime и adapter trust

- [x] `runtime.rs` сокращён с 1846 до 69 строк и стал conventional module root.
- [x] Models, registry, builder и trust разделены на bounded owners.
- [x] Pipeline, trust, process и fixture tests разделены по назначению.
- [x] Лишний runtime cancellation task-local удалён; process context имеет одного owner.
- [x] Public trust operations сразу формируют `VersionedAdapterTrustReport`.
- [x] `RUST-001/002`, `COMP-005`, `JSON-002` закрыты на уровне implementation.

### Docs engine

- [x] 3282-line `docs/legacy_impl.rs` удалён физически.
- [x] Contract models, check/drift, services, frontmatter, proposal orchestration и operations разделены.
- [x] Proposal engine дополнительно разделён на policy/API/operations/shared planners.
- [x] API documentation generation разделено на content/update/narrative owners.
- [x] Behavioral tests разделены на fixtures/check/frontmatter/proposal.
- [x] Source inventory запрещает `legacy_impl`/`include!` и ограничивает размеры модулей.

### Legacy runtime compatibility

- [x] Active CLI/MCP/Search/Generation/Index entrypoints используют `production()` и explicit composition.
- [x] Process-global state описан как четыре compatibility owner-группы.
- [x] Неиспользуемый `SCOPED_STORE_COMPOSITION` и `with_store_composition` удалены.
- [x] Public `require_legacy_*` factory-introspection helpers удалены.
- [x] Source inventory больше не консервирует удалённые compatibility bridges.
- [-] Следующий срез: `legacy-global-runtime` feature и default-vs-all-features compile matrix.

### MCP transport

- [x] Bounded response queue, request concurrency и continuous task reap.
- [x] Correct JSON-RPC/MCP session semantics и explicit composition.
- [x] `server.rs` удалён; production logic разделена на bounded modules.
- [x] Cancellation notifications обрабатываются при заполненном ordinary request limit.

### Explicit composition, publication, storage и CLI

- [x] Active CLI/MCP/RusTok/Graph/API/Docs/Repair используют explicit composition.
- [x] Publication commit-point и best-effort post-commit cleanup реализованы.
- [x] JSONL store и pointer writers декомпозированы.
- [x] CLI 4k-line monolith удалён, command/run/render owners разделены.

## 6. Verification

```bash
cargo fmt --all -- --check
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
cargo run -p ath --quiet --locked -- --help
cargo run -p ath --quiet --locked -- index .
```

До выполнения этого набора новые пункты сохраняют статус `implemented`, а не `verified`.

## 7. Активный рабочий пакет

**Сейчас:** `COMP-003B` — изолировать оставшиеся adapter/projector/Store/Search globals за opt-in `legacy-global-runtime` feature, чтобы default production build физически не содержал process-global factories.

Требования:

- feature в `athanor-app` и forwarding feature в `athanor-runtime-defaults`;
- default build компилирует только explicit composition path;
- all-features build сохраняет ограниченное compatibility window;
- installers и no-composition wrappers отсутствуют либо недоступны без feature;
- source inventory запрещает незащищённые compatibility globals;
- CI получает default и all-features compile/test checks.

Ограничение исполнения: connector не поддерживает patch больших `store.rs/search.rs`, поэтому этот срез должен выполняться атомарным переносом factory blocks в отдельные bounded modules, а не полной перезаписью implementation owners.

**После него:** `COMP-003C`, затем `MCP-007`, `JSON-003`, documentation reconciliation и полный verification.

## 8. Журнал актуализаций

### 2026-07-17 — Legacy runtime compatibility perimeter

- Завершён `COMP-003A` caller inventory.
- Удалены task-local Store bridge и public factory-introspection helpers без изменения active behavior.
- Active CLI/MCP/Search/Generation/Index подтверждены composition-only source regressions.
- Добавлен `docs/development/legacy-runtime-compatibility.md` с owner perimeter и removal sequence.
- `COMP-003` остаётся `in progress`; активным срезом стал `COMP-003B` feature quarantine.

### 2026-07-17 — Runtime and Docs owner decomposition

- `runtime.rs` разделён на model/registry/builder/trust и существующие plugin/process owners; root сокращён до 69 строк.
- Runtime tests разделены на pipeline/trust/process/fixtures.
- Удалён лишний runtime cancellation task-local; `clear_environment` platform defects исправлены.
- Runtime trust emitters возвращают только `athanor.adapter_trust_report.v1`; persisted registry остаётся `athanor.adapter_trust_registry.v2`.
- 3282-line `docs/legacy_impl.rs` удалён без compatibility include.
- Docs engine разделён на contract/check/service/frontmatter/proposal/API/operations и bounded tests.
- `RUNTIME-001`, `RUST-001`, `RUST-002`, `COMP-005`, `JSON-002`, `DOC-003` и `TOOL-001` отмечены `[x] implemented`; execution pending.

### 2026-07-17 — MCP server decomposition and saturation control

- Добавлен executable regression для focused `validate-changed`.
- Удалён 1000-line MCP `server.rs`; lifecycle/protocol/operation/types/tests разделены.
- Cancellation notifications bypass request saturation; overload возвращает retryable `-32001`.
- `MCP-004` и `MCP-006` отмечены `[x] implemented`.

### 2026-07-17 — Explicit composition, CLI, storage and publication

- RuntimeComposition проходит через active CLI/MCP operations.
- CLI monolith и JSONL compatibility includes удалены.
- Publication commit-point и post-commit cleanup semantics исправлены.
