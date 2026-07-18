# Athanor: консолидированный план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-18  
> Статус: active architecture audit

## 1. Правила статусов

- `[x] implemented` — изменение находится в `main`, но не считается проверенным без execution evidence.
- `[x] verified` — реализация и regressions подтверждены выполненными проверками на соответствующем commit.
- `[-] in progress` — полезные срезы находятся в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — подтверждённая работа ещё не начата.
- `[!] blocked` — безопасное изменение или проверка временно недоступны.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id или
protocol version. CLI, daemon и MCP варианты одной операции должны сохранять эквивалентные
application payloads.

## 2. Состояние программы

Process-global runtime state, installer API, write-service wrappers и Context compatibility owners
удалены. Context cores и весь daemon lifecycle — host, query, derived read, command dispatch и write
jobs — composition-first. Index, Generation, Wiki, HTML report, benchmark, Explain и публичный
ChangeMap имеют только composition-aware execution API. Operation-aware no-composition Search
compatibility APIs удалены; последний snapshot Search fallback теперь crate-private и ограничен
внутренним legacy ChangeMap core.

Осталось четыре содержательных пакета и финальная verification matrix:

1. `COMP-003C2B2C2B` — физически очистить ChangeMap core, удалить Store/snapshot Search compatibility
   paths и мигрировать оставшиеся read-service owners с implicit/optional composition.
2. `MCP-007` — определить transactional Index cancellation до и после durable commit point.
3. `JSON-003` — повторить repository-wide schema scan и выполнить enforcement matrix.
4. `DOC-001/002` — убрать stale verification claims и согласовать pipeline current/target/history.
5. `VERIFY-001` — выполнить fmt/test/Clippy/smoke matrix и перевести подтверждённые пункты в `verified`.

`COMP-003A/B1/B2/C1/C2A/C2B1/C2B2A/C2B2B1/C2B2B2A/C2B2B2B/C2B2C1/C2B2C2A`
завершены на уровне implementation. Внутри `COMP-003C2B2C2B` удалены unused operation-aware Search
wrappers, закрыт Explain owner и внешний ChangeMap API маршрутизирован через composition-only facade.
Остающийся debt сосредоточен во внутреннем ChangeMap core, Store facade и read-service owners,
которые ещё импортируют `store::init_store` или принимают optional composition.

## 3. Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Архитектурные owners декомпозированы; internal ChangeMap/Store read cleanup и execution pending |
| `DS-JSON-001` | P1 | `[x] implemented` | Public registry 60; manifest, trust registry и public report имеют разные current owners |
| `DS-JSON-002` | P1 | `[-] in progress` | General и adapter-specific non-public descriptors добавлены; repeat scan pending |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed CLI/daemon/MCP/plugin payload parity реализована; execution pending |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |

## 4. Реестр подтверждённых дефектов и рисков

