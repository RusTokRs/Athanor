# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Назначение: рабочий implementation plan для последовательного улучшения Athanor  
> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `08dc4ea91b5f3b3159fc1f3678d7f60ff1ea1f51`  
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
2. Не назначать числовые thresholds без фактического измерения.
3. Не переводить hosted/platform пункт в `[x]`, пока нет доступного run/status evidence.
4. Изменения архитектуры, release, CI и security одновременно отражать в документации и этом плане.
5. Пока разрешены прямые коммиты в `main`, сохранять Conventional Commits и воспроизводимые проверки. После branch protection перейти на PR.
6. Отсутствие push-triggered workflow run в ответе GitHub-коннектора не является ни успехом, ни ошибкой.
7. Process-local serialization не считается cross-process lease.
8. Одна backend transaction для `SnapshotBatch` не считается атомарной publication всей generation, пока commit marker и read models переключаются отдельно.
9. Embedded exclusive ownership и remote multi-writer semantics — разные контракты.
10. Server-dependent tests не должны неявно запускаться в self-contained `--all-features` graph.
11. Cancellation handle, keyed by operation id, требует уникального id на время активной операции. Daemon request ids это обеспечивают; watcher index jobs сериализованы single-active guard.
12. Отмена не должна блокировать rollback/recovery, необходимые для восстановления целостности.

## 1. Текущий baseline

Подтверждено в коде и workflow:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in SurrealDB;
- shared query/lifecycle conformance для Memory, JSONL и embedded SurrealDB;
- dedicated embedded matrix и opt-in remote SurrealDB job;
- native SurrealQL transaction для полного `SnapshotBatch`;
- atomic snapshot counter, numeric sequence и prepared immutability;
- embedded SurrealKV exclusive-owner contract;
- stable retry mapping: transient lock/conflict → `Busy`, data/statement failures → non-retryable;
- bounded Busy retry `10/25/50/100 ms` с deadline и cancellation polling;
- process-local core `CancellationHandle` без изменения JSON wire shape;
- application `CancellationToken` bridge для daemon index/generate/wiki/html jobs;
- Linux/Windows/macOS quality matrix и optional-feature matrix;
- measurement-only coverage artifacts;
- dependency review, CodeQL v4, Gitleaks, Zizmor и immutable action pins;
- signed archives, installer checksums, signed CycloneDX SBOM и release verification.

Базовые команды:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-core cancellation --locked
cargo test -p athanor-app cancellation --locked
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
| Atomic publication | `[-]` | JSONL staging/recovery, SurrealDB atomic batch rollback | Commit marker и единый generation pointer |
| Concurrent writers | `[-]` | JSONL locking, Surreal atomic counter, embedded ownership, remote two-client test configured | Hosted remote conflict evidence |
| Operation context | `[-]` | deadlines, core cancellation, daemon write-job bridge | Read commands, CLI/MCP, in-flight SDK interruption |
| Storage transaction boundary | `[-]` | SnapshotBatch, native Surreal transaction, rollback regression | Portable prepared handle и atomic data+marker publish |
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

**Статус:** `[-]` — шестой daemon-cancellation slice реализован; hosted evidence, observed remote conflict и generation publication остаются.

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
- [x] Bounded retry `10/25/50/100 ms` только для context-aware writes/publication.
- [x] Deadline проверяется перед попыткой и backoff.
- [x] Core cancellation проверяется перед попыткой и каждые ≤5 ms во время backoff.
- [x] Cancelled retry не делает следующую backend attempt.
- [-] Два remote SDK connections и 32 allocations: код/CI готовы, hosted evidence отсутствует.
- [ ] Детерминированно воспроизвести remote write conflict и проверить public `Busy` mapping.
- [ ] Определить semantics уже выполняющегося SDK request.

#### Daemon cancellation bridge

- [x] Application token хранит optional core `CancellationHandle` в shared state.
- [x] Cancel любого clone выставляет app flag и core cancellation state.
- [x] Cancellation, запрошенная до binding, передаётся при binding.
- [x] Rebind одного token к другому operation id запрещён.
- [x] Operation-aware scheduler выполняет binding до вставки token в daemon registry.
- [x] Compatibility scheduler без context сохранён для test/legacy paths.
- [x] Index, generate, wiki и HTML report используют operation-aware scheduler.
- [x] Index передаёт тот же context в `begin`, `put_snapshot`, `prepare`, `commit`.
- [x] Rollback использует plain abort и не блокируется user cancellation.
- [-] Daemon request contexts уникальны; watcher index использует сериализованный `daemon.index`.
- [ ] Read-only daemon commands перевести на cancellable operation lifecycle.
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

