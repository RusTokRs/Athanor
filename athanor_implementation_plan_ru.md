# Athanor: консолидированный план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-17  
> Статус: active architecture audit

## Правила статусов

- `[x] implemented` — исправление находится в `main`, но ещё не считается проверенным без выполнения regressions.
- `[x] verified` — реализация и regressions подтверждены фактически выполненными проверками.
- `[-] in progress` — полезный срез находится в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — работа не начата.
- `[!] blocked` — требуется compatibility decision, безопасный способ изменения или недоступная платформенная проверка.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id или protocol version. Эквивалентные CLI, daemon и MCP операции не должны иметь разные application payload shapes.

## Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | MCP lifecycle/composition, adapter contracts, publication hardening, JSONL store и bounded CLI root реализованы; Docs service composition, execution и remaining cleanup pending |
| `DS-JSON-001` | P1 | `[-] in progress` | Public registry 60; current adapter owners разделены, legacy library API остаётся |
| `DS-JSON-002` | P1 | `[-] in progress` | 30 general + 4 adapter-specific non-public descriptors; execution и repeat scan pending |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed CLI/daemon/MCP/plugin payload parity реализована; execution pending |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |

## Реестр всех подтверждённых дефектов и рисков

| ID | Priority | Status | Подтверждённая проблема | Критерий закрытия |
| --- | --- | --- | --- | --- |
| `MCP-001` | P1 | `[x] implemented` | Response path использовал unbounded channel | Bounded queue, single writer и выполненные MCP regressions |
| `MCP-002` | P1 | `[x] implemented` | Requests spawn-ились без лимита, completed tasks жили до EOF | In-flight cap, continuous reap и выполненные load/lifecycle tests |
| `MCP-003` | P1 | `[x] implemented` | Неверные JSON-RPC version/error/id/session semantics | Exact `2.0`, standard codes, distinct omitted/null id, session-state tests |
| `MCP-004` | P2 | `[ ] planned` | При полной saturation ordinary request limit может задерживать cancellation notification/control-plane input | Отдельный control-plane path либо доказанная responsiveness regression |
| `MCP-005` | P1 | `[x] implemented` | Transport runtime включал legacy tool dispatcher, вызывавший global app APIs; CLI локально устанавливал process-global runtime | `RuntimeComposition` проходит через server/request/tool lifecycle; legacy include/dispatcher и MCP-local install удалены; source regressions в `main`; execution pending |
| `JSON-001` | P1 | `[x] implemented` | Current adapter manifest имел неверсионированный schema id | `athanor.adapter_manifest.v1`; legacy id только migration input |
| `JSON-002` | P1 | `[-] in progress` | `athanor.adapter_trust.v2` владел persisted registry и public report | Current paths разделены; legacy library emitters удалены/deprecated |
| `JSON-003` | P1 | `[-] in progress` | Inventory ошибочно считался complete без adapter boundaries | Adapter registry/fixtures добавлены; repeat scan и execution завершены |
| `JSON-004` | P2 | `[x] implemented` | Adapter contract symbols были re-exported двумя facade paths | Один canonical re-export path и Clippy без ambiguous re-export warnings |
| `RUST-001` | P1 | `[!] blocked` | Unix test `ProcessCommand` literal дважды задаёт `clear_environment` | Безопасная замена `runtime.rs`, source guard и Rust compile |
| `RUST-002` | P1 | `[!] blocked` | Non-Windows helper не задаёт обязательный `clear_environment` | Безопасная замена `runtime.rs`, source guard и Rust compile |
| `COMP-001` | P2 | `[x] implemented` | `OnceLock::set` conflicts молча игнорировались | Adapter/projector/Store/Search guarded APIs находятся в `main`; execution pending |
| `COMP-002` | P1 | `[x] implemented` | Отсутствующий adapter factory молча создавал empty registry | Legacy path fail-fast; explicit composition остаётся штатным path |
| `COMP-003` | P2 | `[-] in progress` | Store/Search/adapter/projector globals сохраняют process-wide hidden dependencies | Legacy globals удалены после parity и compatibility window |
| `COMP-004` | P2 | `[x] implemented` | `validate_changed` использовал `RuntimeBuilder::new` и hidden global adapter composition | Composition-aware API и focused CLI dispatcher находятся в `main`; execution pending |
| `COMP-005` | P2 | `[ ] planned` | Legacy trust functions в `runtime.rs` ещё возвращают report со старым shared schema | API deprecated/removed либо возвращает только versioned facade type |
| `COMP-006` | P2 | `[x] implemented` | Focused handlers устанавливали process-global runtime перед explicit-composition calls | Все active CLI families, включая MCP, создают/получают explicit composition без global install; execution pending |
| `COMP-007` | P2 | `[x] implemented` | RusTok operation APIs напрямую использовали global `init_store`/global context operation | Composition-aware RusTok audit/graph/context APIs и focused RusTok без global install; execution pending |
| `COMP-008` | P2 | `[-] in progress` | API/Docs/Repair facades использовали task-local store composition вокруг legacy service internals | API snapshot/registry и Repair recovery/latest переведены на прямой `composition.init_store`; перенести Docs check/drift/propose/apply, выполнить behavioral parity/isolation regressions и удалить последний task-local bridge |
| `PUB-001` | P1 | `[x] implemented` | Adapter trust writer мог вернуть failure после durable rename из-за backup cleanup | Cleanup best-effort warning после commit point |
| `PUB-002` | P1 | `[x] implemented` | Staged-replace helpers смешивали durable success и post-commit maintenance failure | Все подтверждённые writers имеют explicit commit point и non-fatal maintenance path; execution pending |
| `PUB-003` | P2 | `[x] implemented` | Не было системных failure-injection regressions для stage/publish/rollback/cleanup | Stage/build, rollback, post-commit cleanup, Drop, target-race и recovery/retention matrix находятся в `main`; execution pending |
| `PUB-004` | P1 | `[x] implemented` | Read-model/index-state finalizers, project registry и publication journal возвращали backup cleanup failure после commit point | Все четыре owner логируют warning и сохраняют successful publication; execution pending |
| `PUB-005` | P1 | `[x] implemented` | JSONL latest и snapshot-sequence имели duplicate replacement helpers с propagating backup cleanup failure | Один `pointer_publication` owner; cleanup best effort, invalid target fail-closed, regressions в `main`; execution pending |
| `STORE-001` | P1 | `[x] implemented` | `athanor-store-jsonl` был цепочкой `strict_latest -> include!(lib) -> include!(store)` с файлами до 1286 строк | Conventional crate root, explicit modules <=300 lines, no compatibility includes, public API preserved; execution pending |
| `CLI-001` | P2 | `[x] implemented` | Корень CLI включал 4k-line `main.rs`, legacy fallback и дублирующую grammar/rendering | `main.rs` и последний include удалены; все команды принадлежат typed bounded root/family models; execution parity pending |
| `CLI-002` | P2 | `[x] implemented` | Plugin commands использовали legacy shared trust-report schema | Focused dispatcher и executable parity regression |
| `CLI-003` | P2 | `[x] implemented` | Direct Read, Application Report и Repair использовали include wrappers, namespace shadowing и файлы до 542 строк | Explicit model/run/render modules, direct composition calls, no family includes/shadows, bounded source regressions; execution pending |
| `CLI-004` | P2 | `[x] implemented` | Graph/RusTok/Check зависели от renderer bridges в legacy namespace, а entry вручную повторял порядок parser dispatch | Dedicated Graph/RusTok/Check/API renderers, typed root command model, no renderer bridges, bounded source regression; execution pending |
| `CLI-005` | P2 | `[x] implemented` | Legacy-only Index/Docs/API/Projects/Analysis/MCP grammar оставалась внутри удаляемого monolith | Dedicated command families, root help/version/unknown handling, tracing и 8 MiB stack сохранены; execution pending |
| `DOC-001` | P3 | `[-] in progress` | Документы содержат stale `verified` claims без current-HEAD evidence | Все затронутые документы имеют честный status/evidence |
| `DOC-002` | P3 | `[ ] planned` | Pipeline architecture содержит противоречия между implemented и planned sections | Current/target/history sections согласованы |
| `VERIFY-001` | P1 | `[!] blocked` | В текущей среде нет Rust toolchain; hosted execution evidence неполно | Выполнены fmt/tests/Clippy/workspace smoke и результаты записаны |
| `TOOL-001` | P2 | `[!] blocked` | Connector не поддерживает точечный patch особенно большого `runtime.rs` | Получен безопасный patch/local-file action либо полностью проверяемый blob для оставшегося runtime файла |
| `TOOL-002` | P2 | `[x] implemented` | Contents connector не позволял безопасно изменить 4k-line CLI monolith точечным patch | Ограничение обойдено полным переносом command families; `apps/ath/src/main.rs` удалён без compatibility shim |

