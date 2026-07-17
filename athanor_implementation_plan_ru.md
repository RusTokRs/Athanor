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
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | MCP, adapter contracts и шесть explicit-composition slice реализованы; execution/cleanup pending |
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
| `JSON-001` | P1 | `[x] implemented` | Current adapter manifest имел неверсионированный schema id | `athanor.adapter_manifest.v1`; legacy id только migration input |
| `JSON-002` | P1 | `[-] in progress` | `athanor.adapter_trust.v2` владел persisted registry и public report | Current paths разделены; legacy library emitters удалены/deprecated |
| `JSON-003` | P1 | `[-] in progress` | Inventory ошибочно считался complete без adapter boundaries | Adapter registry/fixtures добавлены; repeat scan и execution завершены |
| `JSON-004` | P2 | `[x] implemented` | Adapter contract symbols были re-exported двумя facade paths | Один canonical re-export path и Clippy без ambiguous re-export warnings |
| `RUST-001` | P1 | `[!] blocked` | Unix test `ProcessCommand` literal дважды задаёт `clear_environment` | Безопасная замена `runtime.rs`, source guard и Rust compile |
| `RUST-002` | P1 | `[!] blocked` | Non-Windows helper не задаёт обязательный `clear_environment` | Безопасная замена `runtime.rs`, source guard и Rust compile |
| `COMP-001` | P2 | `[x] implemented` | `OnceLock::set` conflicts молча игнорировались | Adapter/projector/Store/Search guarded APIs находятся в `main`; execution pending |
| `COMP-002` | P1 | `[x] implemented` | Отсутствующий adapter factory молча создавал empty registry | Legacy path fail-fast; explicit composition остаётся штатным path |
| `COMP-003` | P2 | `[-] in progress` | Store/Search/adapter/projector globals сохраняют process-wide hidden dependencies | Production entrypoints используют `RuntimeComposition`; globals удалены после parity |
| `COMP-004` | P2 | `[x] implemented` | `validate_changed` использовал `RuntimeBuilder::new` и hidden global adapter composition | Composition-aware API и focused CLI dispatcher находятся в `main`; execution pending |
| `COMP-005` | P2 | `[ ] planned` | Legacy trust functions в `runtime.rs` ещё возвращают report со старым shared schema | API deprecated/removed либо возвращает только versioned facade type |
| `COMP-006` | P2 | `[-] in progress` | Focused handlers создают composition, но часть всё ещё вызывает legacy `athanor_runtime_defaults::install()` | Search/Context/Check/Graph/ValidateChanged/RusTok/Generation/Direct Read очищены; Application Report и Repair migrated/classified |
| `COMP-007` | P2 | `[x] implemented` | RusTok operation APIs напрямую использовали global `init_store`/global context operation и не принимали composition | Composition-aware Rustok audit/graph/context APIs и focused RusTok без global install находятся в `main`; execution pending |
| `PUB-001` | P1 | `[x] implemented` | Adapter trust writer мог вернуть failure после durable rename из-за backup cleanup | Cleanup best-effort warning после commit point |
| `PUB-002` | P1 | `[ ] planned` | Другие staged-replace helpers могут смешивать durable success и post-commit maintenance failure | Полный inventory durability points и исправленные helpers |
| `PUB-003` | P2 | `[ ] planned` | Нет системных failure-injection regressions для stage/publish/rollback/cleanup | Tests для каждой фазы и repair-visible outcomes |
| `CLI-001` | P2 | `[-] in progress` | `entry.rs` включает monolithic `main.rs`; grammar/rendering дублируются | Один root command model; `include!("main.rs")` удалён |
| `CLI-002` | P2 | `[x] implemented` | Plugin commands использовали legacy shared trust-report schema | Focused dispatcher и executable parity regression |
| `DOC-001` | P3 | `[-] in progress` | Документы содержат stale `verified` claims без current-HEAD evidence | Все затронутые документы имеют честный status/evidence |
| `DOC-002` | P3 | `[ ] planned` | Pipeline architecture содержит противоречия между implemented и planned sections | Current/target/history sections согласованы |
| `VERIFY-001` | P1 | `[!] blocked` | В текущей среде нет Rust toolchain, hosted checks для direct-to-main commits не видны | Выполнены fmt/tests/Clippy/workspace smoke и результаты записаны |
| `TOOL-001` | P2 | `[!] blocked` | GitHub contents connector не поддерживает точечный patch большого `runtime.rs` | Получена проверяемая полная копия blob либо доступен безопасный patch path |

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

Осталось:

- [ ] Выполнить MCP tests, fmt, Clippy и client interoperability.
- [ ] Решить `MCP-004`.

### Adapter JSON boundary migration

Реализовано:

- [x] `athanor.adapter_manifest.v1`; legacy unversioned id — migration input.
- [x] `athanor.adapter_trust_registry.v2`; legacy `athanor.adapter_trust.v2` нормализуется перед write.
- [x] `VersionedAdapterTrustReport` / `athanor.adapter_trust_report.v1`.
- [x] Public registry: 60 owners; adapter non-public registry: 2 current + 2 legacy inputs.
- [x] Fixtures, lifecycle/disjointness/source regressions и focused Plugin CLI.
- [x] Trust backup cleanup после durable publish — best effort.
- [x] Adapter documentation examples и stale current-HEAD status claims исправлены.

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
- [x] Composition-aware variants для шести standard Graph operation APIs.
- [x] Composition-aware variants для Rustok architecture context, трёх audit и девяти graph operations.
- [x] Focused Search, Context, Check, Graph, ValidateChanged, RusTok, Generation и Direct Read не вызывают внешний global `install()`.
- [x] Direct Read active routing использует composition-only compatibility wrapper; parser/rendering остаются неизменными до `CLI-001`.
- [x] Source regression защищает facade routing и composition-only handlers.

