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
| Implemented | 35 | Код и source/behavior regressions находятся в `main`; execution часто pending |
| In progress | 3 | Legacy API removal, schema enforcement и documentation claims |
| Planned | 2 | Transactional MCP cancellation и pipeline reconciliation |
| Blocked | 1 | Полный Rust verification недоступен в текущей среде |

Практически осталось **5 рабочих пакетов**:

1. `COMP-003C` — удалить no-composition bootstrap APIs, compatibility owners и legacy factory errors.
2. `MCP-007` — определить transactional Index cancellation до и после durable commit point.
3. `JSON-003` — повторить repository-wide schema scan и выполнить enforcement matrix.
4. `DOC-001/002` — убрать stale verification claims и согласовать pipeline current/target/history.
5. `VERIFY-001` — выполнить fmt/test/Clippy/smoke matrix и перевести подтверждённые пункты в `verified`.

`COMP-003A/B1/B2` завершены на уровне implementation: active entrypoints composition-only, а все оставшиеся compatibility globals находятся только в feature/test-only bounded owners. Основная новая архитектурная работа теперь сосредоточена в окончательном удалении compatibility API и `MCP-007`.

## 3. Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | MCP lifecycle/composition, adapter contracts, publication, JSONL store, CLI root, Runtime, Docs, Store и Search owners декомпозированы; legacy API removal и execution pending |
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
| `RUNTIME-001` | P1 | `[x] implemented` | `runtime.rs` был 1846-line owner для models, registry, builder, trust, process scope и tests | 50-line root и conventional bounded modules; tests разделены; no include; execution pending |
| `COMP-001` | P2 | `[x] implemented` | `OnceLock::set` conflicts молча игнорировались | Compatibility owners fail explicitly; execution pending |
| `COMP-002` | P1 | `[x] implemented` | Отсутствующий adapter factory создавал empty registry | Legacy path fail-fast; explicit composition — штатный path |
| `COMP-003` | P2 | `[-] in progress` | Store/Search/adapter/projector globals сохраняли process-wide hidden dependencies | `A/B1/B2` implemented; final no-composition API removal остаётся |
| `COMP-003A` | P2 | `[x] implemented` | Compatibility perimeter не был зафиксирован; Store bridge и introspection helpers сохранялись | Caller inventory, source perimeter, удалённый Store bridge и private lookup helpers |
| `COMP-003B1` | P2 | `[x] implemented` | Adapter/projector globals компилировались в default production build | Feature/test-only global owners, disabled default paths, CI/source enforcement; execution pending |
| `COMP-003B2` | P2 | `[x] implemented` | Store/Search globals находились в default implementation owners и дублировались в facades | Единственные feature/test-only Store/Search owners; core/facades без `OnceLock`; execution pending |
| `COMP-003C` | P2 | `[-] in progress` | Публичные no-composition wrappers и `athanor_runtime_defaults::install()` сохраняют compatibility surface | Caller parity, удаление wrappers/install/errors/owners и parallel composition isolation tests |
| `COMP-004` | P2 | `[x] implemented` | `validate_changed` использовал hidden global adapter composition | Composition-aware API, focused CLI и executable regression в `main`; execution pending |
| `COMP-005` | P2 | `[x] implemented` | Legacy trust functions возвращали report со старым shared schema | Runtime trust API возвращает versioned public report; compatibility alias не меняет schema; execution pending |
| `COMP-006` | P2 | `[x] implemented` | Focused handlers устанавливали process-global runtime | Все active CLI families, включая MCP, используют explicit composition |
| `COMP-007` | P2 | `[x] implemented` | RusTok operations использовали global store/context APIs | Composition-aware audit/graph/context family и focused CLI в `main` |
| `COMP-008` | P2 | `[x] implemented` | API/Docs/Repair facades использовали task-local store composition | Direct paths используют `composition.init_store`; последний service bridge удалён |
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
| `TOOL-001` | P2 | `[x] implemented` | Connector не предоставляет точечный patch больших Rust files | Runtime, Docs, Store и Search owners мигрированы атомарными Git trees |
| `TOOL-002` | P2 | `[x] implemented` | Contents connector не позволял безопасно менять CLI monolith | Команды перенесены полностью, старый monolith удалён |

## 5. Реализованные архитектурные срезы

### Runtime и adapter trust

- [x] `runtime.rs` сокращён с 1846 до 50 строк и стал conventional module root.
- [x] Models, registry, builder, trust и legacy API разделены на bounded owners.
- [x] Pipeline, trust, process и fixture tests разделены по назначению.
- [x] Adapter globals компилируются только при `legacy-global-runtime` или `cfg(test)`.

### Projection, Store и Search compatibility

