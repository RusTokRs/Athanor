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

Process-global runtime state, installer API и dead no-composition wrappers удалены. Context cores,
daemon query/derived-read paths и daemon write jobs теперь composition-first. Осталось четыре
содержательных пакета и финальная verification matrix:

1. `COMP-003C2B2B2B/C` — завершить mandatory composition propagation через daemon host, Index,
   Generation, Wiki и benchmark public APIs.
2. `MCP-007` — определить transactional Index cancellation до и после durable commit point.
3. `JSON-003` — повторить repository-wide schema scan и выполнить enforcement matrix.
4. `DOC-001/002` — убрать stale verification claims и согласовать pipeline current/target/history.
5. `VERIFY-001` — выполнить fmt/test/Clippy/smoke matrix и перевести подтверждённые пункты в `verified`.

`COMP-003A/B1/B2/C1/C2A/C2B1/C2B2A/C2B2B1/C2B2B2A` завершены на уровне implementation. Store,
Search, Context и daemon execution paths получают `RuntimeComposition`; остающийся compatibility
debt сосредоточен в optional daemon host state и публичных write-service API families.

## 3. Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Архитектурные owners декомпозированы; daemon-host/write-service cleanup и execution pending |
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
| `COMP-003` | P2 | `[-] in progress` | Runtime dependencies скрывались за globals и compatibility APIs | Context/daemon execution migrated; daemon host/public write APIs и execution остаются |
| `COMP-003A` | P2 | `[x] implemented` | Compatibility perimeter не был зафиксирован | Caller inventory и удаление Store bridge/introspection helpers |
| `COMP-003B1` | P2 | `[x] implemented` | Adapter/projector globals компилировались в default build | Quarantine и последующее физическое удаление |
| `COMP-003B2` | P2 | `[x] implemented` | Store/Search globals находились в implementation owners | Bounded owners и последующее физическое удаление |
| `COMP-003C1` | P2 | `[x] implemented` | Feature/test-only globals и legacy errors сохраняли process state | Feature, OnceLock owners, errors и test installation удалены |
| `COMP-003C2` | P2 | `[-] in progress` | Installer и no-composition shims сохраняли compatibility surface | C2A/C2B1/C2B2 закрыты, isolation matrix выполнена |
| `COMP-003C2A` | P2 | `[x] implemented` | State-free installer functions и `runtime_defaults::install()` оставались public | Все installer symbols/re-exports удалены; enforcement/isolation в `main` |
| `COMP-003C2B1` | P2 | `[x] implemented` | Dead no-composition Validate/Search wrappers оставались public | Removed wrappers и source enforcement в `main` |
| `COMP-003C2B2` | P2 | `[-] in progress` | Связанные service chains принимают optional/no composition | Context/daemon-host/write-service slices завершены и fallback branches отсутствуют |
| `COMP-003C2B2A` | P2 | `[x] implemented` | Active Context owner был optional-composition и зависел от удалённого Search wrapper | Composition-first owner активен; behavior/source regressions в `main` |
| `COMP-003C2B2B1` | P2 | `[x] implemented` | Operation-aware Context core принимал optional composition и fallback Store/Search | Core принимает `&RuntimeComposition`; fallback imports/branches удалены; execution pending |
| `COMP-003C2B2B2` | P2 | `[-] in progress` | Daemon host и execution layers используют optional composition | B2B2A execution и B2B2B host-state cleanup закрыты |
| `COMP-003C2B2B2A` | P2 | `[x] implemented` | Daemon queries, derived reads и write jobs выбирали no-composition fallbacks | Query/read/write execution composition-only; source enforcement в `main`; execution pending |
| `COMP-003C2B2B2B` | P2 | `[-] in progress` | `DaemonState.composition`, `serve_daemon` и host constructors остаются optional | Mandatory field/entrypoint, migrated tests и отсутствие `Option<RuntimeComposition>` в daemon host |
| `COMP-003C2B2C` | P2 | `[ ] planned` | Index, Generation, Wiki и benchmark сохраняют no-composition APIs/projector fallbacks | Composition-only signatures, updated re-exports/tests/examples |
| `COMP-004` | P2 | `[x] implemented` | `validate_changed` использовал hidden adapter composition | Только composition-aware public path; execution pending |
| `COMP-005` | P2 | `[x] implemented` | Trust functions возвращали report со старым schema | Versioned public report |
| `COMP-006` | P2 | `[x] implemented` | Focused handlers устанавливали global runtime | Все active CLI families используют explicit composition |
| `COMP-007` | P2 | `[x] implemented` | RusTok operations использовали global store/context | Composition-aware operation family |
| `COMP-008` | P2 | `[x] implemented` | API/Docs/Repair использовали task-local store composition | Direct paths используют explicit composition |
| `COMP-009` | P1 | `[x] implemented` | После удаления Search wrapper старый active Context owner продолжал вызывать удалённый symbol | Active owner заменён; removed symbol вне compiled Context; source regression в `main` |
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
- [x] Dead no-composition Validate/Search wrappers удалены.
- [x] Active Context owner использует mandatory composition для Store и Search.
- [x] Operation-aware Context core использует mandatory composition.
- [x] Daemon query/derived-read/write execution использует mandatory composition.
- [ ] Daemon host state и serve API используют mandatory composition.
- [ ] Index/Generation/Wiki/benchmark public APIs composition-only.

