# Athanor: план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-21  
> Статус: architecture audit verified; product development active

## 1. Статусы и evidence

- `[x] implemented` — реализация опубликована, но отметка не заменяет execution evidence.
- `[-] in progress` — пакет начат, но его Definition of Done закрыт частично.
- `[x] verified` — реализация подтверждена successful matrix на одном exact commit.
- `[ ] planned` — работа относится к следующему этапу.
- `[!] blocked` — required matrix отсутствует или завершилась failure.

Workflow YAML является implementation evidence, а не execution evidence. Exact verification требует
успешного `athanor/verification-matrix` status либо schema-valid JSON evidence для того же SHA.

## 2. Итог архитектурного аудита

Architecture source commit: `c4a494f3a1c1af5dcbad4252c5eb69e00d558b3a`.
Стандартная CI matrix: run `29836572040`, conclusion `success`, completed at
`2026-07-21T14:06:12Z`. Evidence записан в `docs/development/verification-evidence.json`.

Подтверждены cargo-deny, source coverage, default/store-surreal/js-ts-precision/all-features,
formatting, workspace tests, Clippy, installers, index smoke и docs check на Linux, macOS и Windows.

## 3. Завершённые пакеты

### 3.1 `COMP-003` / `COMP-003C2B2C2B` — runtime composition

- [x] Runtime dependencies выражены через mandatory `RuntimeComposition`.
- [x] Check, API, Graph и read/write services разделены на bounded owners.
- [x] Legacy owners и public Store initializer удалены.

### 3.2 `MCP-007` — transactional Index cancellation

- [x] Canonical publication является durable commit point.
- [x] Pre-commit cancellation откатывает artifacts и aborts snapshot.
- [x] Post-commit cancellation не маскирует успешный typed `IndexReport`.

### 3.3 `JSON-003` — contract lifecycle

- [x] Public, internal, adapter и automation registries уникальны и disjoint.
- [x] Production Rust inventories сканируются рекурсивно и fail closed.
- [x] CLI, daemon и MCP используют эквивалентный typed payload.

### 3.4 `DOC-001` / `DOC-002` — documentation status hygiene

- [x] Roadmap является compact current-state ledger.
- [x] Aggregate stale verification claims удалены.
- [x] Pipeline current/target/history разделены.
- [x] Documentation map, roadmap и implementation plan синхронизированы.
- [x] `documentation_status_inventory` фиксирует status/path/alignment/line budgets.

### 3.5 `MCP-004` — control-plane responsiveness

- [x] Notifications bypass ordinary request admission.
- [x] Inline responses используют nonblocking `try_send`.
- [x] Full/closed response queue не завершает reader loop.
- [x] EOF вызывает `cancel_all` до request-task drain.

### 3.6 `VERIFY-001A`–`VERIFY-001F` — CI, docs и project remediation

- [x] Exact status и JSON evidence привязаны к workflow SHA.
- [x] Docs lifecycle поддерживает `active`, `implemented`, `planned`, `draft`, `verified`.
- [x] Repository-owned setup устанавливает Rust `1.95.0`.
- [x] Failed fmt/default-feature/cargo-deny output сохраняется fail-only artifacts.
- [x] Formatting, compile owners, MCP lifecycle и advisory lockfile remediated.

### 3.7 `VERIFY-001G` — full workspace и cross-platform remediation

- [x] V21/V24 закрыли full workspace, path aliases и executable mode на трёх ОС.
- [x] V28–V35 закрыли stale incremental state, Surreal allocation и CRLF inventories.
- [x] V36–V40 закрыли LF-only boundaries и Windows process lifecycle regressions.
- [x] V41 подтвердил Linux/macOS/Windows quality, installer/index/docs, `store-surreal` и stress runs.
- [x] Временная remediation infrastructure отсутствует в verified tree.

## 4. Активная разработка

### 4.1 `VERIFY-001` — execution matrix

- [x] Security, coverage, feature slices и quality jobs завершились success.
- [x] Exact commit: `c4a494f3a1c1af5dcbad4252c5eb69e00d558b3a`.
- [x] Exact run: `29836572040`.
- [x] Successful evidence SHA совпадает с architecture commit.
- [x] `VERIFY-001` и `ARCH-AUDIT-001` имеют статус `[x] verified`.

### 4.2 `API-001` — GraphQL and cross-protocol API consistency

Первый bounded slice нормализует protocol identity на adapter boundary, чтобы уже существующий
OpenAPI/GraphQL response-field checker работал на реальных canonical entities, а не только на вручную
собранных test fixtures.

