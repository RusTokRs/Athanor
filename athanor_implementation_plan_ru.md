# Athanor: консолидированный план реализации

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-17  
> Статус: active verification plan

## Правила статусов

- `[x] verified` — реализация и regressions подтверждены выполненными проверками.
- `[-] in progress` — полезный срез находится в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — работа не начата.
- `[!] blocked` — требуется compatibility decision или недоступная платформенная проверка.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id или protocol version. Эквивалентные CLI, daemon и MCP операции не должны иметь разные application payload shapes.

## Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |
| `DS-JSON-001` | P1 | `[-] in progress` | 59 current Athanor owners; implementation inventory завершена; verification pending |
| `DS-JSON-002` | P1 | `[-] in progress` | Application, transport, process и persistence inventory реализованы; test execution pending |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed payload parity и executable regressions реализованы; execution pending |

## DS-RESOLVE-003 — Validated Artifact Resolver Migration

**Статус:** `[x] verified`.

- [x] Generated read-model и index-state paths разрешаются pointer-first.
- [x] Path/type/schema/snapshot/generation/checksum identity проверяется до использования.
- [x] Runtime consumers используют shared resolver boundary.
- [x] Rustok resolver coverage сохранена после runtime decomposition.

## DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`: implementation scope закрыт, полный verification недоступен в текущей среде.

### Contract foundation

- [x] App-layer `json_contract` facade и trait `VersionedJsonContract`.
- [x] Canonical schema-id validator и typed `JsonContractError`.
- [x] Проверка top-level object/string `schema` и instance/associated-constant equality.
- [x] Registry отклоняет duplicate schema id и duplicate Rust owner.
- [x] Standard, schema-less и non-public protocol registries отделены от public Athanor schema registry.

### Зарегистрированные public families

- [x] Overview/Search, Explain/Impact, Check, Coverage/Capabilities, ChangeMap и Context.
- [x] Core Graph, Project Registry/Resolution, Architecture и specialized RusTok families.
- [x] Index, Benchmark, Changed Validation и Generation.
- [x] Config Validate/Doctor.
- [x] Docs Check/Drift/Apply/Propose Fix.
- [x] API Snapshot/Diff/Cleanup.
- [x] Wiki и HTML.
- [x] Девять Repair report owners.
- [x] Daemon Request v3, Response v3 и Jobs v1.
- [x] Всего 59 current Athanor-schema type реализуют общий contract trait.

### Transport, process и non-public boundaries

- [x] MCP JSON-RPC `2.0` и protocol `2024-11-05` учтены отдельным four-entry registry.
- [x] Source-discover, extractor, linker и checker учтены как schema-less typed protocols.
- [x] Process framing закреплён: newline-terminated JSON stdin и один JSON stdout document.
- [x] `NON_PUBLIC_JSON_CONTRACTS` содержит 30 schemas: 24 current, 5 accepted legacy inputs и 1 historical-only.
- [x] Persisted inventory: registry/daemon endpoint, index-current/state/publication, canonical manifest/latest/commit и recovery journal.
- [x] Generated inventory: validation, generation/current, API snapshot/latest, JSONL/Wiki/HTML manifests.
- [x] Interchange inventory: Docs patch и Wiki/HTML projection payloads.
- [x] Embedded inventory: Index/Generation metrics; schema-less Repair/Daemon fragments остаются вне schema registry.
- [x] Daemon endpoint v2 классифицирован как accepted legacy input; v1 — historical-only и текущим reader отклоняется.
- [x] Filesystem locks, staging/backups, cleanup tombstones и repair guards классифицированы как filesystem recovery protocols.

### Compatibility migrations

- [x] Additive wrappers сохраняют прежние application fields.
- [x] CLI/daemon общие Index, Generation, Wiki и HTML results используют typed reports.
- [x] Direct Config, API Snapshot и Docs Propose Fix используют typed/versioned reports.
- [x] Direct Generate, Wiki и Report Html получили additive `--json`; прежний human output сохранён.
- [x] Repair JSON shapes зарегистрированы без изменения существующего CLI output.
- [x] Daemon v1/v2 request compatibility остаётся accepted input, но не current ownership.
- [x] MCP outer protocol не получает synthetic `athanor.*` schemas; inner text сохраняет schema application report.
- [x] Legacy persisted inputs нормализуются перед current write.

### Golden и integration regressions

- [x] Existing application, Graph, RusTok, Config, Docs, API, Wiki/HTML и Repair fixtures.
- [x] Daemon transport fixture для current request/success/error/jobs shapes.
- [x] MCP standard-protocol fixture, registry uniqueness и fail-closed validators.
- [x] Process/persistence fixture для 24 current documents и четырёх process protocols.
- [x] Boundary regression проверяет disjoint sets, required fields, runtime source observability, type usage и framing.
- [x] Extended public registry regression защищает 59 schema id и ownership.
- [x] `executable_shared_report_cli` запускает реальные Index/Context/Generate/Wiki/HTML commands и проверяет schemas/shared snapshot; execution pending.
- [!] Локальный Rust toolchain отсутствует; fmt/test/Clippy не заявляются выполненными.
- [!] Hosted Actions/check status для `main` не виден через connector.

### Следующий срез

- [x] Versioned application outputs и direct CLI migrations.
- [x] Repair public outputs и embedded state inventory.
- [x] Daemon request/response/error envelopes.
- [x] MCP JSON-RPC и tool-content envelopes.
- [x] Extractor/linker/checker/source process protocols.
- [x] Index/publication/read-model persisted/generated documents.
- [x] Projector payloads/manifests, canonical/generated pointers и repair filesystem protocols.
- [x] Добавить executable CLI parity regressions для Index, Context, Generation, Wiki и HTML.
- [ ] Выполнить targeted contract и executable regressions.
- [ ] Выполнить workspace tests, fmt и Clippy.

