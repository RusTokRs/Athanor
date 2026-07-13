# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Назначение: рабочий implementation plan для последовательного улучшения Athanor  
> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `c17cf088124c8c33aec13ee93e69a4bbf58c5882`  
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
7. Process-local serialization не считается cross-process lease.
8. Одна backend transaction для `SnapshotBatch` не считается атомарной publication всей generation, пока commit marker и read models переключаются отдельно.
9. Не отмечать object-kind visibility подтверждённой, если public contract не позволяет прочитать этот kind независимо.
10. Embedded exclusive ownership и remote multi-writer semantics считать разными контрактами и тестировать отдельно.
11. Server-dependent tests не должны неявно запускаться в self-contained `--all-features` graph.
12. Process-local cancellation state не сериализовать в transport wire contract.
13. Наличие core cancellation primitive не считать E2E cancellation, пока application/daemon token не связан с ним.

## 1. Текущий baseline

Подтверждено в коде и workflow:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in SurrealDB;
- snapshot-aware query/publication contracts;
- shared query и snapshot-lifecycle conformance для Memory, JSONL и embedded SurrealDB;
- отдельная `Store Conformance` matrix для embedded backend и dedicated remote job;
- process-local async write gate для клонированных SurrealDB store handles;
- atomic SurrealDB counter increment и numeric snapshot sequence;
- native SurrealQL transaction для полного `SnapshotBatch` с statement-error checking и rollback test;
- отдельный `prepared` state и запрет late writes после prepare;
- persistent SurrealKV exclusive-owner regression с двумя независимыми `connect()`;
- stable retry mapping: lock contention и подтверждённые transaction conflicts → `Busy`, data/statement failures → non-retryable `AdapterExecution`;
- deadline-bounded retry schedule 10/25/50/100 ms для context-aware SurrealDB writes/publication;
- transport-neutral process-local `CancellationHandle`, привязанный к stable operation id без изменения JSON wire shape;
- cancellation-aware SurrealDB retry с проверкой перед попыткой и polling не более 5 ms во время backoff;
- opt-in `remote` WebSocket feature и ignored-by-default two-connection tests;
- Docker job с SurrealDB `2.6.5`, remote allocation и cross-connection canonical visibility;
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
cargo test -p athanor-core cancellation --locked
cargo test -p athanor-store-memory --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-store-surrealdb --locked
cargo test -p athanor-store-surrealdb cancellation_stops_retry_before_backoff --locked
ATHANOR_SURREAL_REMOTE_URI=ws://127.0.0.1:8000 \
  cargo test -p athanor-store-surrealdb --features remote --test remote --locked -- --ignored
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 2. Сводный статус ранее выявленных проблем

| Пункт | Статус | Что подтверждено | Что остаётся |
| --- | --- | --- | --- |
| 3.1 Query contract по `EntityId` | `[-]` | ID-based queries, `EntityResolver`, shared embedded conformance, remote contract configured | Hosted remote evidence и persistent server path |
| 3.2 Snapshot isolation | `[-]` | exact/latest selectors, committed-only reads, prepared immutability, shared lifecycle | Полная transport matrix и generation-level visibility |
| 3.3 Atomic publication | `[-]` | JSONL staging/recovery; SurrealDB atomic batch rollback | Commit marker, read models и единый generation pointer |
| 3.4 Concurrent writers | `[-]` | JSONL lock/sequence; Surreal atomic counter; embedded exclusive ownership; bounded Busy retry; cancellation-aware backoff; remote two-client test configured | Hosted remote evidence и observed write conflict |
| 3.5 Async process runner | `[-]` | Tokio, bounded streams, application cancellation, Unix process groups | Core-handle binding и Windows Job Objects |
| 4.1 Adapter trust/sandbox | `[-]` | digest binding, path checks, clean environment support | Resource/network/filesystem isolation |
| 4.2 Operation context | `[-]` | deadlines; process-local cancellation handle; SurrealDB check-active retry | Daemon/MCP/CLI E2E propagation и in-flight cancellation |
| 4.3 Pipeline memory | `[-]` | extraction concurrency and byte limits | Aggregate phase budget |
| 4.4 Incremental invalidation | `[-]` | file-local planning/dependency closure | add/remove/rename/cross-file matrix |
| 4.5 Storage transaction boundary | `[-]` | `SnapshotBatch`, shared lifecycle, JSONL prepare, native Surreal batch transaction, remote cross-connection canonical check configured | Backend-neutral Fact contract, portable prepared handle, full lifecycle transaction |
| 4.6 Runtime composition | `[-]` | `RuntimeComposition` in main paths | Remove global registry, inject process runner |
| 4.7 Public API boundaries | `[-]` | focused namespaces/exports | Retire wildcard compatibility surface |
| 4.8 Module decomposition | `[-]` | partial pipeline/plugin/daemon split | CLI/runtime god modules |
| 4.9 Typed errors/protocol | `[-]` | stable codes, daemon v3, MCP error data, Surreal Busy/Cancelled mapping | Full transport compatibility и observed remote-conflict matrix |
| 4.10 Default build | `[x]` | SurrealDB excluded by default; remote protocol opt-in; server tests ignored by default | Maintain boundary |
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
- [x] Remote server tests ignored by default и не ломают self-contained `--all-features`.
- [x] Документация в `docs/development/ci.md`.
- [ ] Подтвердить hosted run всех slices.
- [ ] Сделать feature matrix required check после branch protection.

