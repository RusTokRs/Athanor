# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `6a92a75fba53e2b96dc3aa93859f09bddf47b70d`  
> Дата актуализации: 2026-07-14  
> Статус: active implementation plan

## 0. Правила исполнения

Статусы:

- `[x]` — код/контракт реализован и имеет воспроизводимую проверку;
- `[-]` — частичная реализация либо отсутствует hosted evidence;
- `[ ]` — не начато;
- `[!]` — блокирует следующий обязательный этап.

Правила:

1. Один коммит — одна логически завершённая часть.
2. Не назначать thresholds без фактического измерения.
3. Не считать hosted/platform пункт выполненным без run/status evidence.
4. Backend transaction для `SnapshotBatch` не равна atomic generation publication, пока commit marker и read models переключаются отдельно.
5. Embedded SurrealKV exclusive ownership и remote multi-writer semantics — разные контракты.
6. Server-dependent tests не должны неявно входить в self-contained `--all-features` graph.
7. Один `operation_id` может иметь только одну живую cancellation authority; дополнительные владельцы clone существующий handle.
8. Cancellation/deadline не блокируют rollback и recovery.
9. Production migration считается выполненной только когда публичный `index` path использует typed journal/recovery/publish; test-only legacy код учитывается как cleanup debt.
10. Legacy journal v1 должен читаться после перехода writer на v2.
11. Recovery journal не может управлять путями вне ожидаемых `.athanor` artifacts.
12. Store-level publication protocol принадлежит `athanor-core`; application crate может только re-export или orchestrate его.
13. После coordinator error нельзя автоматически abort snapshot, пока не проверено, что он не стал latest committed.

## 1. Текущий baseline

Реализовано в коде/workflows:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in SurrealDB;
- shared lifecycle conformance для Memory, JSONL и embedded SurrealDB;
- dedicated embedded matrix и opt-in Docker remote SurrealDB job;
- native SurrealQL transaction для полного `SnapshotBatch`;
- atomic snapshot counter, numeric sequence и prepared immutability;
- embedded SurrealKV single-owner contract и retryable `Busy` mapping;
- bounded Busy retry `10/25/50/100 ms` с deadline/cancellation polling;
- exclusive process-local cancellation identity lease;
- daemon cancellation bridge для index/generate/wiki/html;
- core-owned `PreparedSnapshot`/`PreparedSnapshotPublication` protocol;
- compatibility re-export через `athanor-app`;
- context-aware backend forwarding через `AthanorStore`;
- typed prepare→publish/abort regressions для Memory, JSONL и embedded SurrealDB;
- `IndexPublicationJournal` v2 с v1 compatibility, atomic persistence и path validation;
- typed coordinator с rollback/recovery;
- production `index` path через `index_runtime.rs`, typed startup recovery и `publish_prepared_index`;
- production regression на совпадение canonical/read-model/index-state snapshot identities;
- feature, quality, AppSec, coverage, installer и release workflows.

Локальные проверки:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-core cancellation --locked
cargo test -p athanor-core prepared_publication --locked
cargo test -p athanor-app cancellation --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app production_index_runtime_publishes_one_typed_generation --locked
cargo test -p athanor-app --test prepared_publication --locked
cargo test -p athanor-store-memory --test prepared_publication --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-store-surrealdb --test prepared_publication --locked
ATHANOR_SURREAL_REMOTE_URI=ws://127.0.0.1:8000 \
  cargo test -p athanor-store-surrealdb --features remote --test remote --locked -- --ignored
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 2. Сводный статус

| Область | Статус | Подтверждено | Остаётся |
| --- | --- | --- | --- |
| Query/snapshot isolation | `[-]` | committed-only reads, prepare/abort lifecycle, embedded suites | Hosted remote evidence и generation visibility |
| Atomic publication | `[-]` | typed production coordinator, JSONL staging/recovery, Surreal batch rollback | Hosted evidence, data+marker protocol, generation pointer |
| Concurrent writers | `[-]` | JSONL lock, atomic counter, embedded ownership, remote two-client test configured | Hosted remote conflict evidence |
| Operation context | `[-]` | deadline, cancellation lease, daemon write-job bridge, typed publish context | Read commands, CLI/MCP, in-flight SDK interruption |
| Store transaction boundary | `[-]` | native batch transaction, core prepared protocol, three backend regressions, typed runtime recovery | Fact contract, full lifecycle transaction, legacy test extraction |
| Hosted CI governance | `[-]` | matrices/workflows существуют | Hosted evidence, branch protection, required checks |
| AppSec | `[-]` | dependency review, CodeQL, Gitleaks, blocking Zizmor, SBOM | Hosted evidence и push protection |
| Release integrity | `[-]` | fail-closed installers, signed assets/SBOM | Hosted/tag evidence |
| Default build | `[x]` | SurrealDB remote path остаётся opt-in | Maintain boundary |

