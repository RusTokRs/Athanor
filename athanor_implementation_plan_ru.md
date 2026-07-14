# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Назначение: рабочий implementation plan для последовательного улучшения Athanor  
> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `837506aa16954cf8bb0f84887b08b8e54b6a86cf`  
> Дата актуализации: 2026-07-14  
> Статус: active implementation plan

## 0. Правила исполнения

Статусы:

- `[x]` — реализовано и подтверждено воспроизводимой проверкой либо доступным hosted evidence;
- `[-]` — реализация частичная либо ожидает hosted evidence/enforcement;
- `[ ]` — не начато;
- `[!]` — блокирует релиз или следующий обязательный этап.

Правила:

1. Один коммит — одна логически завершённая часть.
2. Не назначать thresholds без фактического измерения.
3. Не переводить hosted/platform пункт в `[x]` без run/status evidence.
4. Архитектурные, CI, release и security изменения отражать в документации и плане.
5. До branch protection допустимы прямые Conventional Commits в `main`.
6. Отсутствие push-run в ответе коннектора не считается успехом или ошибкой.
7. Process-local serialization не считается cross-process lease.
8. Backend transaction для `SnapshotBatch` не считается atomic generation publication, пока commit marker и read models переключаются отдельно.
9. Embedded exclusive ownership и remote multi-writer semantics — разные контракты.
10. Server-dependent tests не должны неявно запускаться в self-contained `--all-features` graph.
11. Cancellation handle, keyed by operation id, требует уникального id на время активной операции.
12. Cancellation/deadline не должны блокировать rollback и recovery.
13. Наличие typed prepared handle не считается migration coordinator, пока journal и publish path продолжают использовать raw `SnapshotId`.

## 1. Текущий baseline

Подтверждено в коде и workflow:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in SurrealDB;
- shared query/lifecycle conformance для Memory, JSONL и embedded SurrealDB;
- dedicated embedded matrix и opt-in remote SurrealDB job;
- native SurrealQL transaction для полного `SnapshotBatch`;
- atomic snapshot counter, numeric sequence и prepared immutability;
- embedded SurrealKV exclusive-owner contract;
- retry mapping: transient lock/conflict → `Busy`, data/statement failures → non-retryable;
- bounded Busy retry `10/25/50/100 ms` с deadline и cancellation polling;
- core `CancellationHandle` без изменения `OperationContext` JSON;
- daemon cancellation bridge для index/generate/wiki/html;
- application-level `PreparedSnapshot` и `PreparedSnapshotPublication` protocol;
- `AthanorStore` делегирует context-aware backend overrides;
- Linux/Windows/macOS quality matrix, optional-feature matrix и store conformance workflow;
- measurement-only coverage artifacts;
- dependency review, CodeQL v4, Gitleaks, Zizmor и immutable action pins;
- signed archives, installer checksums, CycloneDX SBOM и release verification.

Базовые команды:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-core cancellation --locked
cargo test -p athanor-app --test prepared_publication --locked
cargo test -p athanor-store-memory --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-store-surrealdb --locked
ATHANOR_SURREAL_REMOTE_URI=ws://127.0.0.1:8000 \
  cargo test -p athanor-store-surrealdb --features remote --test remote --locked -- --ignored
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 2. Сводный статус проблем

| Пункт | Статус | Подтверждено | Остаётся |
| --- | --- | --- | --- |
| Query contract | `[-]` | ID queries, resolver, embedded suites, remote contract configured | Hosted remote evidence, полный persistent path |
| Snapshot isolation | `[-]` | committed-only reads, prepared immutability, lifecycle suite | Generation-level visibility |
| Atomic publication | `[-]` | JSONL staging/recovery, SurrealDB batch rollback, typed prepared handle | Coordinator migration, commit marker и generation pointer |
| Concurrent writers | `[-]` | JSONL locking, atomic Surreal counter, embedded ownership, remote two-client test configured | Hosted remote conflict evidence |
| Operation context | `[-]` | deadlines, core cancellation, daemon write-job bridge, context-forwarding wrapper | Read commands, CLI/MCP, in-flight SDK interruption |
| Storage transaction boundary | `[-]` | SnapshotBatch, native Surreal transaction, prepared publication extension | Journal migration и atomic data+marker publish |
| Runtime composition | `[-]` | explicit composition in main paths | Global registry removal, injected process runner |
| Default build | `[x]` | SurrealDB opt-in, remote tests ignored by default | Maintain boundary |
| Hosted CI governance | `[-]` | workflows/matrices exist | Hosted evidence, branch protection, required checks |
| AppSec automation | `[-]` | dependency review, CodeQL, Gitleaks, Zizmor, SBOM | Hosted evidence, blocking Zizmor, push protection |
| Installer/release integrity | `[-]` | fail-closed installers and signed assets | Hosted/tag evidence |

