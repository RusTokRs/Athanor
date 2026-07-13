# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Назначение: рабочий implementation plan для последовательного улучшения Athanor  
> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `26135e83d059a916c2bc6f52aeb3ad129c9039c9`  
> Дата актуализации: 2026-07-13  
> Статус: active implementation plan

## 0. Правила исполнения

Статусы:

- `[x]` — реализовано и подтверждено воспроизводимой проверкой либо доступным hosted evidence;
- `[-]` — реализация частичная либо ожидает hosted evidence/enforcement;
- `[ ]` — не начато;
- `[!]` — блокирует релиз или следующий обязательный этап.

Правила:

1. Один коммит — одна логически завершённая часть.
2. Не назначать числовые thresholds без фактического измерения.
3. Не переводить hosted/platform пункт в `[x]`, пока нет доступного run/status evidence.
4. Изменения архитектуры, release, CI и security одновременно отражать в документации и этом плане.
5. Пока разрешены прямые коммиты в `main`, сохранять Conventional Commits и воспроизводимые проверки. После branch protection перейти на PR.
6. Отсутствие push-triggered workflow run в ответе GitHub-коннектора не является ни успехом, ни ошибкой.
7. Process-local serialization не считается backend transaction или cross-process lease.

## 1. Текущий baseline

Подтверждено в коде и workflow:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in SurrealDB;
- snapshot-aware query/publication contracts;
- shared query и snapshot-lifecycle conformance для Memory, JSONL и embedded SurrealDB;
- отдельная `Store Conformance` matrix для трёх backend;
- process-local async write gate для клонированных SurrealDB store handles;
- Linux/Windows/macOS default quality matrix и Linux optional-feature matrix;
- measurement-only source coverage job;
- AppSec workflow: dependency review, CodeQL v4, Gitleaks и Zizmor;
- immutable SHA pinning GitHub Actions;
- signed platform archives, per-binary checksums, fail-closed installers;
- signed CycloneDX SBOM и release verification перед publish.

Базовые команды:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-store-memory --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-store-surrealdb --locked
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 2. Сводный статус ранее выявленных проблем

| Пункт | Статус | Что подтверждено | Что остаётся |
| --- | --- | --- | --- |
| 3.1 Query contract по `EntityId` | `[-]` | ID-based queries, `EntityResolver`, shared Memory/JSONL/embedded-Surreal conformance | Hosted evidence и внешний persistent SurrealDB path |
| 3.2 Snapshot isolation | `[-]` | exact/latest selectors, committed-only reads, shared prepare/commit/abort lifecycle | Полная transport matrix и generation-level visibility |
| 3.3 Atomic publication | `[-]` | JSONL staging/prepare/promotion, rollback, recovery journal | Единый generation pointer и backend transaction |
| 3.4 Concurrent writers | `[-]` | JSONL lock/sequence; process-local Surreal write gate и concurrent ID test | Cross-process Surreal lease/transaction conflicts |
| 3.5 Async process runner | `[-]` | Tokio, bounded streams, cancellation, Unix process groups | Windows Job Objects |
| 4.1 Adapter trust/sandbox | `[-]` | digest binding, path checks, clean environment support | Resource/network/filesystem isolation |
| 4.2 Operation context | `[-]` | deadlines/cancellation in main paths | Полная E2E matrix |
| 4.3 Pipeline memory | `[-]` | extraction concurrency and byte limits | Aggregate phase budget |
| 4.4 Incremental invalidation | `[-]` | file-local planning/dependency closure | add/remove/rename/cross-file matrix |
| 4.5 Storage transaction boundary | `[-]` | `SnapshotBatch`, shared lifecycle suite, JSONL durable prepare, Surreal process-local serialization | Portable prepared handle и native Surreal transaction |
| 4.6 Runtime composition | `[-]` | `RuntimeComposition` in main paths | Remove global registry, inject process runner |
| 4.7 Public API boundaries | `[-]` | focused namespaces/exports | Retire wildcard compatibility surface |
| 4.8 Module decomposition | `[-]` | partial pipeline/plugin/daemon split | CLI/runtime god modules |
| 4.9 Typed errors/protocol | `[-]` | stable codes, daemon v3, MCP error data | Full compatibility/retryability matrix |
| 4.10 Default build | `[x]` | SurrealDB excluded by default and covered by feature matrix | Maintain boundary |
| 5.1 Hosted CI governance | `[-]` | workflows and matrices exist | Hosted evidence, branch protection, required checks |
| 5.2 Coverage measurement | `[-]` | LCOV/JSON/HTML artifact job | Review real baseline before any gate |
| 5.3 AppSec automation | `[-]` | dependency review, CodeQL v4, Gitleaks, Zizmor, immutable pins, signed SBOM | Hosted evidence, blocking Zizmor, push protection |
| 5.4 Strict config | `[x]` | strict parsing and validate/doctor | Maintain contract tests |
| 5.5 Governance | `[-]` | templates, policies, Dependabot | Commit lint, release policy, issue traceability |