| ID | Priority | Status | Подтверждённая проблема | Критерий закрытия |
| --- | --- | --- | --- | --- |
| `MCP-001` | P1 | `[x] implemented` | Response path использовал unbounded channel | Bounded queue, single writer и выполненные MCP regressions |
| `MCP-002` | P1 | `[x] implemented` | Requests spawn-ились без лимита, completed tasks жили до EOF | In-flight cap, continuous reap и lifecycle tests |
| `MCP-003` | P1 | `[x] implemented` | Неверные JSON-RPC version/error/id/session semantics | Exact `2.0`, standard codes, distinct omitted/null id и session tests |
| `MCP-004` | P2 | `[x] implemented` | Saturation ordinary requests останавливала control-plane input | Notifications bypass saturation; overload retryable |
| `MCP-005` | P1 | `[x] implemented` | MCP runtime использовал legacy dispatcher/global installation | Explicit composition через весь MCP lifecycle |
| `MCP-006` | P1 | `[x] implemented` | MCP `server.rs` был монолитом | Bounded lifecycle/operation/protocol/types modules |
| `MCP-007` | P1 | `[ ] planned` | Index cancellation не различает pre-commit rollback и post-commit durable success | Явный commit-point contract и pre/post-commit regressions |
| `JSON-001` | P1 | `[x] implemented` | Adapter manifest имел неверсионированный schema id | `athanor.adapter_manifest.v1`; legacy id только migration input |
| `JSON-002` | P1 | `[x] implemented` | Persisted registry и public report делили schema owner | Current emitters используют раздельные versioned contracts |
| `JSON-003` | P1 | `[-] in progress` | Inventory считался complete без adapter boundaries | Repeat scan и enforcement execution |
| `JSON-004` | P2 | `[x] implemented` | Adapter contract symbols re-exported двумя facade paths | Один canonical re-export path |
| `RUST-001` | P1 | `[x] implemented` | Unix fixture дважды задавал `clear_environment` | Fixture/source regression; Rust execution pending |
| `RUST-002` | P1 | `[x] implemented` | Non-Windows helper не задавал `clear_environment` | Helper/source regression; Rust execution pending |
| `RUNTIME-001` | P1 | `[x] implemented` | `runtime.rs` был 1846-line owner | Conventional bounded modules; no include |
| `COMP-001` | P2 | `[x] implemented` | `OnceLock::set` conflicts молча игнорировались | Process-global runtime storage удалён |
| `COMP-002` | P1 | `[x] implemented` | Отсутствующий adapter factory создавал empty registry | Explicit registry/composition; no hidden fallback |
| `COMP-003` | P2 | `[-] in progress` | Runtime dependencies скрывались за globals и compatibility APIs | Public ChangeMap migrated; internal Store/Search read debt и execution остаются |
| `COMP-003A` | P2 | `[x] implemented` | Compatibility perimeter не был зафиксирован | Caller inventory и удаление Store bridge/introspection helpers |
| `COMP-003B1` | P2 | `[x] implemented` | Adapter/projector globals компилировались в default build | Quarantine и последующее физическое удаление |
| `COMP-003B2` | P2 | `[x] implemented` | Store/Search globals находились в implementation owners | Bounded owners и последующее физическое удаление |
| `COMP-003C1` | P2 | `[x] implemented` | Feature/test-only globals и legacy errors сохраняли process state | Feature, OnceLock owners, errors и test installation удалены |
| `COMP-003C2` | P2 | `[-] in progress` | Installer и no-composition shims сохраняли compatibility surface | C2B2C2B и execution закрыты |
| `COMP-003C2A` | P2 | `[x] implemented` | State-free installer functions и `runtime_defaults::install()` оставались public | Все installer symbols/re-exports удалены; enforcement/isolation в `main` |
| `COMP-003C2B1` | P2 | `[x] implemented` | Dead no-composition Validate/Search wrappers оставались public | Removed wrappers и source enforcement в `main` |
| `COMP-003C2B2` | P2 | `[-] in progress` | Связанные service chains принимают optional/no composition | Public ChangeMap закрыт; internal Store/Search read paths остаются |
| `COMP-003C2B2A` | P2 | `[x] implemented` | Active Context owner был optional-composition и зависел от удалённого Search wrapper | Composition-first owner активен; behavior/source regressions в `main` |
| `COMP-003C2B2B1` | P2 | `[x] implemented` | Operation-aware Context core принимал optional composition и fallback Store/Search | Core принимает `&RuntimeComposition`; fallback imports/branches удалены |
| `COMP-003C2B2B2` | P2 | `[x] implemented` | Daemon host и execution layers использовали optional composition | Mandatory host/query/read/write composition и bounded dispatch в `main` |
| `COMP-003C2B2B2A` | P2 | `[x] implemented` | Daemon queries, derived reads и write jobs выбирали no-composition fallbacks | Query/read/write execution composition-only; source enforcement в `main` |
| `COMP-003C2B2B2B` | P2 | `[x] implemented` | `DaemonState.composition`, `serve_daemon` и host constructors оставались optional | Mandatory field/serve API, migrated tests и отсутствие host Option |
| `COMP-003C2B2C` | P2 | `[-] in progress` | Public service APIs и compatibility facades скрывали composition | C1/C2A и public ChangeMap закрыты; internal read cleanup активен |
| `COMP-003C2B2C1` | P2 | `[x] implemented` | Index, Generation, Wiki, HTML и benchmark имели no-composition APIs/projector fallbacks | Composition-only signatures/cores, narrowed re-exports и source inventory в `main` |
| `COMP-003C2B2C2` | P2 | `[-] in progress` | Read services сохраняли dependency-hidden compatibility paths | Context и public ChangeMap закрыты; internal ChangeMap/Store owners активны |
| `COMP-003C2B2C2A` | P2 | `[x] implemented` | Context, derived-read, Search-operation и RusTok compatibility owners оставались public/physical | Wrappers удалены; `context.rs`/`rustok_operation.rs` удалены; inventory в `main` |
| `COMP-003C2B2C2B` | P2 | `[-] in progress` | Store facade, snapshot Search, ChangeMap и другие read services сохраняют implicit/optional composition | Search wrappers, Explain и public ChangeMap surface закрыты; internal core/Store/read owners pending |
| `COMP-004` | P2 | `[x] implemented` | `validate_changed` использовал hidden adapter composition | Только composition-aware public path; execution pending |
| `COMP-005` | P2 | `[x] implemented` | Trust functions возвращали report со старым schema | Versioned public report |
| `COMP-006` | P2 | `[x] implemented` | Focused handlers устанавливали global runtime | Все active CLI families используют explicit composition |
| `COMP-007` | P2 | `[x] implemented` | RusTok operations использовали global store/context | Composition-aware operation family; duplicate owner удалён |
| `COMP-008` | P2 | `[x] implemented` | API/Docs/Repair использовали task-local store composition | Direct paths используют explicit composition |
| `COMP-009` | P1 | `[x] implemented` | После удаления Search wrapper старый active Context owner продолжал вызывать удалённый symbol | Active owner заменён; obsolete owner физически удалён |
| `COMP-010` | P2 | `[x] implemented` | `daemon.rs` дублировал command dispatcher и смешивал host lifecycle с request execution | Bounded command dispatcher; duplicate `execute_request` удалён; owner 4224→866 lines |
| `PUB-001` | P1 | `[x] implemented` | Trust writer мог вернуть failure после durable rename | Cleanup best effort после commit point |
| `PUB-002` | P1 | `[x] implemented` | Staged replace смешивал durable success и maintenance failure | Explicit commit point/non-fatal maintenance |
| `PUB-003` | P2 | `[x] implemented` | Не было failure-injection regressions | Stage/rollback/cleanup/Drop/race/recovery matrix |
| `PUB-004` | P1 | `[x] implemented` | Finalizers возвращали cleanup failure после commit | Warning и successful publication |
| `PUB-005` | P1 | `[x] implemented` | JSONL pointers имели duplicate replacement helpers | Один pointer-publication owner |
| `STORE-001` | P1 | `[x] implemented` | JSONL store состоял из compatibility includes/large files | Conventional bounded modules |
| `CLI-001` | P2 | `[x] implemented` | CLI root включал 4k-line monolith | Typed bounded command families |
| `CLI-002` | P2 | `[x] implemented` | Plugin commands использовали legacy report schema | Versioned executable contract |
| `CLI-003` | P2 | `[x] implemented` | Read/Application Report/Repair использовали include wrappers | Explicit model/run/render modules |
| `CLI-004` | P2 | `[x] implemented` | Graph/RusTok/Check зависели от renderer bridges | Dedicated renderers/root model |
| `CLI-005` | P2 | `[x] implemented` | Legacy-only grammars оставались в monolith | Dedicated command families |
| `DOC-001` | P3 | `[-] in progress` | Документы содержат stale `verified` claims/removed paths | Current-HEAD evidence либо честный pending status |
| `DOC-002` | P3 | `[ ] planned` | Pipeline architecture противоречит current/planned sections | Current/target/history согласованы |
| `DOC-003` | P2 | `[x] implemented` | Docs engine находился в 3282-line owner | Bounded conventional modules |
| `VERIFY-001` | P1 | `[!] blocked` | Rust toolchain и hosted evidence недоступны | fmt/tests/Clippy/workspace smoke выполнены на одном commit |
| `TOOL-001` | P2 | `[x] implemented` | Connector не позволял точечно менять большие Rust files | Owners мигрированы полными GitHub contents updates |
| `TOOL-002` | P2 | `[x] implemented` | Connector не позволял безопасно менять CLI monolith | Команды перенесены, monolith удалён |