### P0.2. Coverage measurement; blocking gate отложен

**Статус:** `[-]` — измерение полезно, но жёсткий no-regression gate без baseline и PR flow не обоснован.

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV, JSON summary и HTML artifact.
- [x] Локальные команды задокументированы.
- [ ] Получить и проверить первый hosted artifact.
- [ ] Решить по данным, нужен общий floor или targeted coverage для core/store/publication.

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

**Статус:** `[-]` — пятый cancellation-aware retry slice реализован; hosted evidence, daemon E2E bridge, наблюдаемый remote conflict и generation publication остаются.

#### Shared backend contract

- [x] Shared lifecycle `SnapshotBatch → prepare → commit/abort`.
- [x] Невидимость uncommitted/prepared snapshot.
- [-] Entity/relation/diagnostic visibility проверяется независимо; общий `KnowledgeStore` fact-query отсутствует.
- [x] Запрет abort committed snapshot.
- [x] Aborted snapshot не меняет `LatestCommitted`.
- [x] Memory, JSONL и embedded SurrealDB используют одну conformance suite.
- [x] Dedicated embedded matrix и isolated remote job.
- [-] Remote entity+fact canonical test настроен, hosted run не подтверждён.
- [ ] Подтвердить успешные hosted embedded и remote jobs.

#### SurrealDB writer safety

- [x] Общий async write gate для clones одного store handle.
- [x] Atomic counter и numeric sequence.
- [x] Embedded SurrealKV single-owner model и persistent lock regression.
- [x] Подтверждённые lock/transaction conflict messages → retryable `Busy`.
- [x] Data/duplicate/serialization failures → non-retryable `AdapterExecution`.
- [x] Bounded retry 10/25/50/100 ms только для context-aware writes/publication.
- [x] Deadline проверяется перед каждой попыткой и backoff.
- [x] Core `CancellationHandle` разделяется clones одного stable operation id.
- [x] Cancellation state не сериализуется в `OperationContext` JSON.
- [x] Retry вызывает `check_active()` перед попыткой и во время backoff.
- [x] Polling cancellation во время backoff не реже одного раза в 5 ms.
- [x] Cancelled retry не выполняет следующую backend attempt.
- [-] Два независимых remote SDK connections и 32 allocations: код/CI готовы, hosted evidence отсутствует.
- [ ] Детерминированно воспроизвести remote write conflict и подтвердить public `Busy` mapping.
- [ ] Связать application/daemon `CancellationToken` с core `CancellationHandle`.
- [ ] Определить cancellation semantics для уже выполняющегося SDK request.

#### Native SurrealDB batch transaction

- [x] Locked backend: SurrealDB `2.6.5`.
- [x] Все object arrays сериализуются до transaction.
- [x] Entity/fact/relation/diagnostic inserts выполняются в одном `BEGIN`/`COMMIT` query.
- [x] Statement-level failures проверяются через `response.check()`.
- [x] Duplicate-ID rollback regression и clean retry.
- [x] Prepared snapshot запрещает late writes.
- [-] Counter allocation и snapshot-record creation остаются разными requests.
- [-] Batch data transaction не включает commit marker.
- [-] Abort удаляет rows и metadata разными requests.