## ARCH-AUDIT-001 — Repository-wide architecture audit

**Статус:** `[-] in progress`. Рабочий tracker: GitHub issue `#16`.

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

Реализовано:

- [x] Bounded response queue: 32.
- [x] Ordinary in-flight limit: 32.
- [x] Continuous `JoinSet` reap, one stdout writer, EOF drain/join.
- [x] Exact JSON-RPC `2.0`, standard error categories, MCP `2024-11-05` state machine.
- [x] Omitted id отделён от explicit `null`; cancellation/deadline cleanup сохранён.
- [x] `RuntimeComposition` передаётся из CLI composition root через `Arc` в каждую request task.
- [x] Tool schemas и dispatcher разделены на bounded `tools/schema.rs` и `tools/dispatch.rs`.
- [x] Все восемь MCP tools вызывают composition-aware application APIs.
- [x] Legacy `src/lib.rs`, `include!("lib.rs")` и MCP-local global install удалены.
- [x] Source inventories проверяют explicit composition, отсутствие legacy calls и module bounds.

Осталось:

- [ ] Выполнить MCP tests, fmt, Clippy и client interoperability.
- [ ] Решить `MCP-004`.
- [ ] Решить notification cancellation для transactional Index после rollback/durable-success review.

### Adapter JSON boundary migration

