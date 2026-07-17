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
| `ARCH-AUDIT-001` | P1 | `[-] in progress` | MCP, adapter contracts, explicit composition и publication-hardening slices реализованы; execution и remaining cleanup pending |
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
| `COMP-003` | P2 | `[-] in progress` | Store/Search/adapter/projector globals сохраняют process-wide hidden dependencies | Legacy globals удалены после parity и compatibility window |
| `COMP-004` | P2 | `[x] implemented` | `validate_changed` использовал `RuntimeBuilder::new` и hidden global adapter composition | Composition-aware API и focused CLI dispatcher находятся в `main`; execution pending |
| `COMP-005` | P2 | `[ ] planned` | Legacy trust functions в `runtime.rs` ещё возвращают report со старым shared schema | API deprecated/removed либо возвращает только versioned facade type |
| `COMP-006` | P2 | `[x] implemented` | Focused handlers устанавливали process-global runtime перед explicit-composition calls | Все active focused handlers используют explicit composition без external `install()`; execution pending |
| `COMP-007` | P2 | `[x] implemented` | RusTok operation APIs напрямую использовали global `init_store`/global context operation | Composition-aware RusTok audit/graph/context APIs и focused RusTok без global install; execution pending |
| `COMP-008` | P2 | `[-] in progress` | Application Report и Repair временно используют task-local store composition вокруг legacy service APIs | Isolation regression добавлен; выполнить его, внедрить прямые composition APIs и удалить compatibility wrappers после `CLI-001` |
| `PUB-001` | P1 | `[x] implemented` | Adapter trust writer мог вернуть failure после durable rename из-за backup cleanup | Cleanup best-effort warning после commit point |
| `PUB-002` | P1 | `[-] in progress` | Staged-replace helpers смешивали durable success и post-commit maintenance failure | Inventory ведётся в `docs/development/publication-semantics-inventory.md`; все открытые writers исправлены и regressions выполнены |
| `PUB-003` | P2 | `[x] implemented` | Не было системных failure-injection regressions для stage/publish/rollback/cleanup | Stage/build, rollback, post-commit cleanup, Drop, target-race и recovery/retention matrix находятся в `main`; execution pending |
| `PUB-004` | P1 | `[ ] planned` | Read-model/index-state finalizers, project registry и publication journal ещё возвращают backup cleanup failure после commit point | Cleanup best effort с warning; source/failure regressions; caller не получает ложный publication failure |
| `PUB-005` | P1 | `[!] blocked` | Оба JSONL pointer replacement helper возвращают ошибку, если backup deletion ломается после успешного rename | Безопасная полная правка большого `store.rs`/`lib.rs`, pointer regressions и store conformance |
| `CLI-001` | P2 | `[-] in progress` | `entry.rs` включает monolithic `main.rs`; grammar/rendering дублируются | Один root command model; compatibility includes удалены |
| `CLI-002` | P2 | `[x] implemented` | Plugin commands использовали legacy shared trust-report schema | Focused dispatcher и executable parity regression |
| `DOC-001` | P3 | `[-] in progress` | Документы содержат stale `verified` claims без current-HEAD evidence | Все затронутые документы имеют честный status/evidence |
| `DOC-002` | P3 | `[ ] planned` | Pipeline architecture содержит противоречия между implemented и planned sections | Current/target/history sections согласованы |
| `VERIFY-001` | P1 | `[!] blocked` | В текущей среде нет Rust toolchain, hosted checks для direct-to-main commits не видны | Выполнены fmt/tests/Clippy/workspace smoke и результаты записаны |
| `TOOL-001` | P2 | `[!] blocked` | GitHub contents connector не поддерживает точечный patch больших Rust-файлов; connector-triggered hosted workflows не дали наблюдаемого run/commit | Получена проверяемая полная копия blob, доступен безопасный patch/local-file action либо восстановлена наблюдаемая hosted execution |

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
- [x] Search, Context, Check, Graph, ValidateChanged, RusTok, Generation и Direct Read не вызывают external global `install()`.
- [x] Application Report API snapshot/docs propose-fix используют scoped explicit composition.
- [x] Repair recover-index/repair-latest используют scoped explicit composition; retention/cleanup-only paths не требуют store.
- [x] `entry.rs` не содержит `athanor_runtime_defaults::install()`.
- [x] Source regressions защищают active routing и facade composition paths.
- [x] Добавлен concurrent task-local isolation test: scope активен внутри каждой задачи и отсутствует после завершения.