## 5. Реализованные архитектурные срезы

### Explicit runtime composition

- [x] Active CLI/MCP/API/Docs/Repair/Search/Generation/Index paths используют explicit composition.
- [x] `legacy-global-runtime` удалён из Cargo и CI.
- [x] Adapter, Projector, Store и Search global owner files удалены.
- [x] Legacy factory errors и installer APIs удалены.
- [x] Parallel isolation matrix покрывает разные Store/Search/Wiki/HTML factories.
- [x] Dead no-composition Validate/Search project wrappers удалены.
- [x] Active Context owner использует mandatory composition для Store и Search.
- [x] Operation-aware Context core использует mandatory composition.
- [x] Public Context/derived-read/Search-operation compatibility APIs удалены.
- [x] Daemon host/query/derived-read/write execution использует mandatory composition.
- [x] Index/Generation/Wiki/HTML/benchmark public APIs composition-only.
- [x] Explain public execution API composition-only.
- [x] Public ChangeMap execution API composition-only.
- [x] Index RuntimeBuilder и Store fallback branches удалены.
- [x] Generation/Wiki/HTML Store и projector fallback branches удалены.
- [x] Operation-aware snapshot Search и Search-index compatibility wrappers удалены.
- [x] Последний no-composition snapshot Search helper скрыт из public API (`pub(crate)`).
- [ ] ChangeMap core использует только mandatory composition и composition-aware snapshot Search.
- [ ] Crate-private `search_snapshot` compatibility helper физически удалён.
- [ ] Public `store::init_store` compatibility facade удалён.
- [ ] Remaining read-service owners composition-only.

