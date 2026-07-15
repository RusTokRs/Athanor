# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `6b012786521327fb4fdcfe8b0f5dccfd5e977561`  
> Дата актуализации: 2026-07-15  
> Статус: active implementation plan

## 0. Правила исполнения

Статусы:

- `[x]` — реализация и воспроизводимая проверка добавлены;
- `[-]` — реализация частичная либо отсутствует hosted evidence;
- `[ ]` — не начато;
- `[!]` — внешний blocker для обязательного следующего этапа.

Правила:

1. Коммиты идут напрямую в `main`, пока пользователь явно не изменит режим работы.
2. Один коммит — одна логически завершённая часть.
3. Не считать hosted/platform пункт выполненным без run/status evidence.
4. Не назначать coverage threshold без фактического измерения.
5. Cancellation/deadline не должны блокировать rollback или recovery.
6. Journal v1 обязан читаться после перехода writer на v2.
7. Recovery не может удалять или переименовывать paths вне ожидаемых `.athanor` artifacts.
8. Recovery обязана fail-closed до destructive mutation при неверном типе, schema или snapshot identity.
9. Historical index-state backup может использовать валидную предыдущую `athanor.index_state.vN` schema.
10. Backend-neutral facts читаются только через committed canonical snapshots.
11. Data и committed marker считаются atomic только при одном backend-specific publication boundary.
12. Успешный durable publish нельзя отменять повторной cancellation/deadline проверкой после boundary.
13. Backend batch transaction не равна atomic generation publication, пока application artifacts и current pointer переключаются отдельно.
14. Store-level typed publication protocol принадлежит `athanor-core`.

## 1. Текущий baseline

Реализовано:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in embedded/remote SurrealDB;
- shared query/lifecycle/atomic conformance для Memory, JSONL и embedded SurrealDB;
- core-owned `FactQuery`/`FactQueryStore` и committed-only blanket implementation;
- core-owned additive `AtomicSnapshotPublication` capability;
- Memory публикует complete batch и committed marker одной mutex-секцией;
- JSONL публикует exact generation и `commit.json` одним directory rename;
- JSONL exact/latest reads валидируют manifest и declared marker schema/snapshot identity;
- legacy JSONL manifest без marker declaration остаётся читаемым;
- JSONL pointer-finalization failure сохраняет exact committed generation non-abortable;
- SurrealDB одной SurrealQL transaction заменяет rows и ставит `prepared/committed` marker;
- SurrealDB facade retry’ит всю atomic boundary только для подтверждённого `Busy`;
- duplicate-record regression проверяет rollback rows и marker с последующим successful retry;
- configured remote independent reader использует atomic publication и public fact query;
- `AthanorStore` передаёт atomic capability через production trait object;
- активный production coordinator после durable journal/staging собирает complete `SnapshotBatch` и вызывает atomic boundary;
- publish-error и recovery определяют commit journal snapshot через exact canonical probe, а не только `LatestCommitted`;
- exact JSONL commit + failed `latest.json` regression сохраняет journal/application artifacts и запрещает abort;
- finalize и combined-error fault hooks перенесены на atomic boundary;
- transient application store реализует atomic replacement semantics;
- atomic snapshot counter, numeric sequence и prepared immutability;
- embedded SurrealKV single-owner contract и retryable `Busy` mapping;
- bounded Busy retry `10/25/50/100 ms` с deadline/cancellation polling;
- exclusive process-local cancellation identity lease;
- daemon cancellation bridge для index/generate/wiki/html;
- core-owned `PreparedSnapshot`/`PreparedSnapshotPublication` compatibility authority;
- `IndexPublicationJournal` v2 с v1 compatibility, atomic persistence и path validation;
- production index runtime через `index_runtime.rs`;
- broad incremental/validation/cancellation tests на production runtime;
- legacy `crates/athanor-app/src/index.rs` удалён;
- deterministic publication fault suite для prepare/publish/finalize/clear/recovery;
- recovery preflight валидирует artifact type, schema и snapshot identity до mutation;
- committed/uncommitted repeated recovery проверена как idempotent после cleanup;
- historical state backup допускает строгую versioned schema;
- feature, quality, AppSec, coverage, installer и release workflow-файлы.

Платформенное состояние:

- [!] публичная GitHub Actions page не показывает workflow list/runs и отображает onboarding;
- [!] connector не возвращает push-run/status evidence для текущего HEAD;
- [!] локальная среда не содержит Rust toolchain, а DNS-доступ к GitHub clone заблокирован;
- [ ] проверить/включить Actions в repository или organization settings;
- [ ] после активации получить первый push-run и исправить реальные findings.

Локальные/hosted verification commands:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings

cargo test -p athanor-core fact_query --locked
cargo test -p athanor-app fact_query --locked
cargo test -p athanor-store-memory --test fact_query --locked
cargo test -p athanor-store-jsonl --test fact_query --locked
cargo test -p athanor-store-surrealdb --test fact_query --locked

cargo test -p athanor-store-memory --test atomic_publication --locked
cargo test -p athanor-store-jsonl --test atomic_publication --locked
cargo test -p athanor-store-surrealdb --test atomic_publication --locked

cargo test -p athanor-store-memory --test conformance --locked
cargo test -p athanor-store-jsonl --test conformance --locked
cargo test -p athanor-store-surrealdb --test conformance --locked

cargo test -p athanor-app index_runtime_tests --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-app index_publication_fault_tests --locked
cargo test -p athanor-app index_publication_finalize_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p athanor-app index_publication_content_tests --locked
cargo test -p athanor-app index_publication_combined_error_tests --locked
cargo test -p athanor-app --test prepared_publication --locked

ATHANOR_SURREAL_REMOTE_URI=ws://127.0.0.1:8000 \
  cargo test -p athanor-store-surrealdb --features remote --test remote --locked -- --ignored

cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 2. Сводный статус

| Область | Статус | Подтверждено | Остаётся |
| --- | --- | --- | --- |
| Query/snapshot isolation | `[-]` | committed-only entity/fact/relation/diagnostic queries, exact/latest suites | Hosted remote evidence, generation visibility |
| Atomic publication | `[-]` | three backend boundaries, shared conformance, production atomic coordinator, exact recovery | Remove pre-journal canonical mutation, generation pointer |
| Recovery safety | `[-]` | v1/v2 journal, type/content preflight, exact canonical probe, idempotence | Cryptographic integrity, one generation pointer |
| Concurrent writers | `[-]` | JSONL lock, atomic counter, embedded ownership | Hosted remote conflict evidence |
| Operation context | `[-]` | deadline, cancellation lease, daemon write path, atomic Busy retry | Read commands, CLI/MCP, in-flight SDK outcome semantics |
| Store transaction boundary | `[-]` | atomic data+marker for three backends and production final boundary | Eliminate deferred prewrite, one generation pointer |
| Runtime maintainability | `[-]` | focused runtime, active atomic coordinator, legacy compatibility isolated | Remove transitional coordinator/prepare dependency, further decomposition |
| Hosted CI governance | `[!]` | workflow-файлы существуют | Enable/verify Actions, runs, branch protection, required checks |
| AppSec | `[-]` | CodeQL, dependency review, Gitleaks, blocking Zizmor, SBOM configured | Hosted evidence, push protection |
| Release integrity | `[-]` | fail-closed installers, signed assets/SBOM configured | Hosted/tag evidence |
| Default build | `[x]` | remote SurrealDB остаётся opt-in | Maintain boundary |

## 3. P0 — release-blocking quality and security

### P0.1. CI feature matrix

**Статус:** `[!]` — implementation готов, GitHub Actions runs не видны.

- [x] default/no-default, `store-surreal`, `js-ts-precision`, `--all-features`.
- [x] Check/test/Clippy с `--locked`.
- [x] Remote server tests изолированы от обычного `--all-features`.
- [x] SHA-pinned actions и explicit Rust `1.95.0`.
- [!] Проверить/включить GitHub Actions.
- [ ] Подтвердить hosted run всех slices.
- [ ] Сделать checks required после branch protection.

### P0.2. Coverage measurement

**Статус:** `[!]` — measurement job настроен, hosted artifact заблокирован Actions.

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV, JSON summary и HTML artifact.
- [!] Получить первый hosted artifact после активации Actions.
- [ ] По данным выбрать общий floor или targeted gate.

### P0.3. AppSec и supply chain

**Статус:** `[!]` — gates настроены, hosted enforcement заблокирован Actions.

- [x] Dependency review с порогом `moderate`.
- [x] CodeQL v4/Rust `security-extended`.
- [x] Gitleaks full history.
- [x] Blocking Zizmor `1.26.1` high/high.
- [x] Immutable SHA pins.
- [x] CycloneDX SBOM, checksum, Sigstore и provenance.
- [x] Release `verify` блокирует `publish`.
- [!] Подтвердить hosted AppSec/tag release.
- [ ] Подтвердить repository secret push protection.