Осталось:

- [ ] Выполнить source, targeted, isolation и executable regressions.
- [ ] Добавить executable regression для focused `validate-changed`.
- [ ] Заменить compatibility wrappers прямыми composition APIs при декомпозиции CLI.
- [ ] Удалить legacy globals после parity coverage (`COMP-003`).

### Publication semantics

Реализовано:

- [x] `PUB-001`: adapter trust backup cleanup после durable publish — warning.
- [x] Создан repository-wide inventory commit points и cleanup policies.
- [x] `publish_staged_directory` и `replace_output_file`: backup cleanup после commit point больше не меняет успешный результат.
- [x] Daemon endpoint больше не удаляет старый endpoint до публикации нового; использует общий atomic replacement.
- [x] Source-regression защищает inventory, общий helper, daemon endpoint и уже безопасные publishers.
- [x] Tantivy rebuild, cancellable projector output и immutable JSONL generation классифицированы как уже безопасные.
- [x] Repair retention/recovery классифицированы как intentionally strict destructive/recovery operations.
- [x] Failure regressions покрывают staged build failure, rollback restoration после commit-rename failure, Drop cleanup и target race.
- [x] Детерминированная `cfg(test)` инъекция подтверждает successful publication и warning при post-commit backup cleanup failure.
- [x] `publication_recovery_matrix.rs` защищает retention rollback, strict tombstone cleanup, partial recovery и fail-closed live-artifact conflicts.

Осталось:

- [ ] `PUB-004`: исправить read-model/index-state finalizers, project registry и publication journal.
- [!] `PUB-005`: исправить JSONL latest/snapshot-sequence helpers через безопасный patch большого файла.
- [ ] Выполнить publication inventory, failure matrix, projector, app, JSONL и workspace regressions.

### CLI decomposition

