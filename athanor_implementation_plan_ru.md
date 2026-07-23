# Athanor: план реализации и архитектурного аудита

> Репозиторий: `RusTokRs/Athanor`  
> Ветка: `main`  
> Актуализировано: 2026-07-23  
> Статус: architecture audit, `API-001`, `REL-001` verified; `DOCGEN-001 / Slices 0A–1B` execution-confirmed

## 1. Статусы и evidence

- `[x] implemented` — код и source regressions присутствуют, но это не execution evidence.
- `[-] in progress` — пакет закрыт частично.
- `[x] verified` — required matrix успешна на одном exact source commit.
- `[ ] planned` — следующий bounded этап.
- `[!] blocked` — required matrix отсутствует или завершилась failure.

Package promotion требует успешных `athanor/verification-matrix`, `athanor/appsec` и
`athanor/store-conformance` на одном SHA. CI identity хранится в
`docs/development/verification-evidence.json`.

## 2. Verified baseline

Architecture audit source commit: `c4a494f3a1c1af5dcbad4252c5eb69e00d558b3a`; run
`29836572040` завершился success.

Текущий release baseline: `609027eb02caa05346ebfea8538552c42b588c31`:

- CI `29995959544`;
- AppSec `29995960063`;
- Store Conformance `29995959512`;
- Release `29996579628`;
- clean-install smoke `29998347890`.

Annotated tag `v0.2.1` указывает на этот SHA. Матрица покрыла cargo-deny, formatting, workspace tests,
Clippy, Linux/macOS/Windows, installers, index/docs smoke, features и source coverage.

## 3. Завершённые пакеты

### 3.1 `COMP-003` / `COMP-003C2B2C2B` — runtime composition

- [x] Runtime dependencies передаются через mandatory `RuntimeComposition`.
- [x] Check, API, Graph и read/write services разделены на bounded owners.
- [x] Legacy execution owners и public Store initializer удалены.

### 3.2 `MCP-007` — transactional Index cancellation

- [x] Canonical publication является durable commit point.
- [x] Pre-commit cancellation откатывает staged artifacts и snapshot.
- [x] Post-commit cancellation не маскирует успешный `IndexReport`.

### 3.3 `JSON-003` — contract lifecycle

- [x] Public, non-public, adapter и automation registries уникальны и disjoint.
- [x] Production Rust schema literals сканируются рекурсивно и fail closed.
- [x] Current persisted/generated/interchange boundaries имеют fixture coverage.

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
- [x] EOF вызывает `cancel_all` до task drain.

### 3.6 `VERIFY-001A`–`VERIFY-001G` — CI remediation

- [x] Exact CI/AppSec/Store statuses и JSON evidence привязаны к source SHA.
- [x] Repository-owned setup устанавливает Rust `1.95.0`.
- [x] Fail-only diagnostics сохраняют fmt/default-feature/cargo-deny output.
- [x] Full workspace, path aliases, executable mode, CRLF и Windows lifecycle закрыты.
- [x] Linux/macOS/Windows quality, installers, index/docs smoke, features и coverage подтверждены.

### 3.7 `API-001` — GraphQL and OpenAPI consistency

- [x] Canonical protocol identity и request/response/status/security comparison.
- [x] Repository-owned refs, security alternatives и configurable mappings fail closed.
- [x] Exact evidence на `609027eb02caa05346ebfea8538552c42b588c31`.

### 3.8 `REL-001` — release readiness consolidation

- [x] Exact tag/version/changelog contract.
- [x] Linux/Windows archives, checksums, signatures, provenance и CycloneDX SBOM.
- [x] Immutable historical tags `v0.1.0`, `v0.2.0`; published `v0.2.1`.
- [x] Clean Linux и Windows installation smokes.

## 4. Активная разработка

### 4.1 `DOCGEN-001` — evidence-backed documentation generation

#### Slices 0A–0B — contracts and evidence flow

- [x] Strict `DocumentationGenerationRequest` и `DocumentationGenerationManifest`.
- [x] Versioned outline, bounded context, citation, draft и validation-report contracts.
- [x] Hard limits, omissions, portable paths, SHA-256, data policy и quality metrics.
- [x] Minimal fixture repository и Rustok evaluation corpus.
- [x] Full contract-chain alignment и fail-closed regressions.
- [x] Exact evidence на `2a049303e797f00ac53f1e91fc010f284993926d`: CI `30005828864`,
  AppSec `30005828850`, Store `30005828956`.

#### Slice 1A — deterministic architecture profile