### Context owner replacement

- [x] `lib.rs` маршрутизирует `context` на `context_composition.rs`.
- [x] Active normal/operation cores не импортируют `crate::store::init_store`.
- [x] Active normal/operation cores не вызывают no-composition Search builders.
- [x] Search factories вызываются через supplied composition.
- [x] Ranking, relation expansion, diagnostics и limits покрыты integration regressions.
- [x] Внешние Context compatibility edges удалены.
- [x] Старый `context.rs` физически удалён.
- [x] Duplicate `rustok_operation.rs` физически удалён.
- [x] RusTok architecture model отделён от Store/Context execution.

### Daemon composition

- [x] `athd` production startup использует `serve_daemon_with_composition`.
- [x] `DaemonState.composition` обязателен.
- [x] Legacy `serve_daemon` и optional `serve_daemon_inner` удалены.
- [x] Snapshot/Search query cache paths не имеют Store/Search fallback.
- [x] Derived Context/ChangeMap dispatch вызывает только composition-aware APIs.
- [x] Index/Generate/Wiki/HTML jobs используют total composition accessor.
- [x] Control/write commands вынесены в bounded command dispatcher.
- [x] Duplicate `daemon.rs::execute_request` удалён.
- [x] Cancellation/deadline и write-report contracts сохранены bounded test owners.
- [x] Source inventory запрещает возврат optional host state и fallback branches.

### Runtime, Docs, Store, Search, MCP, publication и CLI

- [x] Runtime, Docs, Store и Search разделены на bounded owners.
- [x] Docs legacy owner и CLI monolith удалены.
- [x] Bounded MCP lifecycle и correct JSON-RPC semantics реализованы.
- [x] Publication commit-point и best-effort post-commit cleanup реализованы.
- [x] JSONL store/pointer writers декомпозированы.

## 6. Verification