#### Transactional publication

- [-] Remote `CanonicalSnapshotStore` test читает entity и fact после commit; общий backend-neutral Fact conformance ещё отсутствует.
- [ ] Portable prepared transaction handle вместо неявного lifecycle по `SnapshotId`.
- [ ] Объединить batch data и commit marker в один publication protocol.
- [ ] Ввести immutable generation id для canonical/state/read models.
- [ ] Переключать один current pointer после подготовки всех artifacts.
- [ ] Fault injection для counter/create/write/prepare/commit/pointer/recovery.

#### Реализация

Первый срез:

- `1f1555c992a02db60eb1fbf7af38afa9e6e59610` — process-local SurrealDB write coordination;
- `d9bafc703de583cc70c8ce0fcea2714a447e05e2` — shared lifecycle suite;
- `615f4f7b2237385930ef53b9f8c8d6ff30a0443c` — backend conformance matrix.

Второй срез:

- `664ee557526060fc5bce85111aa4f262d75606c5` — native batch transaction, atomic counter, rollback и prepared immutability;
- `5162733283dfd18c298e8b47be09808095f0ac39` — transaction boundaries.

Третий срез:

- `c716bd35982c49f56212b4de02615f085e3464f0` — retry classification facade;
- `036d79f02b7481f00678b14336b2f579550285e5` — persistent lock regression;
- `47a6504b32a4bc8750062874cf6b65d9891e8213` — embedded ownership/retry architecture.

Четвёртый срез:

- `7f86d8e180c036c6a4962512636bbff849094925` — opt-in remote feature;
- `8a05d3ae1155d12800dcd191b4e7589dd87b260c` — deadline-bounded Busy retry;
- `d3f3c421b00b7b4df8b32708b1afb5968802f105` — remote two-connection tests;
- `7c95fcb8049641f984d2200118edc6521bf04ffb` — Docker remote CI job;
- `29ea91a4d46684971de88baa2a0018ccaefdbc57`, `027d6c0baa8cfa807ebc7a062e6c3036a0683ecb` — ignored isolation и explicit execution.

Пятый срез:

- `68a388b5c1616d7225f2ad20387f761b39af47f0` — process-local core cancellation handle и tests;
- `8ba2b84e1f1e6a5126d3bd6a4a3e183a318e0342` — export cancellation contract;
- `877e3c068d37e90882e00ad68ae7eaab22bf9071` — cancellation-aware SurrealDB retry;
- `3cfd188ee6acde77d804ffafacdfcd09679c3e36` — architecture boundaries;
- `c17cf088124c8c33aec13ee93e69a4bbf58c5882` — CI commands/troubleshooting.

**Definition of Done:** Memory, JSONL и persistent SurrealDB имеют эквивалентный observable lifecycle; все canonical object kinds независимо проверяемы; embedded ownership fail-closed; remote independent writers не теряют updates; retry bounded attempts/deadline/cancellation; partial writes не публикуются; после crash видна только предыдущая или новая целая generation.

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

**Статус:** `[-]` — core cancellation primitive и SurrealDB retry integration реализованы; transport/application E2E propagation не завершён.

- [-] Deadline propagation CLI→app, daemon→app, MCP→app.
- [x] Process-local cloneable `CancellationHandle` для stable operation id.
- [x] Cancellation state не меняет сериализуемую форму `OperationContext`.
- [x] Stable `Cancelled` error code остаётся non-retryable.
- [x] SurrealDB context-aware writes имеют deadline/cancellation-bounded Busy retry.
- [ ] Связать daemon job `CancellationToken` с core handle.
- [ ] Связать CLI/MCP cancellation lifecycle с core handle.
- [ ] Cancellation queued/running writes and reads.
- [ ] Cooperative or isolated synchronous projectors.
- [ ] Rollback/recovery outside request deadline/cancellation.

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
- [x] Embedded/remote store conformance и cancellation retry troubleshooting.
- [ ] Common CI failures для остальных workflows.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. P0.4 — получить hosted embedded/remote conformance и исправить реальные compile/format/API findings.
2. P1.5/P0.4 — связать daemon application token с core cancellation handle.
3. P0.4 — детерминированно воспроизвести remote transaction conflict и проверить public `Busy` mapping/backoff.
4. P0.4 — объединить batch data с commit-marker publication protocol.
5. P0.4 — добавить backend-neutral Fact conformance и portable prepared handle.
6. P0.3 — hosted AppSec evidence и blocking Zizmor.
7. P0.1/P1.4 — hosted matrix, installer и release tag evidence.
8. P0.4 — единая generation publication и fault injection.
9. P1.1 sandbox, coverage decision, decomposition и performance budgets.
10. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный пункт:** `P1.5/P0.4 Cancellation propagation`.