Реализовано:

- [x] `athanor.adapter_manifest.v1`; legacy unversioned id — migration input.
- [x] `athanor.adapter_trust_registry.v2`; legacy `athanor.adapter_trust.v2` нормализуется перед write.
- [x] `VersionedAdapterTrustReport` / `athanor.adapter_trust_report.v1`.
- [x] Public registry: 60 owners; adapter non-public registry: 2 current + 2 legacy inputs.
- [x] Fixtures, lifecycle/disjointness/source regressions и focused Plugin CLI.
- [x] Trust backup cleanup после durable publish — best effort.

Осталось:

- [ ] Закрыть `JSON-002`/`COMP-005`.
- [ ] Выполнить targeted/executable regressions.
- [ ] Повторить repository-wide schema scan.

### Подтверждённые compile-level defects

- [!] `RUST-001`: duplicate `clear_environment`.
- [!] `RUST-002`: missing `clear_environment`.
- [ ] Исправить через гарантированную полную копию большого `runtime.rs` и добавить source guard.

### Explicit composition migration

Реализовано:

- [x] Typed install/unavailable errors и общий `install_once`/`require_installed`.
- [x] Guarded adapter registry/resolver, Wiki/HTML projectors, Store, normal Search и operation-aware Search.
- [x] Silent empty adapter registry fallback удалён.
- [x] `validate_changed_with_composition` и focused `validate-changed` dispatcher.
- [x] Composition-aware variants для standard Graph и RusTok operation families.
- [x] Search, Context, Check, Graph, ValidateChanged, RusTok, Generation, Direct Read, Index/Bench, API, Analysis и MCP используют explicit composition.
- [x] API snapshot/registry direct services и Repair recovery/latest direct services вызывают `composition.init_store` без task-local scope.
- [x] `entry.rs` и active CLI families не содержат `athanor_runtime_defaults::install()`.
- [x] MCP transport не зависит от runtime-defaults crate; production composition остаётся обязанностью CLI composition root.
- [x] Source regressions защищают active routing, API/Repair direct services, MCP composition и оставшийся Docs bridge inventory.