## 3. P0 — release-blocking quality and security gates

### P0.1. CI feature matrix

**Статус:** `[-]` — реализация готова; hosted run и required checks не подтверждены.

- [x] default/no-default, `store-surreal`, `js-ts-precision`, `--all-features`.
- [x] Check/test/Clippy с `--locked`.
- [x] Remote server tests изолированы от обычного `--all-features`.
- [x] Документация в `docs/development/ci.md`.
- [ ] Подтвердить hosted run всех slices.
- [ ] Сделать matrix required check после branch protection.

### P0.2. Coverage measurement

**Статус:** `[-]` — измерение добавлено; blocking floor отложен до реального baseline.

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV, JSON summary и HTML artifact.
- [ ] Получить первый hosted artifact.
- [ ] Решить по данным, нужен общий floor или targeted core/store/publication gate.

### P0.3. AppSec и software supply chain

**Статус:** `[-]` — технический срез реализован; hosted enforcement отсутствует.

- [x] Dependency review с порогом `moderate`.
- [x] CodeQL v4/Rust `security-extended`.
- [x] Gitleaks full history.
- [x] Pinned Zizmor `1.26.1`; пока report-only.
- [x] Все `uses:` pinned на immutable SHA.
- [x] CycloneDX SBOM, checksum, Sigstore и provenance.
- [x] Release `verify` блокирует `publish`.
- [ ] Подтвердить hosted AppSec и tag release.
- [ ] Удалить `--no-exit-codes` после review Zizmor findings.

### P0.4. Store conformance и transactional publication

**Статус:** `[-]` — седьмой prepared-handle/context-forwarding slice реализован; coordinator migration, hosted evidence, observed remote conflict и generation publication остаются.

#### Shared backend contract

- [x] Shared lifecycle `SnapshotBatch → prepare → commit/abort`.
- [x] Невидимость uncommitted/prepared snapshot.
- [x] Запрет abort committed snapshot.
- [x] Aborted snapshot не меняет `LatestCommitted`.
- [x] Memory, JSONL и embedded SurrealDB используют одну suite.
- [x] Dedicated embedded matrix и isolated remote job.
- [-] Remote entity+fact canonical test настроен, hosted run не подтверждён.
- [-] Общий `KnowledgeStore` fact-query отсутствует.

#### SurrealDB writer safety и retry

- [x] Общий async write gate для clones одного store handle.
- [x] Atomic counter и numeric sequence.
- [x] Embedded SurrealKV single-owner model и persistent lock regression.
- [x] Подтверждённые lock/transaction-conflict сообщения → retryable `Busy`.
- [x] Data/duplicate/serialization failures → non-retryable `AdapterExecution`.
- [x] Bounded retry `10/25/50/100 ms` для context-aware writes/publication.
- [x] Deadline/cancellation проверяются перед attempt и во время backoff.
- [x] Cancelled retry не делает следующую backend attempt.
- [-] Два remote SDK connections и 32 allocations: код/CI готовы, hosted evidence отсутствует.
- [ ] Детерминированно воспроизвести remote write conflict и проверить public `Busy` mapping.
- [ ] Определить semantics уже выполняющегося SDK request.

#### Daemon cancellation bridge

- [x] Application token хранит optional core `CancellationHandle` в shared state.
- [x] Cancel любого clone выставляет app flag и core cancellation state.
- [x] Cancellation до binding передаётся при binding.
- [x] Rebind одного token к другому operation id запрещён.
- [x] Scheduler bind выполняется до вставки token в registry.
- [x] Compatibility scheduler сохранён для test/legacy paths.
- [x] Index/generate/wiki/html используют operation-aware scheduler.
- [x] Rollback использует plain abort и не блокируется user cancellation.
- [-] Daemon request contexts уникальны; watcher index использует сериализованный `daemon.index`.
- [ ] Устранить глобальное объединение независимых contexts с одинаковым operation id.
- [ ] Read-only daemon commands перевести на cancellable lifecycle.
- [ ] Связать CLI и MCP cancellation с core handle.

#### Native SurrealDB batch transaction

- [x] Locked backend: SurrealDB `2.6.5`.
- [x] Все arrays сериализуются до transaction.
- [x] Entity/fact/relation/diagnostic inserts выполняются в одном `BEGIN`/`COMMIT` query.
- [x] Statement failures проверяются через `response.check()`.
- [x] Duplicate-ID rollback regression и clean retry.
- [x] Prepared snapshot запрещает late writes.
- [-] Counter allocation и snapshot-record creation — разные requests.
- [-] Batch data transaction не включает commit marker.
- [-] Abort удаляет rows и metadata разными requests.

#### Prepared publication protocol