- [x] Pure app service принимает exact `CanonicalSnapshot`.
- [x] Canonical inputs сортируются независимо от входного порядка.
- [x] Overview, Components, Relationships и Diagnostics формируются deterministic.
- [x] Claims, Mermaid edges и footnotes имеют stable-key/evidence citations.
- [x] Limits и omissions входят в context/document/report.
- [x] Markdown и validation report создаются без store, filesystem, network или provider.

#### Slice 1B — immutable app-layer publication

- [x] Отдельный root `.athanor/generated/documentation` и 8-digit immutable generations.
- [x] `manifest.json`, `architecture/index.md` и `validation-report.json` staging/publish.
- [x] Atomic `current.json` с schema `athanor.documentation_current.v1`.
- [x] Validation report зарегистрирован как current generated boundary.
- [x] `UpToDate` требует exact artifact IDs, paths, media types и deterministic checksums.
- [x] Incomplete/tampered manifest, Markdown или report создают новую generation.
- [x] `force`, immutable history и pre-publication cancellation покрыты regressions.
- [x] Exact source commit `0cfeca8ad4dc3c0632246afa01e43372f4ec3d71` подтверждён:
  - Verification Matrix `30013208011`;
  - AppSec `30013208197`;
  - Store Conformance `30013208312`.

#### Slice 1C — next

- [ ] Загрузить exact committed snapshot через explicit `RuntimeComposition`.
- [ ] Добавить bounded application operation compose + publish.
- [ ] Добавить отдельный CLI command, не меняя молча semantics существующего `ath generate`.
- [ ] Добавить current/manifest/validation inspection в text и JSON modes.
- [ ] Протащить project resolution, exact snapshot identity и cancellation.
- [ ] Добавить missing/uncommitted snapshot, help, JSON parity и cancellation regressions.

`DOCGEN-001` остаётся `[-] in progress`: Slices 0A–1B execution-confirmed, но user-facing committed-snapshot
entrypoint отсутствует. Provider/LLM, daemon и MCP integration остаются вне Slice 1C.

### 4.2 Product backlog

- [ ] broader relationship/framework adapters;
- [ ] richer analysis completeness reporting;
- [ ] module/API/operations/onboarding profiles после Slice 1C;
- [ ] i18n/concept mapping и optional semantic/vector retrieval.

## 5. Программа работ

| ID | Priority | Status | Результат / критерий закрытия |
| --- | --- | --- | --- |
| `ARCH-AUDIT-001` | P1 | `[x] verified` | Run `29836572040` on `c4a494f3` |
| `COMP-003` | P2 | `[x] implemented` | Runtime dependencies explicit |
| `COMP-003C2B2C2B` | P2 | `[x] implemented` | Composition cleanup complete |
| `MCP-007` | P1 | `[x] implemented` | Transactional cancellation preserves durable success |
| `JSON-003` | P1 | `[x] implemented` | Schema lifecycle and fixtures enforced |
| `DOC-001` | P3 | `[x] implemented` | Status hygiene enforced |
| `DOC-002` | P3 | `[x] implemented` | Architecture docs aligned |
| `MCP-004` | P1 | `[x] implemented` | Control input remains observable under saturation |
| `VERIFY-001` | P1 | `[x] verified` | Full release baseline matrix on `609027eb` |
| `API-001` | P1 | `[x] verified` | Cross-protocol consistency exact evidence |
| `REL-001` | P1 | `[x] verified` | `v0.2.1` published and clean-installed |
| `DOCGEN-001` | P2 | `[-] in progress` | Slices 0A–1B execution-confirmed; Slice 1C next |

## 6. Verification matrix

```bash
cargo-deny check
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-app --test documentation_generation_contract_inventory --locked
cargo test -p athanor-app --test documentation_generation_slice0b_inventory --locked
cargo test -p athanor-app --test documentation_architecture_profile_inventory --locked
cargo test -p athanor-app --test documentation_architecture_publication_inventory --locked
cargo test -p athanor-app --test documentation_status_inventory --locked
cargo test -p athanor-app --test json_contract_inventory --locked
cargo test -p athanor-app --test process_persistence_contract_inventory --locked
cargo test -p athanor-app --test release_readiness_inventory --locked
cargo test -p athanor-app --test verification_evidence_inventory --locked
cargo test -p athanor-transport-mcp --test control_plane_saturation_inventory --locked
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 7. Текущий следующий шаг

Реализовать `DOCGEN-001 / Slice 1C`: composition-aware загрузку exact committed snapshot, bounded
architecture generation operation и отдельный CLI generation/inspection surface. Не подключать provider,
daemon или MCP и не изменять существующий coordinated `ath generate` без явного contract migration.