### Definition of Done

`DS-JSON-001` станет `[x] verified`, когда targeted fixtures, executable parity, workspace tests, fmt и Clippy фактически пройдут. Implementation inventory и запланированные executable regressions в audited scope завершены.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`: implementation complete, test execution pending.

### Реализовано

- [x] Application-output inventory и known wrapper migrations.
- [x] Repair public reports и embedded state fragments.
- [x] Daemon current envelopes/Jobs owners и legacy classification.
- [x] MCP native protocol registry и validators.
- [x] Four-entry schema-less process protocol registry.
- [x] 30-entry non-public Athanor schema registry с lifecycle и boundary class.
- [x] Current fixture coverage и явное отсутствие current fixtures для legacy/historical schemas.
- [x] Public/non-public sets взаимно исключительны.
- [x] Repository-wide source assertions связывают inventory с реальными runtime literals и framing.

### Осталось

- [ ] Запустить `process_persistence_contract_inventory` и остальные targeted tests.
- [ ] Устранить возможные compile/fmt/Clippy замечания, обнаруженные verification.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`: implementation и regressions complete, execution pending.

- [x] Direct CLI, daemon и active MCP Context используют `ContextReport`.
- [x] Direct CLI Index/Update и daemon index job используют один `IndexReport` shape.
- [x] Direct CLI Generate и daemon generation job используют один `GenerationReport` shape.
- [x] Direct Wiki/HTML JSON и daemon jobs используют `WikiReport`/`HtmlReport`.
- [x] Direct Config Validate/Doctor используют typed reports.
- [x] Direct API Snapshot и Docs Propose Fix используют versioned wrappers.
- [x] Repair CLI reports имеют shared registry ownership.
- [x] Daemon outer envelopes имеют current typed ownership; inner results сохраняют application schemas.
- [x] MCP outer protocol отделён; inner text содержит versioned application JSON.
- [x] Executable regression покрывает Index schema и metrics.
- [x] Executable regression покрывает Context schema и flattened pack shape.
- [x] Executable regression покрывает Generation/Wiki/HTML schemas и общий canonical snapshot.
- [ ] Фактически выполнить executable parity regression в Rust-enabled среде.
- [ ] Распространить parity verification на remaining shared operations только при фактическом обнаружении gap.

## Оценка остатка

Запланированная реализация завершена. Остался **1 verification-пакет**:

1. Targeted contract tests и executable parity.
2. `cargo fmt`, workspace tests и Clippy.
3. Исправления только по фактическим результатам проверок.

По объёму запланированной реализации осталось **0%**; неизвестный остаток возможен только по результатам compile/fmt/test/Clippy.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test daemon_transport_contracts --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test remaining_application_contracts --locked
cargo test -p athanor-app --test repair_contracts --locked
cargo test -p ath --test direct_config_cli --locked
cargo test -p ath --test direct_application_report_cli --locked
cargo test -p ath --test executable_shared_report_cli --locked
cargo test -p ath --test repair_cli --locked
cargo test -p athanor-app daemon_index_result_matches_public_index_report_shape --locked
cargo test -p athanor-app daemon_generation_result_matches_public_generation_report_shape --locked
cargo test -p athanor-app daemon_wiki_result_matches_public_wiki_report_shape --locked
cargo test -p athanor-app daemon_html_result_matches_public_html_report_shape --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app json_contract --locked
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Активный рабочий пакет

**Сейчас:** `DS-JSON-001/002/003` — implementation и planned regressions завершены: 59 public/current owners, 30 non-public schemas, 4 MCP boundaries, 4 process protocols и executable shared-report parity test.

**Дальше:** только фактический verification-проход и подтверждённые им исправления.

## Журнал

### 2026-07-17 — DS-JSON-003 executable shared-report parity

- Добавлен direct dispatcher для Generate, Wiki и Report Html с additive `--json`.
- Прежний human-readable output сохранён без изменения.
- Добавлен executable regression для реального `ath` binary: Index, Context, Generation, Wiki и HTML.
- Generation/Wiki/HTML связываются с одним snapshot из Index; daemon unit parity уже защищает сериализацию тех же report types.
- Execution не заявляется: Rust toolchain и hosted checks в текущей среде недоступны.

### 2026-07-17 — DS-JSON-001/002 process и persistence inventory

- Добавлен `boundary_contract` registry для persisted/generated/interchange/embedded документов.
- Зафиксированы 24 current schemas, 5 accepted legacy inputs и 1 historical-only schema.
- Инвентаризированы source-discover/extractor/linker/checker schema-less protocols и их framing.
- Добавлены representative fixture и repository-wide enforcement regression.
- Index-current/state/publication, canonical manifest/latest/commit, JSONL/Wiki/HTML manifests и projection payloads классифицированы.
- Daemon endpoint v1 исправлен с legacy-input на historical-only после сверки current reader.
- Implementation inventory в audited JSON scope завершена; verification остаётся неподтверждённым из-за отсутствия Rust toolchain и hosted status.

### 2026-07-17 — DS-JSON-001/002 transport envelopes

- Зарегистрированы current daemon request v3, response v3 и jobs v1 owners; public registry вырос с 56 до 59.
- MCP JSON-RPC 2.0 и protocol 2024-11-05 получили отдельный registry, validators и fixture.

### 2026-07-17 — DS-JSON application waves

- Завершены application wrappers, Config direct CLI, Wiki/HTML parity, API/Docs/Generation, Repair и specialized families.
- Generated, persisted, embedded и interchange boundaries классифицированы по мере миграции.