## 3. P0 — release-blocking quality and security gates

### P0.1. CI feature matrix

**Статус:** `[-]` — реализация и документация готовы; hosted run и required-check enforcement не подтверждены.

- [x] default/no-default, `store-surreal`, `js-ts-precision`, `--all-features`.
- [x] Для каждого slice: check/test/Clippy с `--locked`.
- [x] Документация в `docs/development/ci.md`.
- [ ] Подтвердить hosted run всех slices.
- [ ] Сделать feature matrix required check после branch protection.

Реализация: `d4b437b00fad2b5c2b078f9fb1c684d666e6122f`, `53a68dbba978c6ad6b172faf476df118ba679b9c`.

### P0.2. Coverage measurement; blocking gate отложен

**Статус:** `[-]` — измерение полезно, но жёсткий no-regression gate без baseline и PR flow не обоснован.

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV, JSON summary и HTML artifact.
- [x] Локальные команды задокументированы.
- [ ] Получить и проверить первый hosted artifact.
- [ ] Решить по данным, нужен общий floor или targeted coverage для core/store/publication.

Реализация: `1d5516d8a612ab81a593b83a8b3352ff1bb2468f`, `83efc933c7a5a0363114e3cda572d52521aa4905`.

### P0.3. PR-level AppSec и software supply chain

**Статус:** `[-]` — основной технический срез реализован; hosted evidence и окончательное enforcement не подтверждены.

- [x] Dependency review: новые уязвимости `moderate` и выше.
- [x] CodeQL v4 для Rust, `security-extended`, locked all-features build.
- [x] Gitleaks full-history scan.
- [x] Pinned Zizmor `1.26.1` в offline strict-collection режиме.
- [-] Zizmor report-only до review первого hosted report.
- [x] Все текущие `uses:` pinned на immutable SHA.
- [x] CycloneDX 1.5 SBOM, checksum, Sigstore и provenance.
- [x] Release `verify` блокирует `publish` до проверки archives и SBOM.
- [x] Release permissions разделены по jobs.
- [ ] Подтвердить hosted AppSec jobs и tag release pipeline.
- [ ] Удалить `--no-exit-codes` после разбора Zizmor findings.

### P0.4. Store conformance и transactional publication

**Статус:** `[-]` — первый backend-neutral vertical slice реализован; native transaction, cross-process coordination и generation publication остаются.

#### Shared backend contract

- [x] Расширить `athanor-store-conformance` lifecycle-контрактом `SnapshotBatch → prepare → commit/abort`.
- [x] Проверять невидимость uncommitted/prepared snapshot.
- [x] Проверять публикацию entity/fact/relation/diagnostic одним batch после commit.
- [x] Проверять запрет abort committed snapshot.
- [x] Проверять, что aborted snapshot удаляется и не меняет `LatestCommitted`.
- [x] Запускать query и lifecycle suites для Memory, JSONL и real embedded SurrealDB.
- [x] Добавить dedicated `Store Conformance` workflow matrix.
- [ ] Подтвердить успешный hosted matrix run.

#### SurrealDB writer safety