- [x] Ввести backend-neutral `PreparedSnapshot` в application publication API.
- [x] Сериализовать handle как snapshot identity без backend-specific wire format.
- [x] `prepare_publication` использует context-aware prepare и возвращает typed handle.
- [x] `publish_prepared` использует context-aware commit.
- [x] `abort_prepared` использует plain abort вне user cancellation/deadline.
- [x] `AthanorStore` делегирует все context-aware write/publication methods inner backend.
- [x] Regression-test различает context batch/prepare/commit и plain abort.
- [-] Index pipeline готовит snapshot, но coordinator journal ещё хранит raw `SnapshotId`.
- [ ] Перевести index journal, commit и recovery на `PreparedSnapshot`.
- [ ] Добавить conformance test typed prepare→publish/abort для Memory, JSONL и SurrealDB.

#### Transactional publication

- [ ] Объединить canonical batch и commit marker в один проверяемый backend protocol.
- [ ] Добавить backend-neutral Fact verification.
- [ ] Ввести immutable generation id для canonical/state/read models.
- [ ] Переключать один current pointer после подготовки всех artifacts.
- [ ] Fault injection: counter/create/write/prepare/commit/pointer/recovery.

#### Реализация P0.4

1. Shared conformance и coordination:
   - `1f1555c992a02db60eb1fbf7af38afa9e6e59610`
   - `d9bafc703de583cc70c8ce0fcea2714a447e05e2`
   - `615f4f7b2237385930ef53b9f8c8d6ff30a0443c`
2. Native transaction и rollback:
   - `664ee557526060fc5bce85111aa4f262d75606c5`
   - `5162733283dfd18c298e8b47be09808095f0ac39`
3. Embedded ownership/retry:
   - `c716bd35982c49f56212b4de02615f085e3464f0`
   - `036d79f02b7481f00678b14336b2f579550285e5`
4. Remote conformance/deadline retry:
   - `7f86d8e180c036c6a4962512636bbff849094925`
   - `8a05d3ae1155d12800dcd191b4e7589dd87b260c`
   - `d3f3c421b00b7b4df8b32708b1afb5968802f105`
   - `7c95fcb8049641f984d2200118edc6521bf04ffb`
5. Core cancellation-aware retry:
   - `68a388b5c1616d7225f2ad20387f761b39af47f0`
   - `877e3c068d37e90882e00ad68ae7eaab22bf9071`
6. Daemon cancellation bridge:
   - `425ac8730ca83f0eab88a82e52699f2efb13510d`
   - `270d64bbcce511b1e0b4ec6b8af743fdfe005395`
   - `e7cf5acc0d1e184075eac44691fcb067c4a20ec6`
   - `767983285fb28932648cf52cd299e67e3b24ee52`
7. Prepared publication/context forwarding:
   - `96fa47190209af82f0dc603eaeeae5ddcdabf200`
   - `5ff81251915cfcc810d047133f3bd5bebdf2fa50`
   - `8642b53dfb59fa00555bc632c879d28400131714`
   - `01274a3d9c9601d3a304309ed09a6f5af07bd069`
   - `837506aa16954cf8bb0f84887b08b8e54b6a86cf`

**Definition of Done:** Memory, JSONL и persistent SurrealDB имеют эквивалентный observable lifecycle; independent writers не теряют updates; retries bounded attempts/deadline/cancellation; typed prepared state используется coordinator; partial writes не публикуются; после crash видна только предыдущая или новая целая generation.

## 4. P1 — execution safety и maintainability

### P1.1. External process sandbox hardening

- [ ] `clean_environment` как production default и environment allowlist.
- [ ] Windows Job Objects.
- [ ] Linux sandbox/launcher boundary.
- [ ] Network/filesystem/CPU/memory/process limits.
- [ ] Timeout/cancellation/orphan/limit tests.

### P1.2. CLI decomposition

- [ ] Оставить в `main.rs` bootstrap, parse, dispatch и top-level errors.
- [ ] Разнести handlers/rendering.
- [ ] Help/exit-code/JSON contract tests.

### P1.3. Runtime decomposition

- [ ] Focused process contracts/builders.
- [ ] Composition wiring вне legacy paths.
- [ ] Cancellation-aware `ProcessRunner` через composition.
- [ ] Удалить legacy global registry.

### P1.4. Installer verification

**Статус:** `[-]` — реализация готова; hosted/tag evidence отсутствует.

- [x] Per-binary checksums, fail-closed Linux/Windows installers и tamper smoke.
- [x] Archive/Sigstore/provenance verification docs.
- [ ] Подтвердить hosted smoke и следующий version tag.

### P1.5. Operation context и protocol E2E

**Статус:** `[-]` — core primitive, Surreal retry, wrapper forwarding и daemon write-job propagation реализованы; все transports ещё не покрыты.

