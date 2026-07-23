# Athanor: план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-22  
> Статус: architecture audit и `API-001` verified; исторический `v0.1.0` сохранён, готовится candidate `0.2.0`

## 1. Статусы и evidence

- `[x] implemented` — реализация опубликована, но отметка не заменяет execution evidence.
- `[-] in progress` — пакет начат, но его Definition of Done закрыт частично.
- `[x] verified` — реализация подтверждена successful matrix на одном exact commit.
- `[ ] planned` — работа относится к следующему этапу.
- `[!] blocked` — required matrix отсутствует или завершилась failure.

Workflow YAML является implementation evidence, а не execution evidence. Для package promotion exact
verification требует successful `athanor/verification-matrix`, `athanor/appsec` и
`athanor/store-conformance` statuses на одном SHA. CI identity дополнительно сохраняется в
`docs/development/verification-evidence.json`.

## 2. Итог архитектурного аудита и текущий verified baseline

Architecture audit source commit: `c4a494f3a1c1af5dcbad4252c5eb69e00d558b3a`.
Первичная audit matrix: run `29836572040`, conclusion `success`, completed at
`2026-07-21T14:06:12Z`.

Текущий verified product baseline: `23ef991baff6e416b1c790536a369fe950733252`.
На этом exact SHA успешно завершились:

- CI run `29957667631`;
- AppSec run `29957667663`;
- Store Conformance run `29957667601`.

CI evidence записан в `docs/development/verification-evidence.json`. Матрица покрывает cargo-deny,
source coverage, default/store-surreal/js-ts-precision/all-features, formatting, workspace tests,
Clippy, installers, index smoke и docs check на Linux, macOS и Windows.

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

- [x] Exact CI, AppSec, Store Conformance и JSON evidence привязаны к workflow SHA.
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

### 3.8 `API-001` — GraphQL and cross-protocol API consistency

- [x] GraphQL и OpenAPI операции публикуют canonical protocol identity.
- [x] Request-body properties сравниваются с GraphQL variables и matching named input objects.
- [x] OpenAPI path/query/header parameters и repository-owned external refs нормализуются fail closed.
- [x] Response containers, fields, scalar/list families и nullability сравниваются симметрично.
- [x] Effective OpenAPI security requirements сопоставляются с GraphQL authentication families.
- [x] Status-code policy, authentication и permission scopes входят в diagnostic contract.
- [x] Mutually exclusive security alternatives не объединяют scopes, а AND-схемы сохраняют состав.
- [x] Configurable `@athanorSecurity` / `@athanorSecurityMapping` mapping покрыт regressions.
- [x] Multi-root GraphQL responses выбираются по минимальному contract drift.
- [x] Package подтверждён exact CI/AppSec/Store evidence на
  `23ef991baff6e416b1c790536a369fe950733252`.

## 4. Активная разработка

### 4.1 `VERIFY-001` — execution matrix

- [x] Security, coverage, feature slices и quality jobs завершились success.
- [x] Architecture audit evidence сохранён для source commit
  `c4a494f3a1c1af5dcbad4252c5eb69e00d558b3a`.
- [x] Current product evidence сохранён для
  `23ef991baff6e416b1c790536a369fe950733252`.
- [x] `VERIFY-001`, `ARCH-AUDIT-001` и `API-001` имеют статус `[x] verified`.

### 4.2 `REL-001` — release readiness consolidation

- [x] Release workflow собирает Linux и Windows archives, checksums, signatures, provenance и SBOM.
- [x] `scripts/verify_release_version.py` требует exact `v<semver>` и одинаковые версии `ath`/`athd`.
- [x] Недатированная, отсутствующая, пустая или некалендарная version-section `CHANGELOG.md` блокирует
  release tag.
- [x] Дублирующиеся version-sections блокируют публикацию fail closed.
- [x] HTML comments, thematic breaks, пустые list/task markers и пустые fenced blocks не считаются
  содержательными release notes; непустой fenced content разрешён.
- [x] Matching changelog section публикуется как `release-notes.md`, а changelog входит в binary archives.
- [x] `docs/development/release.md` фиксирует supported artifacts, checklist и recovery policy.
- [x] `release_readiness_inventory` защищает workflow, guard, package versions, changelog и runbook.
- [x] Исторический `v0.1.0` сохранён на `593fe228fee71c53bd426472781d20fa08c501e5` и не переиспользуется.
- [x] Секция `0.2.0` заморожена с датой `2026-07-22`; release-hardening notes входят в candidate notes.
- [x] Runbook требует свободное имя тега и explicit candidate SHA с тремя successful statuses.
- [ ] Candidate `v0.2.0` должен пройти exact tag workflow целиком.

Статический Definition of Done, строгая календарная валидация, однозначный выбор version-section и
фильтрация декоративного changelog content реализованы. Пакет остаётся `[-] in progress`, поскольку
source-level regressions и successful push matrix не заменяют реальный tag-triggered build, SBOM,
signature, provenance и publication run.

### 4.3 Product backlog

- [ ] broader relationship/framework adapters;
- [ ] richer analysis completeness reporting;
- [ ] evidence-backed documentation generation;
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
| `VERIFY-001` | P1 | `[x] verified` | Runs `29957667631`, `29957667663`, `29957667601` succeeded on `23ef991baff6e416b1c790536a369fe950733252` |
| `API-001` | P1 | `[x] verified` | Five bounded slices and full exact CI/AppSec/Store evidence |
| `REL-001` | P1 | `[-] in progress` | Historical `v0.1.0` preserved; content-valid dated `0.2.0` candidate and exact-SHA runbook prepared |

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
cargo test -p athanor-app --test release_readiness_inventory --locked
cargo test -p athanor-app --test verification_evidence_inventory --locked
cargo test -p athanor-transport-mcp --test control_plane_saturation_inventory --locked
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 7. Текущий следующий шаг

Синхронизировать package versions и `Cargo.lock` для `0.2.0`, получить successful
`athanor/verification-matrix`, `athanor/appsec` и `athanor/store-conformance` на одном новом exact SHA,
затем создать annotated tag `v0.2.0` именно на этом SHA. После successful tag-triggered workflow
проверить artifacts и installation smokes и повысить `REL-001` до `[x] verified`.