- [x] Wiki/HTML `OnceLock` перенесены в `projection_legacy_global.rs`.
- [x] Store factory имеет единственного owner-а `store_legacy_global.rs`.
- [x] Search factories имеют единственного owner-а `search_legacy_global.rs`.
- [x] Default build содержит disabled paths без process-global factory storage.
- [x] Store разделён на root, KnowledgeStore, publication и canonical owners.
- [x] Search разделён на orchestration, contracts и index lifecycle owners.
- [x] CI feature matrix содержит default, `legacy-global-runtime` и `all-features` slices.

### Docs engine

- [x] 3282-line `docs/legacy_impl.rs` удалён физически.
- [x] Contract models, check/drift, services, frontmatter, proposal orchestration и operations разделены.
- [x] Proposal engine дополнительно разделён на policy/API/operations/shared planners.
- [x] API documentation generation разделено на content/update/narrative owners.
- [x] Behavioral tests разделены на fixtures/check/frontmatter/proposal.

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
cargo check --workspace --locked
cargo check --workspace --features legacy-global-runtime --locked
cargo check --workspace --all-features --locked
cargo test -p athanor-app runtime --locked
cargo test -p athanor-app docs --locked
cargo test -p athanor-app adapter_contract --locked
cargo test -p athanor-app --test runtime_modularity_inventory --locked
cargo test -p athanor-app --test service_composition_inventory --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
cargo test -p athanor-app --test legacy_factory_migration --features legacy-global-runtime --locked
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

**Сейчас:** `COMP-003C` — удалить оставшиеся public no-composition wrappers и process-global compatibility bootstrap после финальной caller/parity инвентаризации.

Требования:

- repository-wide caller inventory для no-composition Index/Search/Generation/Wiki и RuntimeBuilder paths;
- external compatibility surface либо мигрирован, либо получает явный breaking-change record;
- удалены `athanor_runtime_defaults::install()`, installer exports и `legacy-global-runtime` feature;
- удалены `LegacyFactoryInstallError`, `LegacyFactoryUnavailableError` и четыре global owner-а;
- RuntimeComposition остаётся единственным runtime dependency root;
- parallel isolation tests используют разные Store/Search/projector factories без global leakage;
- targeted/default/all-features tests и Clippy выполнены.

**После него:** `MCP-007`, затем `JSON-003`, documentation reconciliation и полный verification.

## 8. Журнал актуализаций

### 2026-07-17 — Store and Search feature quarantine

- Реализован `COMP-003B2`.
- Store factory state удалён из `store.rs` и facade; единственный compatibility owner — `store_legacy_global.rs`.
- Store trait implementations разделены на `knowledge`, `publication` и `canonical` modules.
- Search factory state удалён из `search.rs` и facade; единственный compatibility owner — `search_legacy_global.rs`.
- Search contracts и index lifecycle вынесены в `search/model.rs` и `search/index.rs`.
- Default Store/Search no-composition paths fail fast и указывают на `RuntimeComposition`.
- Source inventory фиксирует точные global owners и module-size boundaries.
- Статус — implemented, execution pending.

### 2026-07-17 — Adapter and projector feature quarantine

- Реализован `COMP-003B1`.
- Добавлен `legacy-global-runtime` feature в `athanor-app` и forwarding feature в `athanor-runtime-defaults`.
- Adapter registry/resolver globals компилируются только при feature или unit tests; default owner fail-fast и не содержит `OnceLock`.
- Wiki/HTML projector globals перенесены в `projection_legacy_global.rs` с такой же feature/test boundary.
- Runtime compatibility wrappers вынесены в bounded `runtime/legacy_api.rs`; root остаётся 50 строк.
- CI получил отдельный `legacy-global-runtime` slice; source inventory проверяет default-vs-legacy perimeter.

### 2026-07-17 — Legacy runtime compatibility perimeter

- Завершён `COMP-003A` caller inventory.
- Удалены task-local Store bridge и public factory-introspection helpers без изменения active behavior.
- Active CLI/MCP/Search/Generation/Index подтверждены composition-only source regressions.
- Добавлен `docs/development/legacy-runtime-compatibility.md` с owner perimeter и removal sequence.

### 2026-07-17 — Runtime and Docs owner decomposition

- `runtime.rs` разделён на model/registry/builder/trust и существующие plugin/process owners.
- Runtime tests разделены на pipeline/trust/process/fixtures.
- Удалён лишний runtime cancellation task-local; `clear_environment` platform defects исправлены.
- Runtime trust emitters возвращают только `athanor.adapter_trust_report.v1`; persisted registry остаётся `athanor.adapter_trust_registry.v2`.
- 3282-line `docs/legacy_impl.rs` удалён без compatibility include.
- Docs engine разделён на contract/check/service/frontmatter/proposal/API/operations и bounded tests.