### P0.4. Store conformance и transactional publication

**Статус:** `[-]` — backend matrix и production final boundary atomic; deferred pipeline всё ещё создаёт/пишет/prepare’ит canonical snapshot до durable journal.

#### Shared backend contract

- [x] `SnapshotBatch → prepare → commit/abort` compatibility lifecycle.
- [x] Невидимость uncommitted/prepared snapshot.
- [x] Запрет abort committed snapshot.
- [x] Aborted snapshot не меняет `LatestCommitted`.
- [x] Memory, JSONL и embedded SurrealDB shared query/lifecycle/atomic suites.
- [x] Core-owned `FactQuery` и additive `FactQueryStore`.
- [x] Exact/latest fact queries и filters/limit включены в shared conformance.
- [-] Remote independent reader использует public `query_facts`, hosted run отсутствует.

#### SurrealDB writer safety

- [x] Process-local write gate для cloned handles.
- [x] Atomic counter и numeric sequence.
- [x] Embedded SurrealKV single-owner regression.
- [x] Confirmed lock/transaction conflicts → retryable `Busy`.
- [x] Data/duplicate/serialization failures → non-retryable adapter error.
- [x] Context-aware bounded retry whole atomic boundary.
- [-] Remote atomic independent-reader/32-allocation tests настроены, hosted evidence отсутствует.
- [ ] Детерминированно воспроизвести remote write conflict.
- [ ] Определить semantics уже выполняющегося/transport-ambiguous SDK request.

#### Cancellation bridge

- [x] Exclusive cancellation lease на active `operation_id`.
- [x] Duplicate active registration → `Conflict`.
- [x] Same-token/same-id bind идемпотентен.
- [x] Daemon index/generate/wiki/html используют operation-aware scheduler.
- [x] Atomic publication проверяет cancellation/deadline до durable boundary.
- [x] После successful durable publish cancellation повторно не проверяется.
- [x] Rollback/recovery cancellation-independent.
- [ ] Read-only daemon commands перевести на cancellable lifecycle.
- [ ] Связать CLI и MCP cancellation с core handle.

#### Typed journal и production atomic coordinator

- [x] Journal v2 хранит serialized `PreparedSnapshot`; v1 читается и нормализуется.
- [x] Unknown schema/fields, неверные paths/id fail-closed.
- [x] Journal write/load/clear atomic.
- [x] Active production alias переключён на `index_publication_atomic.rs`.
- [x] После durable journal и staging coordinator собирает complete `SnapshotBatch` из output.
- [x] Final canonical boundary использует `publish_snapshot_batch_with_context`.
- [x] Publish error exact-probes journal snapshot перед rollback/abort.
- [x] Exact committed generation сохраняет journal/application artifacts и не abort’ится.
- [x] Uncommitted/absent exact generation откатывает application artifacts и abort’ится.
- [x] Recovery определяет commit через exact canonical load/marker, не только `LatestCommitted`.
- [x] Exact commit + failed JSONL latest pointer + recovery regression добавлен.
- [x] Finalize sabotage hooks перенесены на atomic boundary.
- [x] Combined publish/rollback/abort failures сохраняют все причины.
- [x] Recovery preflight валидирует type/schema/snapshot identity до mutation.
- [x] Read-model/state backups обязаны представлять одну previous generation.
- [x] Historical state backup принимает строгую versioned schema.
- [x] Committed/uncommitted repeated recovery после cleanup идемпотентны.
- [-] Legacy guard/inner coordinator остаются как явно разрешённый transition layer.
- [!] Hosted compile/test/fmt/Clippy evidence отсутствует из-за Actions blocker.

#### Canonical data + commit marker