- [x] Добавить общий async write gate для всех clones одного `SurrealKnowledgeStore`.
- [x] Сериализовать begin/write/prepare/commit/abort внутри одного Athanor process.
- [x] Добавить 16-way concurrent snapshot allocation test и требование уникальных ID.
- [-] Concurrent writer contract частично закрыт: in-process race устранён, cross-process conflict semantics отсутствуют.
- [ ] Реализовать backend lease или native transaction для независимых процессов.
- [ ] Добавить explicit conflict/retry tests с двумя независимыми store connections/processes.

#### Transactional publication

- [ ] Ввести portable prepared transaction handle вместо неявного lifecycle по `SnapshotId`.
- [ ] Реализовать native SurrealDB transaction для полного `SnapshotBatch` и commit marker.
- [ ] Проверять per-statement backend errors до commit и делать rollback/cancel.
- [ ] Ввести единый immutable generation id для canonical/state/read models.
- [ ] Переключать один current pointer после подготовки всех artifacts.
- [ ] Добавить fault injection для begin/write/prepare/commit/pointer/recovery.

#### Реализация первого среза

- `1f1555c992a02db60eb1fbf7af38afa9e6e59610` — SurrealDB process-local write coordination wrapper;
- `bba33de8faba9ec9c8d0daedcdbf29ece61c1ce2` — Tokio sync primitives;
- `b91eb5c1089af8b4f94902bc730a43f57e2b6e40` — serialized facade как crate entrypoint;
- `d9bafc703de583cc70c8ce0fcea2714a447e05e2` — shared lifecycle conformance suite;
- `5ca54c6ff251fb8e50f3d8198a9f0a43a6c072c4` — Memory lifecycle integration;
- `1f824e56813893d8cf4b312ea3f54465d464e530` — JSONL lifecycle integration;
- `ea57e7ffc2d7733a2623569c46fe3cc99d14c28f` — SurrealDB lifecycle и concurrent allocation tests;
- `615f4f7b2237385930ef53b9f8c8d6ff30a0443c` — backend conformance CI matrix;
- `a8b0cbfae90d607221d7bbea5bd47cda91269d55`, `26135e83d059a916c2bc6f52aeb3ad129c9039c9` — architecture documentation.

**Definition of Done:** Memory, JSONL и persistent SurrealDB имеют эквивалентный observable lifecycle; независимые writers не теряют updates; partial writes не публикуются; после crash видна только предыдущая или новая целая generation.

## 4. P1 — безопасность исполнения и архитектурная сопровождаемость

### P1.1. External process sandbox hardening

- [ ] `clean_environment` как production default и environment allowlist.
- [ ] Windows Job Objects.
- [ ] Linux sandbox/launcher boundary.
- [ ] Network/filesystem/CPU/memory/process limits.
- [ ] Capability report и timeout/cancellation/orphan/limit tests.

### P1.2. Декомпозиция CLI

- [ ] Оставить в `main.rs` bootstrap, parse, dispatch и top-level errors.
- [ ] Разнести command handlers и rendering.
- [ ] Добавить help/exit-code/JSON contract tests.
- [ ] Не менять пользовательскую семантику.

### P1.3. Декомпозиция runtime

- [ ] Focused process contracts/builders.
- [ ] Composition wiring вне legacy paths.
- [ ] Cancellation-aware `ProcessRunner` через composition.
- [ ] Deprecate/remove legacy registry и добавить dependency-boundary tests.

### P1.4. Installer verification

**Статус:** `[-]` — реализация готова; hosted Windows/Linux evidence и tag packaging отсутствуют.

- [x] Per-binary `SHA256SUMS`, fail-closed Linux/Windows installers и positive/tamper CI smoke.
- [x] Archive/Sigstore/provenance verification задокументирована.
- [ ] Подтвердить hosted smoke jobs и следующий version tag.

### P1.5. Operation context и protocol E2E matrix

- [ ] Deadline propagation CLI→app, daemon→app, MCP→app.
- [ ] Stable code/retryable mapping и daemon v2/v3 compatibility.
- [ ] Cancellation queued/running writes and reads.
- [ ] Cooperative or isolated synchronous projectors.
- [ ] Rollback/recovery outside request deadline.

### P1.6. Aggregate performance budgets

- [ ] Aggregate byte budget, peak RSS и phase timings.
- [ ] 10k/50k/100k thresholds и large-file/high-relation scenarios.
- [ ] Incremental/full ratio matrix.