Осталось:

- [ ] Перенести Docs check/drift/propose/apply с task-local scope на прямые service internals (`COMP-008`).
- [ ] Выполнить behavioral parity/isolation regressions для API/Docs/Repair direct paths.
- [ ] Выполнить source, targeted и executable regressions.
- [ ] Добавить executable regression для focused `validate-changed`.
- [ ] Удалить legacy globals после parity coverage (`COMP-003`).

### Publication semantics

Реализовано:

- [x] `PUB-001`–`PUB-005`: inventory, common helpers, app finalizers и JSONL pointer writers исправлены.
- [x] Rename нового target является commit point; последующий backup cleanup не меняет successful result.
- [x] Invalid non-file pointer target fail-closed до commit.
- [x] Failure regressions покрывают stage, rollback, cleanup, Drop, target race и recovery matrix.
- [x] JSONL latest и snapshot sequence используют один `pointer_publication` helper.

Осталось:

- [ ] Выполнить publication inventory, failure matrix, projector, app, JSONL conformance и workspace regressions.

### JSONL store modularization

Реализовано:

- [x] Удалён `strict_latest.rs` wrapper и оба `include!` слоя.
- [x] `lib.rs` стал conventional module root.
- [x] Выделены bounded production modules.
- [x] Public API `JsonlKnowledgeStore`/`PathIndex*` сохранён.
- [x] Strict latest, compatibility, repair normalization, sequence allocation и indexes покрыты regressions.

Осталось:

- [ ] Выполнить JSONL unit, integration и shared conformance tests.

### CLI decomposition

Реализовано:

- [x] `apps/ath/src/main.rs` и `include!("main.rs")` удалены.
- [x] `entry.rs` содержит только bootstrap, tracing, runtime creation и typed dispatch; 8 MiB CLI stack сохранён.
- [x] `root_command.rs` единолично владеет всеми command families, root help/version и unknown-command behavior.
- [x] Index/Bench/Update, Docs, API, Projects, Analysis и MCP выделены в отдельные typed modules.
- [x] Wiki/HTML/Generate имеют одного владельца grammar в Generation family.
- [x] MCP CLI создаёт production composition и вызывает explicit transport entry point без global bootstrap.
- [x] Direct Read, Repair, Graph, Check, API diff и RusTok разделены на bounded model/run/render modules.
- [x] Source enforcement запрещает family include, namespace shadowing, root legacy fallback и renderer bridges.

Осталось:

- [ ] Выполнить parser/help/version/unknown-command regressions.
- [ ] Выполнить JSON/text rendering и executable parity regressions.
- [ ] Выполнить MCP stdio/tool interoperability regressions.

### Documentation reconciliation

- [x] MCP transport и direct-operation docs обновлены под explicit composition и честный pending-execution status.
- [x] Затронутые adapter/publication/CLI contract docs приведены к честному current status.
- [ ] `DOC-001`: проверить остальные `status: verified` и stale `apps/ath/src/main.rs` references.
- [ ] `DOC-002`: согласовать current/target/history pipeline sections.

## DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`.

- [x] Contract trait/validator и unique owner registry.
- [x] 60 current public Athanor owners.
- [x] Adapter trust public report имеет отдельный current owner.
- [x] Shared CLI/daemon report types для Index, Context, Generation, Wiki и HTML.
- [ ] Legacy adapter trust report API cleanup.
- [ ] Targeted/workspace verification.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