## 3. P0 — release-blocking quality and security

### P0.1. CI feature matrix

**Статус:** `[-]` — implementation готов, hosted evidence отсутствует.

- [x] default/no-default, `store-surreal`, `js-ts-precision`, `--all-features`.
- [x] Check/test/Clippy с `--locked`.
- [x] Remote server tests изолированы от обычного `--all-features`.
- [x] SHA-pinned actions и explicit Rust `1.95.0`.
- [ ] Подтвердить hosted run всех slices.
- [ ] Сделать checks required после branch protection.

### P0.2. Coverage measurement

**Статус:** `[-]` — measurement есть, blocking floor не обоснован данными.

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV, JSON summary и HTML artifact.
- [ ] Получить первый hosted artifact.
- [ ] Решить по данным, нужен общий floor или targeted gate.

### P0.3. AppSec и software supply chain

**Статус:** `[-]` — технические gates реализованы, hosted enforcement не подтверждён.

- [x] Dependency review с порогом `moderate`.
- [x] CodeQL v4/Rust `security-extended`.
- [x] Gitleaks full history.
- [x] Pinned blocking Zizmor `1.26.1` для high/high findings.
- [x] Immutable SHA pins.
- [x] CycloneDX SBOM, checksum, Sigstore и provenance.
- [x] Release `verify` блокирует `publish`.
- [ ] Подтвердить hosted AppSec/tag release.
- [ ] Подтвердить repository secret push protection.

### P0.4. Store conformance и transactional publication

**Статус:** `[-]` — typed production runtime включён; hosted evidence, legacy test extraction и generation-level atomicity остаются.

#### Shared backend contract

- [x] `SnapshotBatch → prepare → commit/abort` lifecycle.
- [x] Невидимость uncommitted/prepared snapshot.
- [x] Запрет abort committed snapshot.
- [x] Aborted snapshot не меняет `LatestCommitted`.
- [x] Memory, JSONL и embedded SurrealDB используют shared suite.
- [x] Dedicated embedded matrix и isolated remote job.
- [-] Remote entity+fact test настроен, hosted run отсутствует.
- [-] Общий `KnowledgeStore` fact-query отсутствует.

#### SurrealDB writer safety

- [x] Process-local write gate для cloned handles.
- [x] Atomic counter и numeric sequence.
- [x] Embedded SurrealKV single-owner regression.
- [x] Confirmed lock/transaction conflicts → retryable `Busy`.
- [x] Data/duplicate/serialization failures → non-retryable adapter error.
- [x] Context-aware bounded retry с deadline/cancellation.
- [-] Remote two-client/32-allocation test настроен, hosted evidence отсутствует.
- [ ] Детерминированно воспроизвести remote write conflict.
- [ ] Определить semantics уже выполняющегося SDK request.

#### Cancellation bridge

- [x] Exclusive cancellation lease на active `operation_id`.
- [x] Duplicate active registration → non-retryable `Conflict`.
- [x] Same-token/same-id bind идемпотентен.
- [x] Daemon index/generate/wiki/html используют operation-aware scheduler.
- [x] Typed runtime повторно использует тот же `OperationContext` для publish.
- [x] Rollback/recovery выполняются plain path вне user cancellation.
- [ ] Read-only daemon commands перевести на cancellable lifecycle.
- [ ] Связать CLI и MCP cancellation с core handle.

#### Native SurrealDB transaction

- [x] SurrealDB `2.6.5` locked backend.
- [x] Entity/fact/relation/diagnostic arrays выполняются в одном `BEGIN`/`COMMIT` query.
- [x] Statement failures проверяются через `response.check()`.
- [x] Duplicate-ID rollback и clean retry regression.
- [x] Prepared snapshot запрещает late writes.
- [-] Counter allocation и snapshot-record creation — разные requests.
- [-] Batch transaction не включает commit marker.
- [-] Abort удаляет rows и metadata разными requests.

#### Prepared publication protocol

