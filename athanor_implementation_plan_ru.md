# Athanor: консолидированный план реализации

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-17  
> Статус: active implementation plan

## Правила статусов

- `[x] verified` — реализация и regressions подтверждены выполненными проверками.
- `[-] in progress` — полезный срез находится в `main`, но Definition of Done закрыт не полностью.
- `[ ] planned` — работа не начата.
- `[!] blocked` — требуется compatibility decision или недоступная платформенная проверка.

JSON считается внешним контрактом. Несовместимое изменение требует нового major schema id. Эквивалентные CLI, daemon и MCP операции не должны иметь разные application payload shapes.

## Текущая последовательность

| ID | Priority | Status | Result |
| --- | --- | --- | --- |
| `DS-RESOLVE-003` | P1 | `[x] verified` | Validated Artifact Resolver Migration завершена |
| `DS-JSON-001` | P1 | `[-] in progress` | Registry содержит 59 current Athanor contracts; MCP protocol registry отделён |
| `DS-JSON-002` | P1 | `[-] in progress` | Application и transport inventory завершены; process/persistence pending |
| `DS-JSON-003` | P1 | `[-] in progress` | Typed CLI/daemon/MCP inner payload parity реализована; executable verification pending |

## DS-RESOLVE-003 — Validated Artifact Resolver Migration

**Статус:** `[x] verified`.

- [x] Generated read-model и index-state paths разрешаются pointer-first.
- [x] Path/type/schema/snapshot/generation/checksum identity проверяется до использования.
- [x] Runtime consumers используют shared resolver boundary.
- [x] Rustok resolver coverage сохранена после runtime decomposition.

## DS-JSON-001 — Versioned JSON Contracts

**Статус:** `[-] in progress`.

### Contract foundation

- [x] App-layer `json_contract` facade и trait `VersionedJsonContract`.
- [x] Canonical schema-id validator и typed `JsonContractError`.
- [x] Проверка top-level object/string `schema` и instance/associated-constant equality.
- [x] Registry отклоняет duplicate schema id и duplicate Rust owner.
- [x] Standard protocol registry отделён от Athanor schema registry.

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
- [x] MCP JSON-RPC/tool-content envelopes учтены отдельным registry по native protocol versions.

### Compatibility migrations и classifications

- [x] Additive wrappers сохраняют прежние application fields.
- [x] CLI/daemon общие Index, Generation, Wiki и HTML results используют typed reports.
- [x] Direct Config, API Snapshot и Docs Propose Fix используют typed/versioned reports.
- [x] Repair JSON shapes зарегистрированы без изменения существующего CLI output.
- [x] Daemon v1/v2 request compatibility остаётся accepted input, но не current ownership.
- [x] Daemon Error/Command/Job — embedded; Endpoint v3 — persisted runtime descriptor.
- [x] MCP JSON-RPC 2.0 и protocol 2024-11-05 не получают synthetic `athanor.*` schema ids.
- [x] MCP text content переносит внутренний serialized Athanor report с его собственным schema id.
- [x] Generated, persisted, embedded и interchange документы отделены от public registry.

### Golden и integration regressions

- [x] Existing application, Graph, RusTok, Config, Docs, API, Wiki/HTML и Repair fixtures.
- [x] Daemon transport fixture для current request/success/error/jobs shapes.
- [x] Daemon inventory regression исключает legacy и embedded/persisted owners.
- [x] MCP standard-protocol fixture, registry uniqueness и fail-closed validators.
- [x] Existing MCP runtime unit tests покрывают initialize, tools list, parse/error behavior.
- [x] Extended Athanor registry regression защищает 59 schema id и ownership.
- [!] Локальный Rust toolchain отсутствует; fmt/test/Clippy не заявляются выполненными.
- [!] Hosted Actions/check status для `main` не виден через connector.

### Следующий срез

- [x] Versioned application outputs и direct CLI migrations.
- [x] Repair public outputs и embedded state inventory.
- [x] Daemon request/response/error envelopes.
- [x] MCP JSON-RPC и tool-content envelopes.
- [ ] Инвентаризировать extractor/linker/checker process protocols.
- [ ] Инвентаризировать index/publication/read-model persisted documents.
- [ ] Классифицировать projector payloads/manifests, repair guards и remaining pointer documents.
- [ ] Добавить executable CLI/daemon parity regressions для оставшихся общих операций.
- [ ] Выполнить targeted и workspace verification.

### Definition of Done

`DS-JSON-001` станет `[x] verified`, когда process/persistence inventory завершена, executable parity regressions пройдены, fixtures выполняются, а fmt, targeted tests, workspace tests и Clippy зелёные.

## DS-JSON-002 — Registry-wide inventory и enforcement

**Статус:** `[-] in progress`.

### Завершено