Пятый срез внесён в код, но без hosted run не считается полностью подтверждённым. Следующий срез:

1. добавить binding API между существующим `athanor-app::CancellationToken` и `athanor_core::CancellationHandle`;
2. связать daemon job cancellation с тем же `OperationContext`, который передаётся в pipeline/store;
3. проверить отмену queued/running index write во время Busy backoff;
4. не force-abort-ить уже выполняющийся backend request без отдельного backend-safe механизма;
5. после bridge вернуться к hosted remote evidence и deterministic conflict scenario;
6. не переходить к commit-marker redesign, пока cancellation propagation и remote job не закреплены тестами.

## 8. Definition of Done проекта

- Public queries изолированы committed snapshot.
- Memory, JSONL и SurrealDB проходят общий conformance suite.
- Publication crash-safe и atomic на generation boundary.
- Embedded persistent ownership fail-closed и диагностируется как retryable Busy.
- Remote concurrent writers безопасны между независимыми processes/connections.
- Busy retries ограничены attempts, deadline и E2E cancellation.
- Все canonical object kinds доступны проверяемому backend-neutral contract.
- External adapters bounded, cancellable и не оставляют orphan processes.
- Optional feature graphs и AppSec gates блокируют regressions.
- Coverage остаётся измеренной метрикой и становится gate только при доказанной пользе.
- Installers и release assets fail-closed проверяются, включая signed SBOM/provenance.
- CLI/runtime не остаются god modules.
- Typed errors/deadlines/cancellation эквивалентны во всех transports.
- Roadmap, plan, issues и release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-13 — пятый P0.4/P1.5 cancellation-aware retry slice

- В core добавлен cloneable process-local `CancellationHandle` для stable operation id.
- Cancellation registry хранит weak entries и не меняет serialized `OperationContext`.
- Добавлены tests shared cancellation, unchanged wire shape и anonymous-context rejection.
- SurrealDB retry использует `check_active()` перед attempts и во время backoff.
- Backoff polling ограничивает cancellation latency пятью миллисекундами.
- Cancelled retry не выполняет следующую backend attempt.
- Уже выполняющийся SDK request не force-abort-ится.
- Existing daemon `CancellationToken` пока не связан автоматически с core handle.
- P0.4/P1.5 остаются `[-]` до hosted evidence и E2E daemon bridge.

### 2026-07-13 — четвёртый P0.4 remote/retry slice

- Добавлен opt-in `remote` feature и ignored server-dependent tests.
- Dedicated Docker CI job поднимает SurrealDB `2.6.5`.
- Два SDK connections выполняют concurrent allocations и cross-connection canonical read.
- Context-aware writes retry-ят только `Busy` с bounded deadline-aware schedule.

### 2026-07-13 — третий P0.4 embedded conflict/retry slice

- Transient SurrealDB messages переводятся в retryable `Busy`.
- Persistent SurrealKV second-owner connection fail-closed.
- Embedded backend зафиксирован как single-owner process model.

### 2026-07-13 — второй P0.4 SurrealDB transaction slice

- `put_snapshot` переведён на один backend transaction.
- Добавлены statement checking, rollback regression, atomic counter и prepared immutability.

### 2026-07-13 — первый P0.4 store conformance slice

- Shared lifecycle suite подключена к Memory, JSONL и embedded SurrealDB.
- Добавлена Store Conformance workflow matrix.

### 2026-07-13 — AppSec, installer integrity и feature matrix

- Добавлены AppSec workflow, immutable action pins, signed SBOM и release verification.
- Installers получили fail-closed per-binary checksum verification.
- Добавлены optional feature matrix и measurement-only coverage artifacts.