Осталось:

- [ ] Закрыть `COMP-006`: Application Report требует composition-aware API snapshot/docs propose-fix paths; Repair требует отдельной проверки и composition-aware repair APIs до удаления entry-level install.
- [ ] Выполнить Rustok/Generation/Direct Read source и executable regressions.
- [ ] Добавить executable regression для focused `validate-changed`.
- [ ] Удалить legacy globals и compatibility wrappers после parity coverage.

### Publication semantics

- [x] `PUB-001` закрыт в implementation.
- [ ] `PUB-002`: inventory остальных staged-replace helpers и durability points.
- [ ] `PUB-003`: failure-injection tests для stage/publish/rollback/cleanup.

### CLI decomposition

- [x] Plugin и ValidateChanged command families вынесены в focused dispatchers.
- [ ] Инвентаризировать remaining parser/render duplication.
- [ ] Ввести один root command model и удалить `include!("main.rs")` после parity coverage.

### Documentation reconciliation

- [x] Затронутые adapter contract docs приведены к честному current status.
- [ ] `DOC-001`: проверить остальные `status: verified`.
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
cargo test -p athanor-app adapter_contract --locked
cargo test -p athanor-app --test adapter_contract_inventory --locked
cargo test -p athanor-app legacy_factory --locked
cargo test -p athanor-app --test legacy_factory_migration --locked
cargo test -p athanor-app graph_operation --locked
cargo test -p ath --test direct_plugin_cli --locked
cargo test -p ath -- direct_validate_changed_cli --locked
cargo test -p ath -- direct_graph_cli --locked
cargo test -p ath -- direct_rustok_composed_cli --locked
cargo test -p ath -- direct_generation_cli --locked
cargo test -p ath -- direct_read_composed_cli --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p ath --test executable_shared_report_cli --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
```

Локальный Rust toolchain отсутствует. Hosted status для последних direct-to-main commits через доступный connector не отображается. Новые пакеты не помечаются `[x] verified` без execution evidence.

## Активный рабочий пакет

**Сейчас:** Direct Generation и active Direct Read routing используют explicit `RuntimeComposition`; внешний process-global runtime не устанавливается. Старый Direct Read body оставлен compatibility include до удаления CLI monolith. Execution evidence отсутствует.

**Следом:** закончить `COMP-006` для Application Report и Repair; затем безопасный repair `runtime.rs`, legacy trust API cleanup и publication inventory.

## Журнал

### 2026-07-17 — Sixth explicit-composition slice

- Direct Read active routing переведён на `direct_read_composed_cli`.
- Все Context/Explain/Overview/Impact/ChangeMap/Search операции продолжают использовать существующие composition-aware APIs.
- Compatibility body не устанавливает внешний runtime: wrapper локально поглощает исторический installer call и передаёт только production composition.
- Source regression фиксирует active routing и запрещает возврат external global install.
- `COMP-006` остаётся `[-] in progress`: Application Report и Repair ещё открыты, verification не выполнялась.

### 2026-07-17 — Fifth explicit-composition slice

- Direct Generation очищен от global install без изменения command grammar или report rendering.
- Wiki, HTML и immutable generation продолжают использовать существующие composition-aware application APIs.
- Source regression добавляет Generation в список composition-only focused handlers.
- `COMP-006` остаётся `[-] in progress`: Direct Read и Application Report ещё открыты, verification не выполнялась.

### 2026-07-17 — Fourth explicit-composition slice

- Добавлен отдельный composition-aware RusTok application facade для architecture context, audit и graph operations.
- Store создаётся через `RuntimeComposition::init_store`, nested context — через composition-aware operation API.
- Focused RusTok dispatcher переведён на `athanor_runtime_defaults::production()` без global `install()`.
- Source regression защищает весь RusTok operation family и active CLI routing.
- `COMP-007` отмечен `[x] implemented`; verification остаётся заблокированной отсутствующим Rust toolchain/hosted status.

### 2026-07-17 — Third explicit-composition slice

- Direct Check очищен от global install.
- Standard Graph operation family получила composition-aware variants.
- Direct Graph переведён на explicit composition.
- RusTok global dependency выделена как `COMP-007`, а не скрыта ложным completion claim.

### 2026-07-17 — Second explicit-composition slice

- Store/Search globals помещены за guarded facades.
- Добавлены composition-aware `validate_changed` и focused dispatcher.
- Direct Search/Context/ValidateChanged очищены от global install.

### 2026-07-17 — Consolidated defect registry

- Все подтверждённые дефекты, риски, tool limitations и verification blockers получили ID и критерий закрытия.

### 2026-07-17 — First explicit-composition slice

- Adapter/projector conflicts стали explicit; empty-registry fallback удалён.

### 2026-07-17 — Adapter contract migration

- Разделены manifest, persisted trust registry и public trust report owners.

### 2026-07-17 — MCP hardening

- Bounded lifecycle, JSON-RPC semantics и initialization state реализованы; execution pending.