- [-] Backend и active final coordinator выполнены; pre-journal mutation остаётся.
- [x] Core-owned additive `AtomicSnapshotPublication` без compatibility fallback.
- [x] Memory complete replacement + marker в одной mutex-секции.
- [x] JSONL hidden staging, declared marker, exact directory rename и fail-closed validation.
- [x] Legacy JSONL manifest без marker declaration имеет explicit compatibility policy.
- [x] JSONL pointer-finalization error оставляет exact generation readable/non-abortable.
- [x] SurrealDB rows + marker одной transaction; statement failure откатывает всё.
- [x] SurrealDB facade retry’ит entire boundary только для classified `Busy`.
- [x] Общий atomic conformance подключён к Memory, JSONL и embedded SurrealDB.
- [-] Remote atomic regression настроен, hosted run отсутствует.
- [x] Production coordinator передаёт complete `SnapshotBatch` в atomic capability.
- [x] Recovery использует exact canonical commit probe.
- [x] Pointer-failure application recovery fault regression добавлен.
- [ ] Deferred pipeline: не вызывать `put_snapshot_with_context` до durable journal.
- [ ] Deferred pipeline: не вызывать `prepare_publication` до durable journal.
- [ ] Начальная snapshot allocation и cleanup authority должны иметь crash-safe journal semantics.
- [ ] После удаления prewrite убрать production dependence от prepared compatibility handle.

#### Следующий transactional layer

- [ ] Ввести immutable generation id для canonical/state/read models.
- [ ] Переключать один current pointer после подготовки всех artifacts.
- [ ] Восстанавливать/переключать backend latest pointer через generation protocol.
- [ ] Расширить fault injection на pointer switch и generation cleanup.
- [ ] Добавить cryptographic/content checksum для application artifacts.

Последние implementation-коммиты:

- `a15ae020f336a5146f2f32dea4db9fb32eb3f743` — core atomic capability;
- `ccd20127d77c59719f3bafc040552efd584be058` / `1fc5290e061ddb8dd13a79bba64b99cdd8514555` — JSONL marker validation и regressions;
- `ad9b662998f79006d3d3aef2b109e0a847bb4326` / `4de437bf0120ea156197e52e0d50308fecc1a48f` — SurrealDB transaction/facade;
- `753aa0bf1412f030a54723b4bc2bb797abe54d43` — shared atomic conformance;
- `a12d1f96b5f0fc8351fd5a92ae2c8e86464cdb0f` — application store atomic trait-object boundary;
- `04ff9d372f1fe89c63a6a3836ba4e7cae951dba2` — active atomic coordinator and exact recovery;
- `68a867eedb06726452af39fe3962ed718ed6e77f` — production alias cutover;
- `d4d8d1e7caeefb02f272edf192e4b1bff66d7cff` / `22d87a9cf6d97f7d2ae7bf33ee132e95c34dd72a` — pointer-failure recovery regression/registration;
- `a7a4058dd6b57b237c9b05dccf61998d7eeaca6a` — combined-error atomic fixture;
- `f0b256ce47db2220aca63a4bebe5d8feb33158ad` — finalize fault injection preserved;
- `e166fc9bea564c656a94b4a33ec8a0061b4ff3eb` — transient atomic store;
- `5325edcf515a6b0d6ba6890848f924e1397acf2f` — transitional legacy dead-code policy.

## 4. P1 — execution safety и maintainability

### P1.1. External process sandbox

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

- [x] Index production runtime отделён от publication coordinator и tests.
- [x] Legacy index god-file удалён.
- [-] Atomic coordinator активен, legacy inner transition layer ещё существует.
- [ ] Focused daemon process contracts/builders.
- [ ] Cancellation-aware `ProcessRunner` через composition.
- [ ] Удалить остальные legacy global registries.

### P1.4. Installer verification

- [x] Per-binary checksums и fail-closed Linux/Windows installers.
- [x] Tamper smoke и archive/Sigstore/provenance docs.
- [!] Hosted smoke зависит от Actions blocker.
- [ ] Подтвердить version tag.

### P1.5. Operation context E2E

- [x] Cloneable cancellation handle и stable `Cancelled`.
- [x] JSON shape `OperationContext` не изменён.
- [x] Exclusive identity lease и reuse после drop.
- [x] SurrealDB retry bounded deadline/cancellation.
- [x] Daemon write-job propagation.
- [x] Atomic publication использует operation-aware boundary.
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
- [x] Cancellation, fact-query, atomic publication, runtime publication и recovery commands.
- [ ] Common CI failures для остальных workflows.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. `[!]` Включить/проверить GitHub Actions и получить compile/test/fmt/Clippy evidence.
2. P0.4 — убрать canonical put/prepare из deferred pipeline до durable journal.
3. P0.4 — crash-safe snapshot allocation/abort authority без prepared pre-journal state.
4. P0.4 — удалить production dependence от legacy prepared coordinator.
5. P0.4 — immutable generation id и один current pointer.
6. P0.4 — remote conflict evidence и pointer-switch fault injection.
7. P1.5 — read-only daemon/CLI/MCP cancellation.
8. P0.3/P0.1/P1.4 — hosted AppSec, matrices, installers и tag evidence.
9. P1.1, daemon/CLI decomposition и performance budgets.
10. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный blocker:** `GitHub Actions activation/visibility`.