- [x] Application-output inventory и known wrapper migrations.
- [x] Repair public reports и embedded state fragments.
- [x] Daemon current envelopes/Jobs owners и legacy classification.
- [x] MCP standard JSON-RPC/tool-content protocol registry и validators.
- [x] Persisted: Project Registry state, index-current publication journal и daemon endpoint.
- [x] Generated: validation result, generation manifest/current pointer, API snapshot/latest pointer.
- [x] Embedded: metrics, Repair fragments и Daemon error/command/job.
- [x] Interchange: `athanor.docs_patch.v1`.
- [x] Classification sets взаимно исключительны.

### Осталось

- [ ] Extractor/linker/checker process protocols inventory.
- [ ] Index-current/index-state/publication/read-model manifest inventory.
- [ ] Projector payloads/manifests, repair guards, generated/canonical pointers и remaining persisted documents.

## DS-JSON-003 — CLI/daemon/MCP parity

**Статус:** `[-] in progress`.

- [x] Direct CLI, daemon и active MCP Context используют `ContextReport`.
- [x] Direct CLI Index/Update и daemon index job используют один `IndexReport` shape.
- [x] Direct CLI Generate и daemon generation job используют один `GenerationReport` shape.
- [x] Wiki/HTML app results и daemon jobs используют typed reports.
- [x] Direct Config Validate/Doctor используют typed reports.
- [x] Direct API Snapshot и Docs Propose Fix используют versioned wrappers.
- [x] Repair CLI reports имеют shared registry ownership.
- [x] Daemon outer envelopes имеют current typed ownership; inner results сохраняют application schemas.
- [x] MCP outer protocol отделён; inner text содержит versioned application JSON.
- [ ] Добавить executable CLI/daemon Index parity regression.
- [ ] Добавить executable CLI/daemon Generation parity regression.
- [ ] Добавить executable Context parity regression.
- [ ] Добавить executable Wiki/HTML parity regressions.
- [ ] Распространить parity enforcement на remaining shared operations.

## Оценка остатка

Осталось **1 implementation-пакет и 1 verification-пакет**:

1. Process + persistence boundaries: extractor/linker/checker protocols, index/publication/read-model/projector documents, repair guards и pointers.
2. Verification: executable parity, fmt, targeted tests, workspace tests и Clippy.

По объёму реализации осталось ориентировочно **6–12% DS-JSON работы**. Verification может занять сопоставимое время, если обнаружатся compile/fmt/Clippy regressions.

## Проверки

```bash
cargo fmt --all -- --check
cargo test -p athanor-app --test daemon_transport_contracts --locked
cargo test -p athanor-transport-mcp --test mcp_transport_contracts --locked
cargo test -p athanor-app --test remaining_application_contracts --locked
cargo test -p athanor-app --test repair_contracts --locked
cargo test -p ath --test direct_config_cli --locked
cargo test -p ath --test direct_application_report_cli --locked
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

**Сейчас:** `DS-JSON-001/002/003` — application и transport inventory реализованы; 59 Athanor-schema owners плюс отдельный MCP registry; verification pending.

**Дальше:** process/persistence boundaries, затем полный verification-проход.

## Журнал

### 2026-07-17 — DS-JSON-001/002 transport envelopes

- Зарегистрированы current daemon request v3, response v3 и jobs v1 owners.
- Registry вырос с 56 до 59 Athanor-schema contracts.
- Legacy daemon requests v1/v2 оставлены accepted input; embedded и persisted daemon types исключены.
- MCP JSON-RPC 2.0 и protocol 2024-11-05 получили отдельный four-entry registry, validators и fixture.
- Synthetic `athanor.mcp_*` schema запрещён regression-проверкой.
- Локальный запуск fmt/test/Clippy не выполнен: Rust toolchain и исходящий network в среде отсутствуют; hosted status через connector не виден.

### 2026-07-17 — DS-JSON-001/002 Repair reports и state inventory

- Зарегистрированы девять Repair report owners; registry вырос с 47 до 56.
- Embedded repair states/rows/tombstones исключены из public registry.
- `athanor.index_current_publication.v1` классифицирован как persisted journal.

### 2026-07-17 — DS-JSON-001/002/003 remaining application wrappers

- Добавлены `athanor.api_snapshot.v1` и `athanor.docs_propose_fix.v1`.
- Registry расширен с 45 до 47 owners.
- `api snapshot` и `docs propose-fix` направлены через direct dispatcher.

### 2026-07-17 — DS-JSON-001/003 direct Config CLI parity

- Config Validate/Doctor переведены на typed reports.
- Добавлен executable regression; human-readable output сохранён.

### 2026-07-17 — Earlier DS-JSON waves

- Зарегистрированы Config, Wiki/HTML, API, Generation, Docs, Index, Project Registry, Context, Architecture, Graph и specialized RusTok families.
- Generated, persisted, embedded и interchange boundaries классифицированы по мере миграции.