#### Transactional publication

- [ ] Ввести backend-neutral prepared publication handle.
- [ ] Объединить batch data и commit marker в один проверяемый protocol.
- [ ] Добавить backend-neutral Fact verification.
- [ ] Ввести immutable generation id для canonical/state/read models.
- [ ] Переключать один current pointer после подготовки всех artifacts.
- [ ] Fault injection: counter/create/write/prepare/commit/pointer/recovery.

#### Реализация P0.4

1. Shared conformance и process-local coordination:
   - `1f1555c992a02db60eb1fbf7af38afa9e6e59610`
   - `d9bafc703de583cc70c8ce0fcea2714a447e05e2`
   - `615f4f7b2237385930ef53b9f8c8d6ff30a0443c`
2. Native transaction и rollback:
   - `664ee557526060fc5bce85111aa4f262d75606c5`
   - `5162733283dfd18c298e8b47be09808095f0ac39`
3. Embedded ownership/retry classification:
   - `c716bd35982c49f56212b4de02615f085e3464f0`
   - `036d79f02b7481f00678b14336b2f579550285e5`
4. Remote conformance и deadline retry:
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
   - `6d9ce2c3785db2e6d681d3f1fa5919c0d24cb6b4`
   - `767983285fb28932648cf52cd299e67e3b24ee52`
   - `49b71c82af8eacc06bda2e61f9b434305b993e67`
   - `71ed635a26b9ec42dc2273e97d36acc2cc7a4c76`
   - `08dc4ea91b5f3b3159fc1f3678d7f60ff1ea1f51`

**Definition of Done:** Memory, JSONL и persistent SurrealDB имеют эквивалентный observable lifecycle; independent writers не теряют updates; retries bounded attempts/deadline/cancellation; partial writes не публикуются; после crash видна только предыдущая или новая целая generation.

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

**Статус:** `[-]` — core primitive, Surreal retry и daemon write-job propagation реализованы; все transports ещё не покрыты.

- [-] Deadline propagation CLI→app, daemon→app, MCP→app.
- [x] Cloneable core cancellation handle и stable non-retryable `Cancelled`.
- [x] Cancellation state не меняет JSON shape `OperationContext`.
- [x] SurrealDB retry bounded deadline/cancellation.
- [x] Daemon index/generate/wiki/html registry cancellation доходит до core context.
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
- [ ] Common CI failures для остальных workflows.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. P0.4 — получить hosted embedded/remote results и исправить реальные compile/fmt/Clippy findings.
2. P0.4 — ввести backend-neutral publication method, объединяющий data и commit marker для SurrealDB.
3. P0.4 — детерминированно воспроизвести remote transaction conflict.
4. P1.5 — read-only daemon и CLI/MCP cancellation propagation.
5. P0.4 — Fact conformance, generation pointer и fault injection.
6. P0.3 — hosted AppSec evidence и blocking Zizmor.
7. P0.1/P1.4 — hosted matrices, installers и tag evidence.
8. P1.1 sandbox и P1.5 protocol matrix.
9. Coverage decision, decomposition и performance budgets.
10. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный пункт:** `P0.4 transactional publication`.

Следующий кодовый срез:

1. добавить backend-neutral `publish_snapshot`/prepared-publication contract;
2. для SurrealDB объединить canonical batch и committed marker в одну backend transaction либо доказуемый idempotent protocol;
3. сохранить JSONL staging/recovery semantics;
4. добавить regression: fault до marker не виден readers, success виден целиком;
5. rollback/recovery не должны прекращаться из-за user cancellation;
6. не переходить к generation pointer до закрепления data+marker contract тестами.

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

### 2026-07-14 — daemon cancellation bridge

- Application `CancellationToken` получил shared optional core handle.
- Registry clone и running-task clone отменяют один `OperationContext`.
- Binding выполняется scheduler до публикации token в registry.
- Index/generate/wiki/html переведены на operation-aware scheduler.
- Index storage publication использует тот же context для begin/write/prepare/commit.
- Rollback остаётся plain abort, чтобы cancellation не блокировала cleanup.
- Compatibility scheduler сохранён для tests/legacy paths.
- P0.4/P1.5 остаются `[-]` до hosted evidence и полной transport propagation.

### 2026-07-13 — cancellation-aware retry

- Добавлен core cancellation handle без изменения wire shape.
- SurrealDB backoff проверяет cancellation перед attempts и во время sleep.

### 2026-07-13 — remote/transaction slices

- Добавлены native batch transaction, rollback regression, atomic counter, embedded ownership, remote two-client tests и Docker job.
- AppSec, release integrity, coverage measurement и feature matrix синхронизированы с планом.