- [x] `PreparedSnapshot` и `PreparedSnapshotPublication` принадлежат `athanor-core` рядом с `KnowledgeStore`.
- [x] `athanor-app` сохраняет прежний API через compatibility re-export.
- [x] Prepare/publish context-aware; abort cancellation-independent.
- [x] Cancellation race после backend prepare не теряет cleanup handle.
- [x] `AthanorStore` сохраняет backend context overrides.
- [x] Recording regression для context batch/prepare/commit и plain abort.
- [x] Memory typed lifecycle regression.
- [x] JSONL typed lifecycle regression.
- [x] Embedded SurrealDB typed lifecycle regression.

#### Typed index journal и production coordinator

- [x] Journal v2 хранит serialized `PreparedSnapshot`.
- [x] Journal v1 читается и нормализуется в v2.
- [x] Unknown schema/fields fail-closed.
- [x] Atomic write/load/clear с filesystem regression.
- [x] Journal load проверяет expected read-model/index-state paths.
- [x] Publication id запрещает path separators/traversal.
- [x] `publish_prepared_index` реализует journal → read model/state prepare → typed publish → finalize.
- [x] Publish failure откатывает application artifacts и вызывает `abort_prepared`.
- [x] `recover_interrupted_publication` различает committed и uncommitted typed journals.
- [x] Public `index` module переключён на `index_runtime.rs`.
- [x] Deferred-prepared boundary превращается в `PreparedSnapshot` перед coordinator call.
- [x] Startup использует typed v1/v2 recovery до загрузки previous state.
- [x] Production path не содержит local journal v1 или direct `commit_snapshot`.
- [x] Coordinator error guard проверяет latest committed перед дополнительным abort.
- [x] `index_publication` больше не требует production `dead_code` allowance.
- [x] Production regression проверяет canonical/read-model/index-state snapshot identity и cleared journal.
- [-] Старый `index.rs` временно компилируется только как test-only legacy suite с локальным allowance.
- [ ] После hosted compile перенести broad incremental/validation tests и удалить legacy-файл.

Последние implementation-коммиты:

- `068ab8e795889d8b115ca0fa69196568f9ff7c09` — typed coordinator, recovery и path hardening;
- `5331c5ee7251eb2a5ac9e3b2aa68b7a86dec2cc0` — core-owned prepared publication protocol;
- `8ffd6b93b9c0813f9c1358f23952226ee2abfa37` — core exports;
- `8f75e74796d044ce42b829cdc319e074e8177a69` — app compatibility re-export;
- `c9f941b9314ef83864d5b0d3467514aa1d50866e` — Memory typed lifecycle regression;
- `4a87ca11da3ed9e7d00743fb0d9cd79d3d179963` — embedded SurrealDB typed lifecycle regression;
- `710bd6f0cb89ac4f6df39cbf2a7246dc9fb03f6a` — typed production runtime module;
- `a8b53fb753c54d77b8662f28acb3c8ca35d3d78a` — coordinator error ownership fix;
- `3cd88de3169f833b6e5d3ba9636d35fd843abb90` — production module cutover;
- `3a8adc1afde9601554450a64e540f904986c3c76` — production runtime regression;
- `6d1cdcd5de79ca08e7f68075861a3bec1072d3a6` — Clippy import correction;
- `fbfd782820653db974c7e0fb18ed6d8594cb8718` — test registration;
- `6a92a75fba53e2b96dc3aa93859f09bddf47b70d` — architecture documentation.

#### Следующий transactional layer

- [ ] Объединить canonical data и commit marker в один backend protocol.
- [ ] Добавить backend-neutral Fact verification.
- [ ] Ввести immutable generation id для canonical/state/read models.
- [ ] Переключать один current pointer после подготовки всех artifacts.
- [ ] Fault injection: counter/create/write/prepare/commit/pointer/recovery.

## 4. P1 — execution safety и maintainability

### P1.1. External process sandbox hardening

- [ ] `clean_environment` production default и environment allowlist.
- [ ] Windows Job Objects.
- [ ] Linux sandbox/launcher boundary.
- [ ] Network/filesystem/CPU/memory/process limits.
- [ ] Timeout/cancellation/orphan/limit tests.

### P1.2. CLI decomposition

- [ ] Оставить в `main.rs` bootstrap/parse/dispatch/top-level errors.
- [ ] Разнести handlers/rendering.
- [ ] Help/exit-code/JSON contract tests.

### P1.3. Runtime decomposition

- [ ] Focused process contracts/builders.
- [ ] Composition wiring вне legacy paths.
- [ ] Cancellation-aware `ProcessRunner` через composition.
- [ ] Удалить legacy global registry.

### P1.4. Installer verification