**Текущий безопасный кодовый срез до снятия blocker:** `P0.4 remove deferred canonical prewrite`.

1. Разделить snapshot allocation от записи canonical rows и prepare lifecycle.
2. Deferred pipeline должен вернуть complete canonical batch и cleanup authority без `put_snapshot_with_context`.
3. До durable journal разрешены только allocation и обратимый metadata state.
4. После journal и application staging active coordinator выполняет единственный data+marker mutation.
5. Ошибка до journal удаляет allocated empty snapshot без prepared canonical rows.
6. Crash между deferred extraction и journal не должен оставлять prepared/staged data generation.
7. Обновить pipeline/runtime regressions и удалить compatibility calls из production path.
8. Не считать package hosted-подтверждённым без Actions/remote run.

## 8. Definition of Done проекта

- Public entity/fact/relation/diagnostic queries изолированы committed snapshot.
- Memory, JSONL и SurrealDB проходят общий query/lifecycle/atomic conformance suite.
- Publication crash-safe и atomic на generation boundary.
- Recovery fail-closed и идемпотентна при malformed/missing/mismatched artifacts.
- Embedded ownership fail-closed; remote writers не теряют updates.
- Busy retry bounded attempts/deadline/cancellation.
- Daemon/CLI/MCP имеют эквивалентную cancellation semantics.
- External adapters bounded, cancellable и не оставляют orphan processes.
- Optional feature и AppSec gates блокируют regressions.
- Installers/release assets fail-closed, включая SBOM/provenance.
- CLI/runtime не остаются god modules.
- Roadmap, plan, issues и release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-15 — production atomic coordinator и exact recovery

- `AthanorStore` и transient store получили atomic capability.
- Active production coordinator пишет journal/stages application artifacts, затем публикует complete batch atomic boundary.
- Publish errors и recovery используют exact journal snapshot probe вместо latest-only semantics.
- Exact JSONL commit при failed `latest.json` остаётся non-abortable; recovery сохраняет matching application artifacts.
- Finalize и combined-error fault injections перенесены на atomic path.
- Legacy inner coordinator сохранён как transition/journal compatibility layer.
- Deferred canonical put/prepare до journal остаётся незакрытым crash window и следующим рабочим пакетом.

### 2026-07-15 — backend atomic publication matrix

- Memory реализует complete batch + committed marker одной mutex-секцией.
- JSONL публикует exact generation одним rename и валидирует declared marker.
- Legacy JSONL manifests без marker declaration остаются читаемыми по explicit policy.
- SurrealDB заменяет rows и ставит committed marker одной transaction; statement failure откатывает всё.
- Shared atomic conformance подключён к Memory, JSONL и embedded SurrealDB.
- Remote independent-reader test переведён на atomic publication; hosted evidence отсутствует.

### 2026-07-14 — backend-neutral FactQuery

- Добавлен core-owned `FactQuery` и additive `FactQueryStore`.
- Blanket implementation читает только committed `CanonicalSnapshotStore`.
- Shared conformance проверяет exact/latest и uncommitted isolation.

### 2026-07-14 — recovery content preflight и idempotence

- Current/staging/backup artifacts валидируются по типу, JSON schema и snapshot identity до mutation.
- Committed mismatch, uncommitted replaced-current mismatch и mixed backup generations fail-closed.
- Historical state backups допускают строгую предыдущую versioned schema.
- Committed и uncommitted recovery безопасно повторяются после journal cleanup.

### 2026-07-14 — finalize и recovery fault matrix

- Добавлены deterministic finalize, journal-clear и combined cleanup failures.
- Committed generation завершается typed recovery после transient fault.

### 2026-07-14 — production typed publication cutover

- Production index path переведён на `index_runtime.rs`.
- Legacy index god-file удалён после миграции regressions.

### 2026-07-13 — transaction, CI и security baseline

- Добавлены native batch transaction, atomic counter, embedded ownership, remote test configuration, feature matrix, coverage, AppSec и release integrity workflows.