- [x] Plugin, ValidateChanged и остальные active command families маршрутизируются через focused dispatchers.
- [ ] Инвентаризировать remaining parser/render duplication.
- [ ] Ввести один root command model и удалить `include!("main.rs")`/compatibility includes после parity coverage.

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
cargo test -p athanor-app store_facade --locked
cargo test -p athanor-app graph_operation --locked
cargo test -p ath --test direct_plugin_cli --locked
cargo test -p ath -- direct_validate_changed_cli --locked
cargo test -p ath -- direct_graph_cli --locked
cargo test -p ath -- direct_rustok_composed_cli --locked
cargo test -p ath -- direct_generation_cli --locked
cargo test -p ath -- direct_read_composed_cli --locked
cargo test -p ath -- direct_application_report_composed_cli --locked
cargo test -p ath -- repair_composed_cli --locked
cargo test -p athanor-app --test publication_semantics_inventory --locked
cargo test -p athanor-app --test publication_recovery_matrix --locked
cargo test -p athanor-projector-support --test publication_failure_semantics --locked
cargo test -p athanor-projector-support --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p ath --test executable_shared_report_cli --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
```

Локальный Rust toolchain отсутствует. Hosted status для последних direct-to-main commits через доступный connector не отображается. Новые пакеты не помечаются `[x] verified` без execution evidence.

## Активный рабочий пакет

**Сейчас:** `PUB-003` отмечен `[x] implemented`: stage/build, rollback, post-commit maintenance, Drop, target-race и recovery/retention fault coverage находятся в `main`. Execution evidence отсутствует.

**Следом:** `PUB-004` — сделать cleanup best-effort в read-model/index-state finalizers, project registry и publication journal, начиная с файлов, для которых connector возвращает полные проверяемые blob’ы. `PUB-005`, `RUST-001`, `RUST-002` и `COMP-005` остаются ограничены `TOOL-001`.

## Журнал

### 2026-07-17 — Publication failure matrix implementation complete

- Добавлена детерминированная `cfg(test)` инъекция отказа backup cleanup после успешного commit rename.
- Unit test подтверждает `Ok`, новый visible target, сохранённый recovery backup и warning path.
- Repository-wide source enforcement защищает наличие fault hook и теста.
- `PUB-003` отмечен `[x] implemented`; execution evidence отсутствует.

### 2026-07-17 — Publication rollback and recovery matrix

- Добавлен rollback regression: после переноса previous target в backup провал commit rename восстанавливает предыдущую публикацию без временных siblings.
- Добавлен `publication_recovery_matrix.rs` для retention compensation, strict tombstone cleanup, partial recovery и fail-closed conflicts.
- Inventory и verification commands синхронизированы.
- `PUB-003` остаётся `[-] in progress`: post-commit cleanup injection и execution evidence отсутствуют.

### 2026-07-17 — Publication failure regression slice

- Добавлен `publication_failure_semantics.rs` с тремя fault-сценариями: build failure, Drop cleanup и target race.
- Inventory дополнен фактическим покрытием и оставшейся rollback/post-commit cleanup matrix.
- `PUB-003` переведён в `[-] in progress`; execution не выполнялась.
- Два временных hosted migration workflow не дали наблюдаемого run/commit через доступную интеграцию и удалены из `main`.
- `PUB-004` и `PUB-005` не помечены реализованными: соответствующие source-файлы не изменялись.

### 2026-07-17 — Publication durability inventory

- Инвентаризированы staged/publish/rename owners и для каждого указан commit point и cleanup policy.
- Общие directory/file replacement helpers больше не возвращают maintenance failure после успешного target rename.
- Daemon endpoint использует общий atomic replacement вместо delete-before-rename.
- Добавлены `PUB-004` для app-level finalizers/registry/journal и `PUB-005` для JSONL pointer writers.
- Добавлены inventory document и source-regression; execution не выполнялась.

### 2026-07-17 — Scoped composition isolation regression

- Добавлен concurrent `tokio::task_local` test для store composition scope.
- Обе параллельные задачи видят собственный активный scope.
- После join scope отсутствует в вызывающей задаче.
- `COMP-008` остаётся `[-] in progress` до execution и удаления compatibility bridge.

### 2026-07-17 — Eighth explicit-composition slice

- Repair active routing переведён на `repair_composed_cli`.
- `recover-index` и `repair-latest` получили explicit-composition facade.
- Retention и cleanup-only repair paths классифицированы как не требующие store factory.
- Entry-level global runtime installation удалён.
- `COMP-006` отмечен `[x] implemented`; execution остаётся неподтверждённой.

### 2026-07-17 — Seventh explicit-composition slice

- Store facade получил task-local scoped `RuntimeComposition` для compatibility services.
- API snapshot и docs propose-fix получили composition-aware facades.
- Application Report active routing переведён на composition-only wrapper.
- Добавлен `COMP-008` для контроля scope isolation и последующего удаления bridge.

### 2026-07-17 — Sixth explicit-composition slice

- Direct Read active routing переведён на `direct_read_composed_cli`.
- Context/Explain/Overview/Impact/ChangeMap/Search продолжают использовать composition-aware APIs.
- Compatibility body не устанавливает внешний runtime.

### 2026-07-17 — Fifth explicit-composition slice

- Direct Generation очищен от global install.
- Wiki, HTML и immutable generation используют composition-aware APIs.

### 2026-07-17 — Fourth explicit-composition slice

- Добавлен composition-aware RusTok facade для architecture context, audit и graph operations.
- Focused RusTok dispatcher переведён на production composition без global install.

### 2026-07-17 — Third explicit-composition slice

- Direct Check и standard Graph family очищены от global install.

### 2026-07-17 — Second explicit-composition slice

- Store/Search globals помещены за guarded facades.
- Добавлены composition-aware `validate_changed`, Search и Context focused paths.

### 2026-07-17 — First explicit-composition slice

- Adapter/projector conflicts стали explicit; empty-registry fallback удалён.

### 2026-07-17 — Adapter contract migration

- Разделены manifest, persisted trust registry и public trust report owners.

### 2026-07-17 — MCP hardening

- Bounded lifecycle, JSON-RPC semantics и initialization state реализованы; execution pending.