- [x] GraphQL operations публикуют canonical `protocol = graphql`.
- [x] OpenAPI operations нормализуются до canonical `protocol = openapi` через fail-closed adapter boundary.
- [x] Existing checker сравнивает response fields операций с одинаковым normalized name.
- [x] Первый slice подтверждён стандартной main matrix: run `29845490657` succeeded on `cb2db0bb374f845fa0dbd086b120a0def82b0d9d`.
- [x] Второй slice сравнивает request-body properties с GraphQL variables или matching named input objects; scalar/list/required/nullability normalization и regressions слиты в `f20fcbbec9780975fc497b739e6df72d4f30901b` после successful CI `29848358160`, AppSec `29848358435` и Store Conformance `29848359149` на PR head `c4ffbc759adb8bbdbe7cda7afc1955343e828e81`.
- [x] Третий slice публикует canonical OpenAPI path/query/header parameter metadata, разрешает repository-owned external request/response refs и сравнивает response schema containers, fields, scalar/list families и nullability; слит в `7b92cb2fe587f5134af5a417365bb63493885e19` после successful CI `29853853811`, AppSec `29853855239` и Store Conformance `29853854118` на PR head `d6209afc2b2b711d29924f26699aa88f6df431bd`.
- [x] Четвёртый slice публикует effective OpenAPI security requirements, сохраняет GraphQL directive argument values и сравнивает status-code policy, authentication families и permission scopes; слит в `13ec9bc8b92e02ca15a8c7d7258f4aa2d9e58397` после successful CI `29861948723`, AppSec `29861948815` и Store Conformance `29861948791` на PR head `ad66c8b1bbd43f2ede1ea1d6b089debb58ac5600`.
- [-] Пятый bounded slice опубликован в `main`, но exact verification ещё не завершена:
  - [x] исправлена ошибка четвёртого slice: scopes больше не объединяются между mutually exclusive OpenAPI security alternatives (`97805cda6d8996af247fd0e30fdbfee3386a9ce9`);
  - [x] исправлена AND-семантика OpenAPI: все security schemes внутри одного alternative должны быть представлены GraphQL authentication families (`49f56c991a1dcf61ae438b2f3e8f7c4e83a8942b`);
  - [x] добавлена configurable GraphQL security-directive mapping через canonical metadata или `@athanorSecurity` / `@athanorSecurityMapping`;
  - [x] repository-owned `components.parameters` публикуются также из partial component documents (`3b5cb1ea60c16c1a125ddfe54fcc997d587d2976`);
  - [x] external parameter refs разрешаются по normalized repository path, remote URL refs остаются explicit local-checker boundary;
  - [x] multi-root GraphQL responses выбираются по минимальному contract drift без ложного diagnostic, когда один root совместим (`615b306729c62d95a043e66a83b137798663dc91`);
  - [x] сохранены прежние diagnostic payload fields и regressions предыдущих contract slices;
  - [x] active implementation SHA пятого slice: `49f56c991a1dcf61ae438b2f3e8f7c4e83a8942b`;
  - [ ] получить successful CI, AppSec и Store Conformance evidence на exact active implementation SHA.
- [ ] повысить полный `API-001` до verified только после закрытия remaining Definition of Done и exact successful main matrix.

### 4.3 Product backlog

- [ ] broader relationship/framework adapters;
- [ ] richer analysis completeness reporting;
- [ ] evidence-backed documentation generation;
- [ ] release-readiness consolidation;
- [ ] i18n/concept mapping и optional semantic/vector retrieval.

## 5. Программа работ

| ID | Priority | Status | Результат / критерий закрытия |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[x] verified` | Run `29836572040` succeeded on `c4a494f3a1c1af5dcbad4252c5eb69e00d558b3a` |
| `COMP-003` | P2 | `[x] implemented` | Runtime dependencies explicit |
| `COMP-003C2B2C2B` | P2 | `[x] implemented` | Composition cleanup complete |
| `MCP-007` | P1 | `[x] implemented` | Transactional cancellation preserves durable success |
| `JSON-003` | P1 | `[x] implemented` | Schema lifecycle and payload parity enforced |
| `DOC-001` | P3 | `[x] implemented` | Stale verification and removed paths cleaned |
| `DOC-002` | P3 | `[x] implemented` | Pipeline/status docs aligned |
| `MCP-004` | P1 | `[x] implemented` | Control input remains observable under saturation |
| `VERIFY-001A` | P1 | `[x] implemented` | Exact JSON/status evidence channels |
| `VERIFY-001B` | P1 | `[x] implemented` | Docs gate matches lifecycle semantics |
| `VERIFY-001C` | P1 | `[x] implemented` | Workflow toolchain matches Rust 1.95 |
| `VERIFY-001D` | P1 | `[x] implemented` | Failure diagnostics retrievable |
| `VERIFY-001E` | P1 | `[x] implemented` | Formatting, owners and lockfile remediated |
| `VERIFY-001F` | P1 | `[x] implemented` | Structural execution blockers closed |
| `VERIFY-001G` | P1 | `[x] implemented` | Cross-platform blockers closed by V21/V24/V41 |
| `VERIFY-001` | P1 | `[x] verified` | Run `29836572040` succeeded on `c4a494f3a1c1af5dcbad4252c5eb69e00d558b3a` |
| `API-001` | P1 | `[-] in progress` | Five bounded slices implemented; exact active-main CI/AppSec/Store verification pending |

## 6. Verification matrix

```bash
cargo-deny check
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-extractor-openapi --locked
cargo test -p athanor-extractor-graphql --locked
cargo test -p athanor-checker-api --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p athanor-app --test verification_evidence_inventory --locked
cargo test -p athanor-transport-mcp --test control_plane_saturation_inventory --locked
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 7. Текущий следующий шаг

Получить exact successful main matrix для пятого bounded `API-001` slice на
`49f56c991a1dcf61ae438b2f3e8f7c4e83a8942b`. При failure сначала устранить конкретную regression и
обновить эту отметку; при success отметить slice `[x]`, записать run IDs и затем проверить
оставшийся Definition of Done всего `API-001`.