- [-] Deadline propagation CLI→app, daemon→app, MCP→app.
- [x] Cloneable core cancellation handle и stable non-retryable `Cancelled`.
- [x] Cancellation state не меняет JSON shape `OperationContext`.
- [x] SurrealDB retry bounded deadline/cancellation.
- [x] `AthanorStore` сохраняет backend context overrides.
- [x] Daemon index/generate/wiki/html cancellation доходит до core context.
- [ ] Read-only daemon jobs и queries.
- [ ] CLI/MCP cancellation lifecycle.
- [ ] Cancellation queued/running reads.
- [ ] Cooperative/isolated synchronous projectors.
- [ ] Rollback/recovery вне request deadline/cancellation.

### P1.6. Aggregate performance budgets

- [ ] Aggregate byte budget, peak RSS и phase timings.
- [ ] 10k/50k/100k scenarios.
- [ ] Incremental/full ratio matrix.

## 5. P2 — governance и contributor experience

### P2.1. Branch protection

- [ ] Disable force push/deletion и direct push после bootstrap gates.
- [ ] Required: quality, feature matrix, store conformance, AppSec, docs.
- [ ] Stale review dismissal, up-to-date policy и secret push protection.

### P2.2. Traceability

- [ ] Issue на каждый открытый P0/P1 package.
- [ ] Acceptance criteria и verification commands.
- [ ] Commit/PR links и синхронизация issue status.

### P2.3. Commit/release policy

- [ ] Conventional commit lint.
- [ ] Automated changelog/release notes.
- [ ] Version/tag/Cargo consistency.

### P2.4. Локальная воспроизводимость

- [x] Feature, coverage, installer/release, AppSec и store commands задокументированы.
- [x] Prepared publication regression command зафиксирован в плане.
- [ ] Common CI failures для остальных workflows.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. P0.4 — получить hosted compile/test/fmt/Clippy evidence для prepared-handle slice.
2. P0.4 — мигрировать index coordinator/journal/recovery на `PreparedSnapshot`.
3. P0.4 — объединить canonical data и commit marker для SurrealDB.
4. P0.4 — детерминированно воспроизвести remote transaction conflict.
5. P1.5 — устранить operation-id registry collision и покрыть read-only daemon/CLI/MCP.
6. P0.4 — Fact conformance, generation pointer и fault injection.
7. P0.3 — hosted AppSec evidence и blocking Zizmor.
8. P0.1/P1.4 — hosted matrices, installers и tag evidence.
9. P1.1 sandbox, decomposition и performance budgets.
10. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный пункт:** `P0.4 prepared publication coordinator migration`.

Следующий кодовый срез:

1. добавить `Option<PreparedSnapshot>` в deferred pipeline result либо отдельный prepared output type;
2. записывать typed handle в index publication journal;
3. заменить direct `commit_snapshot` на `publish_prepared`;
4. заменить recovery/rollback raw snapshot path на `abort_prepared` с backward-compatible journal parsing;
5. добавить tests: journal round-trip, publish через context override, rollback вне cancellation;
6. после этого спроектировать SurrealDB data+marker atomic protocol.

## 8. Definition of Done проекта

- Public queries изолированы committed snapshot.
- Memory, JSONL и SurrealDB проходят общий conformance suite.
- Publication crash-safe и atomic на generation boundary.
- Embedded ownership fail-closed; remote writers не теряют updates.
- Busy retry bounded attempts/deadline/cancellation.
- Daemon/CLI/MCP имеют эквивалентную cancellation semantics.
- External adapters bounded, cancellable и не оставляют orphan processes.
- Optional feature и AppSec gates блокируют regressions.
- Installers/release assets fail-closed, включая SBOM/provenance.
- CLI/runtime не остаются god modules.
- Roadmap, plan, issues и release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-14 — prepared publication handle и context forwarding

- Добавлены `PreparedSnapshot` и `PreparedSnapshotPublication`.
- Handle сериализуется как snapshot identity.
- Prepare/publish используют context-aware backend methods; abort остаётся plain cleanup.
- Исправлен `AthanorStore`: wrapper больше не теряет SurrealDB context overrides.
- Добавлен integration test, различающий context batch/prepare/commit и plain abort.
- Production index coordinator пока не мигрирован и продолжает использовать raw `SnapshotId`.
- P0.4 остаётся `[-]`; активным назначен coordinator migration.

### 2026-07-14 — daemon cancellation bridge

- Application token и daemon registry связаны с core cancellation handle.
- Index/generate/wiki/html переведены на operation-aware scheduler.
- Rollback остаётся plain abort.

### 2026-07-13 — transaction, retry, remote и security slices

- Добавлены native batch transaction, rollback regression, atomic counter, embedded ownership, remote two-client tests и Docker job.
- Добавлены cancellation-aware retry, AppSec, release integrity, coverage measurement и feature matrix.
