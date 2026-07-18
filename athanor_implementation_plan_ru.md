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

Process-global runtime state, installer API и dead no-composition wrappers удалены. Context переведён
на отдельный composition-first owner. Осталось четыре содержательных пакета и финальная
verification matrix:

1. `COMP-003C2B2B/C` — завершить mandatory composition propagation через daemon, operation-aware
   Context, Index, Generation, Wiki и benchmark.
2. `MCP-007` — определить transactional Index cancellation до и после durable commit point.
3. `JSON-003` — повторить repository-wide schema scan и выполнить enforcement matrix.
4. `DOC-001/002` — убрать stale verification claims и согласовать pipeline current/target/history.
5. `VERIFY-001` — выполнить fmt/test/Clippy/smoke matrix и перевести подтверждённые пункты в `verified`.

`COMP-003A/B1/B2/C1/C2A/C2B1/C2B2A` завершены на уровне implementation. Активный Context owner
обязательно принимает `RuntimeComposition` для Store/Search routing. Оставшийся compatibility debt
связан с daemon state и другими service families, где composition всё ещё optional либо скрыта за
no-composition signatures.

## 3. Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | Архитектурные owners декомпозированы; composition service cleanup и execution pending |
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
| `COMP-003` | P2 | `[-] in progress` | Runtime dependencies скрывались за globals и compatibility APIs | `A/B1/B2/C1/C2A/C2B1/C2B2A` implemented; B2B/B2C и execution остаются |
| `COMP-003A` | P2 | `[x] implemented` | Compatibility perimeter не был зафиксирован | Caller inventory и удаление Store bridge/introspection helpers |
| `COMP-003B1` | P2 | `[x] implemented` | Adapter/projector globals компилировались в default build | Quarantine и последующее физическое удаление |
| `COMP-003B2` | P2 | `[x] implemented` | Store/Search globals находились в implementation owners | Bounded owners и последующее физическое удаление |
| `COMP-003C1` | P2 | `[x] implemented` | Feature/test-only globals и legacy errors сохраняли process state | Feature, OnceLock owners, errors и test installation удалены |
| `COMP-003C2` | P2 | `[-] in progress` | Installer и no-composition shims сохраняли compatibility surface | C2A, C2B1, C2B2A/B/C закрыты, isolation matrix выполнена |
| `COMP-003C2A` | P2 | `[x] implemented` | State-free installer functions и `runtime_defaults::install()` оставались public | Все installer symbols/re-exports удалены; source enforcement и isolation test в `main` |
| `COMP-003C2B1` | P2 | `[x] implemented` | Dead no-composition Validate/Search wrappers оставались public | Removed wrappers и source enforcement в `main` |
| `COMP-003C2B2` | P2 | `[-] in progress` | Связанные service chains принимают optional/no composition | B2A/B/C завершены и fallback branches отсутствуют |
| `COMP-003C2B2A` | P2 | `[x] implemented` | Active Context owner был optional-composition и зависел от удалённого Search wrapper | Composition-first owner активен; Store/Search только через composition; behavior/source regressions в `main` |
| `COMP-003C2B2B` | P2 | `[-] in progress` | Operation-aware Context, daemon queries и `DaemonState` всё ещё используют optional composition | Mandatory composition в daemon lifecycle и derived reads; no fallback imports/branches |
| `COMP-003C2B2C` | P2 | `[ ] planned` | Index, Generation, Wiki и benchmark сохраняют no-composition APIs/projector fallbacks | Composition-only signatures, updated re-exports/tests/examples |
| `COMP-004` | P2 | `[x] implemented` | `validate_changed` использовал hidden adapter composition | Только composition-aware public path; execution pending |
| `COMP-005` | P2 | `[x] implemented` | Trust functions возвращали report со старым schema | Versioned public report |
| `COMP-006` | P2 | `[x] implemented` | Focused handlers устанавливали global runtime | Все active CLI families используют explicit composition |
| `COMP-007` | P2 | `[x] implemented` | RusTok operations использовали global store/context | Composition-aware operation family |
| `COMP-008` | P2 | `[x] implemented` | API/Docs/Repair использовали task-local store composition | Direct paths используют explicit composition |
| `COMP-009` | P1 | `[x] implemented` | После удаления `get_or_build_search_index` старый active Context owner продолжал вызывать удалённый symbol | Active module заменён; removed symbol отсутствует в compiled Context; source regression в `main` |
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
- [x] Legacy factory errors удалены.
- [x] Все `install_*` functions и `athanor_runtime_defaults::install()` удалены.
- [x] Installer symbols больше не re-exported из `athanor-app`.
- [x] Unit tests создают локальную `test_runtime::composition()` без installation.
- [x] Parallel isolation matrix покрывает разные Store/Search/Wiki/HTML factories.
- [x] Dead no-composition Validate/Search wrappers удалены.
- [x] Active Context owner использует mandatory composition для Store и Search.
- [ ] Operation-aware Context и daemon host используют mandatory composition.
- [ ] Index/Generation/Wiki/benchmark public APIs composition-only.