**Статус:** `[-]` — code готов, hosted/tag evidence отсутствует.

- [x] Per-binary checksums и fail-closed Linux/Windows installers.
- [x] Tamper smoke и archive/Sigstore/provenance docs.
- [ ] Подтвердить hosted smoke и version tag.

### P1.5. Operation context и protocol E2E

**Статус:** `[-]` — daemon write path покрыт, все transports ещё нет.

- [x] Cloneable core cancellation handle и stable `Cancelled`.
- [x] JSON shape `OperationContext` не изменён.
- [x] Exclusive identity lease и reuse после drop.
- [x] SurrealDB retry bounded deadline/cancellation.
- [x] Daemon write-job propagation.
- [x] Typed index publish использует bound context; rollback/recovery cancellation-independent.
- [ ] Read-only daemon jobs/queries.
- [ ] CLI/MCP cancellation lifecycle.
- [ ] Cancellation queued/running reads.
- [ ] Cooperative/isolated synchronous projectors.

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

- [x] Feature, coverage, AppSec, installer/release и store commands.
- [x] Cancellation, backend publication и production runtime regression commands.
- [ ] Common CI failures для остальных workflows.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. P0.4 — получить hosted compile/test/fmt/Clippy evidence для typed runtime cutover.
2. P0.4 — исправить реальные hosted findings, не повышая статус без evidence.
3. P0.4 — перенести broad tests из test-only `index.rs` и удалить legacy-файл.
4. P0.4 — объединить canonical data и commit marker.
5. P0.4 — детерминированно воспроизвести remote transaction conflict.
6. P1.5 — read-only daemon/CLI/MCP cancellation.
7. P0.4 — Fact conformance, generation pointer и fault injection.
8. P0.3/P0.1/P1.4 — hosted AppSec, matrices, installers и tag evidence.
9. P1.1, decomposition и performance budgets.
10. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный пункт:** `P0.4 hosted verification and legacy-test extraction after typed runtime cutover`.

Следующий кодовый срез после evidence:

1. перенести incremental, validation и cancellation tests из test-only `index.rs` в focused test modules;
2. удалить `legacy_index` registration и локальный journal v1 implementation;
3. удалить старый `crates/athanor-app/src/index.rs`;
4. сохранить production `index_runtime.rs` public surface без изменений;
5. после clean workspace checks перейти к canonical data+commit-marker protocol.

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

### 2026-07-14 — typed production index runtime cutover

- Добавлен focused `index_runtime.rs`, сохраняющий прежний public API и `repo_id_for_root`.
- Startup recovery переключён на typed journal v1/v2 recovery.
- Deferred pipeline output превращается в `PreparedSnapshot` и публикуется через `publish_prepared_index` с тем же `OperationContext`.
- Direct `commit_snapshot` и local journal v1 исключены из production path.
- Coordinator error guard проверяет latest committed перед дополнительным cancellation-independent abort; это закрывает leak при ошибке записи journal.
- Добавлен production regression на согласованность canonical/read-model/index-state snapshot identities, cleared journal и no-change rerun.
- Старый `index.rs` сохранён только как временная test-only suite; production на него больше не ссылается.
- Hosted compile/test/fmt/Clippy evidence пока отсутствует, поэтому P0.4 остаётся `[-]`.

### 2026-07-14 — core-owned prepared protocol и backend matrix

- `PreparedSnapshot`/`PreparedSnapshotPublication` перенесены рядом с `KnowledgeStore` в `athanor-core`.
- `athanor-app` сохраняет source compatibility через re-export.
- Добавлены Memory и embedded SurrealDB typed lifecycle regressions.
- JSONL, Memory и embedded SurrealDB используют один typed protocol.

### 2026-07-14 — typed index coordinator boundary

- Добавлены `publish_prepared_index` и typed `recover_interrupted_publication`.
- Publish boundary сохраняет journal до canonical publish и finalization application artifacts.
- Recovery сохраняет committed generation либо восстанавливает backups для uncommitted snapshot.
- Journal paths проверяются против ожидаемого project root; publication id не допускает traversal.

### 2026-07-14 — cancellation и prepared protocol

- Введена exclusive cancellation identity lease и idempotent application binding.
- Daemon write jobs связаны с core cancellation.
- Добавлен typed prepare/publish/abort protocol и context-forwarding wrapper.

### 2026-07-13 — transaction, remote и security slices

- Добавлены native batch transaction, rollback regression, atomic counter, embedded ownership, remote two-client tests, AppSec, release integrity, coverage measurement и feature matrix.