```bash
cargo fmt --all -- --check
cargo check --workspace --locked
cargo check --workspace --all-features --locked
cargo test -p athanor-app runtime --locked
cargo test -p athanor-app daemon --locked
cargo test -p athanor-app search_operation --locked
cargo test -p athanor-app derived_read_operation --locked
cargo test -p athanor-app --test runtime_modularity_inventory --locked
cargo test -p athanor-app --test service_composition_inventory --locked
cargo test -p athanor-app --test read_service_composition_inventory --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
cargo test -p athanor-app --test write_service_composition_inventory --locked
cargo test -p athanor-app --test composition_isolation --locked
cargo test -p athanor-app --test context_composition_inventory --locked
cargo test -p athanor-app --test context_pack_behavior --locked
cargo test -p athanor-app --test daemon_composition_inventory --locked
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

**Сейчас:** `COMP-003C2B2C2B` — Store/Search и remaining read-service composition cleanup.

Отметки:

- [x] удалить no-composition operation-aware snapshot Search wrapper;
- [x] удалить no-composition operation-aware Search-index wrapper;
- [x] добавить source inventory, запрещающий возврат удалённых wrappers;
- [x] сделать Explain composition-only и удалить Store fallback;
- [x] маршрутизировать public ChangeMap module через composition-only bounded facade;
- [x] убрать no-composition `change_map_project` из внешних re-exports;
- [x] скрыть последний `search_snapshot` compatibility helper до `pub(crate)`;
- [x] добавить inventory для composition-only ChangeMap facade и bounded Search fallback;
- [ ] физически переписать ChangeMap core на mandatory `&RuntimeComposition` без optional branch;
- [ ] передавать composition в task Search через `search_snapshot_with_composition`;
- [ ] удалить crate-private `search_snapshot` compatibility helper после core migration;
- [ ] удалить временный legacy core boundary после физической очистки `change_map.rs`;
- [ ] удалить public `store::init_store` compatibility edge после migration всех callers;
- [ ] мигрировать Overview, Impact, Coverage, Capabilities, Graph, Check, API/registry,
  Repair/Docs и другие read owners с `store::init_store`/optional composition;
- [ ] обновить embedding examples после physical core cleanup;
- [ ] устранить source-level warnings после owner cleanup;
- [ ] выполнить targeted/default/all-features tests и Clippy.

**Следующий обязательный срез:** физически очистить `change_map.rs`: удалить `change_map_project`,
`Option<&RuntimeComposition>`, `crate::store::init_store` и fallback match; task Search должен вызывать
`search_snapshot_with_composition`. После этого удалить crate-private `search_snapshot` и временный
legacy core boundary, оставив один conventional ChangeMap owner без forwarding compatibility API.

**После пакета:** `MCP-007`, затем `JSON-003`, documentation reconciliation и полный verification.

## 8. Журнал актуализаций

### 2026-07-18 — Composition-only public ChangeMap boundary

- В рамках `COMP-003C2B2C2B` добавлен bounded `change_map_facade.rs`.
- `lib.rs` маршрутизирует public `change_map` module через composition-only facade.
- Внешний `change_map_project` больше не re-exported; доступен только
  `change_map_project_with_composition`.
- Последний no-composition `search_snapshot` скрыт до `pub(crate)` и ограничен legacy ChangeMap core.
- `read_service_composition_inventory` проверяет routing, explicit composition, отсутствие glob
  re-export и bounded facade sizes.
- Internal `change_map.rs` всё ещё содержит optional Store/Search fallback и поэтому не отмечен как
  закрытый; следующим шагом назначена его физическая очистка без сохранения forwarding shims.
- Статус среза — implemented, Rust/hosted verification pending.

### 2026-07-18 — Composition-only Explain owner

- В рамках `COMP-003C2B2C2B` удалён no-composition `explain_project`.
- `explain_project_inner` принимает обязательный `&RuntimeComposition`.
- Store создаётся только через `composition.init_store`; `crate::store::init_store`, optional state и
  fallback branch удалены.
- `read_service_composition_inventory` запрещает возврат no-composition Explain API и fallback.
- Формат `EntityExplanation` и pure `explain_snapshot` не изменялись.
- Статус среза — implemented, Rust/hosted verification pending.

### 2026-07-18 — Bounded Search compatibility cleanup

- В рамках `COMP-003C2B2C2B` удалены `search_snapshot_with_operation_context` и
  `get_or_build_search_index_with_operation_context`.
- Удалён ставший ненужным `Arc` import из Search facade.
- Добавлен `read_service_composition_inventory`, запрещающий возврат удалённых APIs.
- Inventory отдельно фиксирует, что последний no-composition `search_snapshot` временно ограничен
  ChangeMap и является следующим migration target.
- Статус пакета остаётся `in progress`; implementation evidence есть, Rust/hosted verification нет.

### 2026-07-18 — Physical Context compatibility removal

- Реализован `COMP-003C2B2C2A`.
- Удалены no-composition Context, derived Context/ChangeMap и Search operation entrypoints.
- Удалён duplicate no-composition RusTok operation module и его re-exports.
- Физически удалены `context.rs` и `rustok_operation.rs`.
- `rustok_architecture.rs` оставлен owner-ом contracts и pure snapshot transformation.
- `context_composition_inventory` проверяет mandatory routing и physical owner absence.
- Developer guide больше не заявляет сохранённые compatibility wrappers.
- `COMP-003C2B2C2B` назначен активным.
- Статус — implemented, execution pending.

### 2026-07-18 — Composition-only write services

- Реализован `COMP-003C2B2C1`.
- Удалены no-composition Index, Generation, Wiki, HTML report и benchmark entrypoints.
- Добавлены composition-aware non-cancellable operation-context entrypoints для Index, Generation,
  Wiki и HTML report.
- Index core всегда использует `RuntimeComposition::init_store` и `RuntimeBuilder::from_composition`.
- Generation/Wiki/HTML cores всегда используют supplied Store/projector factories.
- Projector compatibility functions физически удалены из `projection.rs`.
- Stable indexing re-exports сужены до composition-aware API.
- Добавлен `write_service_composition_inventory`; legacy migration enforcement расширен.
- Статус — implemented, execution pending.

### 2026-07-18 — Mandatory daemon host composition

- Реализован `COMP-003C2B2B2B`.
- `DaemonState.composition` заменён на обязательный `RuntimeComposition`.
- Удалены legacy `serve_daemon`, optional `serve_daemon_inner` и missing-composition errors.
- Добавлен bounded `daemon_command_dispatch.rs`; duplicate `daemon.rs::execute_request` удалён.
- `daemon.rs` сокращён с 4224 до 866 строк и оставлен owner-ом wire types/transport lifecycle.
- Read dispatcher делегирует control/write commands bounded dispatcher-у.
- Cancellation/deadline и write-report shape tests перенесены в bounded test owners.
- Статус — implemented, execution pending.

### 2026-07-18 — Daemon execution composition

- Реализован `COMP-003C2B2B2A`.
- `daemon_queries` больше не импортирует Store или no-composition Search fallback.
- Derived Context/ChangeMap dispatch вызывает только composition-aware operations.
- Index/Generate/Wiki/HTML write jobs требуют composition до создания job.
- Статус — implemented, execution pending.

### 2026-07-18 — Operation-aware Context composition

- Реализован `COMP-003C2B2B1`.
- `context_project_with_operation_context_impl` принимает обязательный `&RuntimeComposition`.
- Store и operation-aware Search строятся только через composition.
- Удалены optional composition и fallback branches из core.
- Статус — implemented, execution pending.

### 2026-07-18 — Composition-first Context owner

- Реализован `COMP-003C2B2A`.
- Зарегистрирован `COMP-009`: старый active Context owner вызывал удалённый Search symbol.
- Добавлен bounded `context_composition.rs`; `lib.rs` переключён на новый owner.
- Добавлены source inventory и behavior regressions.
- Статус — implemented, execution pending.

### 2026-07-18 — Dead no-composition wrapper cleanup

- Реализован `COMP-003C2B1`.
- Удалены dead Validate/Search wrappers и добавлен source regression.
- Статус — implemented, execution pending.

### 2026-07-18 — Runtime installer API removal

- Реализован `COMP-003C2A`.
- Удалены installer APIs и добавлена parallel composition isolation matrix.
- Статус — implemented, execution pending.

### 2026-07-17 — Process-global runtime removal

- Реализован `COMP-003C1`.
- Удалён Cargo feature, global owner files, legacy errors и test installation bootstrap.