## 5. P2 — governance и contributor experience

### P2.1. Branch protection

- [ ] Disable force push/deletion и после bootstrap gates запретить direct push.
- [ ] Required: quality, feature matrix, store conformance, AppSec, docs.
- [ ] Stale review dismissal, up-to-date policy и secret push protection.

### P2.2. Traceability

- [ ] Issue на каждый открытый P0/P1 package.
- [ ] Acceptance criteria и verification commands в issue.
- [ ] Commit/PR links и синхронизация issue status с планом.

### P2.3. Commit/release policy

- [ ] Conventional commit lint и запрет `fix`, `changes`, `wip`.
- [ ] Automated changelog/release notes и readiness checklist.
- [ ] Version/tag/Cargo consistency.

### P2.4. Локальная воспроизводимость

- [x] Feature matrix, coverage measurement, installer/release и AppSec commands.
- [ ] Store conformance commands и troubleshooting в `docs/development/ci.md`.
- [ ] Common CI failures.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. P0.4 — получить hosted conformance evidence и реализовать native SurrealDB batch transaction/cross-process conflict semantics.
2. P0.3 — hosted AppSec evidence и blocking Zizmor.
3. P0.1/P1.4 — hosted matrix, installer и release tag evidence.
4. P0.4 — единая generation publication и fault injection.
5. P1.1 sandbox hardening.
6. P1.5 protocol E2E.
7. Coverage gate decision перед крупными рефакторингами.
8. P1.2/P1.3 decomposition.
9. P1.6 performance budgets.
10. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный пункт:** `P0.4 Store conformance и transactional publication`.

Первый срез завершён в коде, но не считается полностью подтверждённым без hosted run. Следующий срез:

1. проверить hosted Memory/JSONL/embedded-Surreal matrix;
2. выбрать backend-native SurrealDB transaction boundary для полного `SnapshotBatch`;
3. устранить read-modify-write allocation через backend transaction/lease, работающий между независимыми connections;
4. добавить conflict и rollback tests;
5. обновить архитектурную документацию и этот план;
6. не переходить к generation pointer, пока backend transaction semantics не зафиксированы тестами.

## 8. Definition of Done проекта

- Public queries изолированы committed snapshot.
- Memory, JSONL и SurrealDB проходят общий conformance suite.
- Publication crash-safe и atomic на generation boundary.
- Concurrent writers безопасны между независимыми процессами.
- External adapters bounded, cancellable и не оставляют orphan processes.
- Optional feature graphs и AppSec gates блокируют regressions.
- Coverage остаётся измеренной метрикой и становится gate только при доказанной пользе.
- Installers и release assets fail-closed проверяются, включая signed SBOM/provenance.
- CLI/runtime не остаются god modules.
- Typed errors/deadlines/cancellation эквивалентны во всех transports.
- Roadmap, plan, issues и release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-13 — первый P0.4 store conformance slice

- Shared conformance расширен lifecycle `SnapshotBatch → prepare → commit/abort`.
- Memory, JSONL и embedded SurrealDB подключены к одной suite.
- Добавлен dedicated Store Conformance workflow matrix.
- SurrealDB получил process-local async write gate, общий для cloned handles.
- Добавлен 16-way concurrent snapshot allocation test.
- Process-local gate явно не объявляется native transaction или cross-process lease.
- P0.4 переведён из `[ ]` в `[-]`; следующая цель — native batch transaction и independent-writer conflicts.

### 2026-07-13 — AppSec, immutable actions и signed SBOM

- Добавлены dependency review, CodeQL v4, Gitleaks, Zizmor, immutable pins и signed CycloneDX SBOM.
- Release verify блокирует publish; permissions сужены по jobs.
- P0.3 оставлен `[-]` до hosted evidence и blocking enforcement.

### 2026-07-13 — installer integrity и coverage decision

- Coverage сохранён как measurement artifact без выдуманного threshold.
- Release archives получили per-binary checksums; installers проверяют binaries до копирования.

### 2026-07-13 — feature matrix и первичный аудит

- Добавлена optional feature matrix.
- План сверён с архитектурой, workflows, installers и крупными modules.