### Context owner replacement

- [x] `lib.rs` маршрутизирует `context` на `context_composition.rs`.
- [x] Active normal/operation cores не импортируют `crate::store::init_store`.
- [x] Active normal/operation cores не вызывают no-composition Search builders.
- [x] Search factories вызываются через supplied composition.
- [x] Ranking, relation expansion, diagnostics и limits покрыты integration regressions.
- [ ] Внешние compatibility edges удалены после daemon/RusTok caller migration.
- [ ] Старый `context.rs` физически удалён.

### Daemon composition

- [x] `athd` production startup использует `serve_daemon_with_composition`.
- [x] Snapshot/Search query cache paths не имеют Store/Search fallback.
- [x] Derived Context/ChangeMap dispatch вызывает только composition-aware APIs.
- [x] Index/Generate/Wiki/HTML jobs проверяют composition до создания job.
- [x] Source inventory запрещает возврат execution fallback branches.
- [ ] `DaemonState.composition` перестал быть `Option`.
- [ ] Legacy `serve_daemon` и optional `serve_daemon_inner` удалены.
- [ ] Daemon test constructors передают обязательную composition.

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
cargo test -p athanor-app --test runtime_modularity_inventory --locked
cargo test -p athanor-app --test service_composition_inventory --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
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

**Сейчас:** `COMP-003C2B2B2B` — mandatory composition в daemon host state и construction API.

Требования:

- заменить `DaemonState.composition: Option<RuntimeComposition>` на обязательную composition;
- удалить `serve_daemon` compatibility entrypoint и `serve_daemon_inner(..., Option<...>)`;
- заменить `daemon_queries::composition() -> Option<_>` на обязательный accessor либо прямой borrow;
- убрать оставшийся branching в Context compatibility branch `daemon.rs`;
- обновить daemon test state constructors и source inventories;
- удалить временный `context_project_with_operation_context` edge, когда daemon/RusTok callers migrated;
- выполнить targeted/default/all-features tests и Clippy.

**После него:** `COMP-003C2B2C`, затем `MCP-007`, `JSON-003`, documentation reconciliation и полный verification.

## 8. Журнал актуализаций

### 2026-07-18 — Daemon execution composition

- Реализован `COMP-003C2B2B2A`.
- `daemon_queries` больше не импортирует Store или no-composition Search fallback.
- Derived Context/ChangeMap dispatch вызывает только composition-aware operations.
- Index/Generate/Wiki/HTML write jobs требуют composition до создания job.
- Добавлен `daemon_composition_inventory` для read/write execution и явного remaining host debt.
- `COMP-003C2B2B2B` назначен активным.
- Статус — implemented, execution pending.

### 2026-07-18 — Operation-aware Context composition

- Реализован `COMP-003C2B2B1`.
- `context_project_with_operation_context_impl` принимает обязательный `&RuntimeComposition`.
- Store и operation-aware Search строятся только через composition.
- Удалены `Option<RuntimeComposition>`, `match composition`, `init_store` и no-composition Search fallback из core.
- Temporary public compatibility edge изолирован в `derived_read_operation.rs` до daemon migration.
- Source inventory расширен на operation-aware core.
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