### Context owner replacement

- [x] `lib.rs` маршрутизирует `context` на `context_composition.rs`.
- [x] Active owner не импортирует `crate::store::init_store`.
- [x] Active owner не вызывает удалённый `get_or_build_search_index`.
- [x] Search factory вызывается через supplied composition.
- [x] Ranking, relation expansion, diagnostics и limits покрыты integration regressions.
- [ ] Старый `context.rs` физически удалён после миграции оставшихся callers.

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

**Сейчас:** `COMP-003C2B2B` — mandatory composition через operation-aware Context и daemon host.

Требования:

- заменить `context_project_with_operation_context_impl(..., Option<&RuntimeComposition>, ...)` на
  обязательный `&RuntimeComposition`;
- удалить no-composition `context_project_with_operation_context`;
- заменить `DaemonState.composition: Option<RuntimeComposition>` на обязательную composition;
- удалить `serve_daemon` compatibility entrypoint либо сделать единственный composition-required API;
- удалить fallback imports/branches `init_store` и no-composition Search из `daemon_queries`;
- убрать branching в `daemon_derived_read_dispatch` и `daemon.rs`;
- обновить source inventories, daemon tests и embedding documentation;
- физически удалить quarantined `context.rs`, когда все callers migrated.

**После него:** `COMP-003C2B2C`, затем `MCP-007`, `JSON-003`, documentation reconciliation и полный verification.

## 8. Журнал актуализаций

### 2026-07-18 — Composition-first Context owner

- Реализован `COMP-003C2B2A`.
- Обнаружен и зарегистрирован `COMP-009`: старый active Context owner вызывал symbol, удалённый в `C2B1`.
- Добавлен bounded `context_composition.rs`; `lib.rs` переключён на новый owner.
- Store и Search routing выполняются только через supplied `RuntimeComposition`.
- Старый 823-line `context.rs` quarantined вне compiled module graph до физического удаления.
- Добавлены source inventory и behavior regressions.
- `COMP-003C2B2B` назначен активным.
- Статус — implemented, execution pending.

### 2026-07-18 — Dead no-composition wrapper cleanup

- Реализован `COMP-003C2B1`.
- Удалён public `validate_changed`; остаётся `validate_changed_with_composition`.
- Удалён public `search_project`; остаются composition-aware variants.
- Удалены неиспользуемые `get_or_build_search_index` и `get_or_build_search_index_sync`.
- Source regression запрещает возврат удалённых wrappers.
- Статус — implemented, execution pending.

### 2026-07-18 — Runtime installer API removal

- Реализован `COMP-003C2A`.
- Удалены adapter, Store, Search, Wiki и HTML installer functions.
- Удалён `athanor_runtime_defaults::install()` и installer re-exports.
- Добавлены source regression и parallel composition isolation matrix.
- Статус — implemented, execution pending.

### 2026-07-17 — Process-global runtime removal

- Реализован `COMP-003C1`.
- Удалён Cargo feature, global owner files, legacy errors и test installation bootstrap.

### 2026-07-17 — Composition and architecture slices

- Реализованы `COMP-003A/B1/B2`, explicit CLI/MCP/RusTok/Graph composition, runtime/docs/store/search decomposition, publication hardening и CLI migration.