- [x] Four-entry MCP registry; four schema-less process protocols.
- [x] 30-entry general non-public registry.
- [x] Four-entry adapter non-public registry.
- [x] Adapter migration/disjointness/source enforcement.
- [ ] Repeat schema scan и execution всех enforcement tests.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`: implementation/regressions complete, execution pending.

- [x] Shared Index/Context/Generation/Wiki/HTML types.
- [x] Direct Plugin CLI uses `VersionedAdapterTrustReport`.
- [x] Executable regressions cover shared reports/plugin migration.
- [ ] Execute parity regressions.

## Verification

```bash
cargo fmt --all -- --check
cargo test -p athanor-transport-mcp --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo test -p athanor-app --test mcp_runtime_bootstrap_inventory --locked
cargo test -p athanor-app --test service_composition_inventory --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
cargo test -p athanor-app adapter_contract --locked
cargo test -p athanor-app --test adapter_contract_inventory --locked
cargo test -p athanor-app store_facade --locked
cargo test -p athanor-app graph_operation --locked
cargo test -p ath -- root_command --locked
cargo test -p ath -- index_cli --locked
cargo test -p ath -- docs_cli --locked
cargo test -p ath -- api_cli --locked
cargo test -p ath -- analysis_cli --locked
cargo test -p ath -- direct_validate_changed_cli --locked
cargo test -p ath -- direct_graph_cli --locked
cargo test -p ath -- rustok_cli --locked
cargo test -p ath -- direct_generation_cli --locked
cargo test -p ath -- direct_read --locked
cargo test -p ath -- repair --locked
cargo test -p ath --test direct_plugin_cli --locked
cargo test -p athanor-app --test publication_semantics_inventory --locked
cargo test -p athanor-app --test publication_recovery_matrix --locked
cargo test -p athanor-projector-support --test publication_failure_semantics --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p ath --test executable_shared_report_cli --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- --help
cargo run -p ath --quiet --locked -- index .
```

Локальный Rust toolchain отсутствует. Новые пакеты не помечаются `[x] verified` без execution evidence на том же commit.

## Активный рабочий пакет

**Сейчас:** `COMP-008` остаётся `[-] in progress`: API snapshot/registry и Repair recovery/latest переведены на прямые service composition modules; Docs — единственный оставшийся task-local Store bridge. Execution evidence отсутствует.

**Следом:** вынести Docs snapshot loading/build/apply internals в direct composition boundary, затем выполнить behavioral parity/isolation regressions. Параллельно остаются `MCP-004`, transactional Index cancellation review и Rust-enabled verification.

## Журнал

### 2026-07-17 — Direct API and Repair service composition

- Добавлены bounded `application_report_composition/api_direct.rs` и `repair_composition/direct.rs`.
- API snapshot/registry и Repair recovery/latest вызывают `composition.init_store` напрямую.
- Для Repair сохранён lock-before-store-init ordering; для API registry сохранён first-handler resolution и deterministic ordering.
- `application_report_composition.rs` использует task-local Store scope только для четырёх Docs operations.
- `service_composition_inventory.rs` запрещает возврат API/Repair к task-local/global store lookup и фиксирует module bounds.
- `COMP-008` остаётся in progress до direct Docs internals и behavioral execution evidence.

### 2026-07-17 — MCP explicit composition migration

- `RuntimeComposition` передаётся из `mcp_cli.rs` в transport server и через `Arc` клонируется в каждую request task.
- Добавлены conventional `tools/schema.rs` и `tools/dispatch.rs`; dispatcher вызывает composition-aware app APIs для всех восьми MCP tools.
- Удалены legacy dispatcher, textual include и MCP-local global install.
- Transport crate не зависит от runtime-defaults; production composition остаётся обязанностью CLI composition root.
- `MCP-005` и `COMP-006` отмечены `[x] implemented`; Rust execution evidence отсутствует.

### 2026-07-17 — Complete CLI root migration

- Все legacy-only grammars перенесены в bounded typed modules.
- Удалены `apps/ath/src/main.rs`, последний `include!("main.rs")` и legacy fallback.
- `CLI-001`, `CLI-005` и `TOOL-002` отмечены `[x] implemented`; execution pending.

### 2026-07-17 — Root command and renderer boundary

- Graph, Check и API diff переведены с legacy bridges на dedicated renderer modules.
- RusTok разделён на model/run/render; `CLI-004` отмечен `[x] implemented`.

### 2026-07-17 — JSONL store and publication slices

- JSONL store переведён на conventional bounded modules.
- Publication commit-point, rollback и post-commit cleanup semantics исправлены.
- `PUB-002`–`PUB-005` и `STORE-001` отмечены `[x] implemented`; execution pending.

### 2026-07-17 — Adapter contract migration

- Разделены manifest, persisted trust registry и public trust report owners.
