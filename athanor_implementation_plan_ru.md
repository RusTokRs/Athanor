# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `9bb8af9bee7a04aba12c32ba5e78c11c8aab497c`  
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
- core-owned `FactQuery`/`FactQueryStore` с subject/object/kind/extractor/limit semantics;
- blanket committed-only fact queries для любого `CanonicalSnapshotStore`;
- core-owned additive `AtomicSnapshotPublication` capability;
- Memory публикует complete batch и committed marker одной mutex-секцией;
- JSONL публикует complete exact generation и `commit.json` одним directory rename;
- JSONL exact/latest reads валидируют manifest и declared marker schema/snapshot identity;
- legacy JSONL manifest без marker declaration остаётся читаемым;
- JSONL pointer-finalization failure сохраняет exact committed generation non-abortable;
- SurrealDB одной SurrealQL transaction заменяет rows и ставит `prepared/committed` marker;
- SurrealDB facade retry’ит всю atomic boundary только для подтверждённого `Busy`;
- duplicate-record regression проверяет rollback rows и marker с последующим успешным retry;
- configured remote independent reader использует atomic publication и public fact query;
- atomic snapshot counter, numeric sequence и prepared immutability;
- embedded SurrealKV single-owner contract и retryable `Busy` mapping;
- bounded Busy retry `10/25/50/100 ms` с deadline/cancellation polling;
- exclusive process-local cancellation identity lease;
- daemon cancellation bridge для index/generate/wiki/html;
- core-owned `PreparedSnapshot`/`PreparedSnapshotPublication`;
- `IndexPublicationJournal` v2 с v1 compatibility, atomic persistence и path validation;
- production index runtime через `index_runtime.rs`;
- typed coordinator с guarded publish, rollback и recovery;
- broad incremental/validation/cancellation tests на production runtime;
- legacy `crates/athanor-app/src/index.rs` удалён;
- deterministic publication fault suite для prepare/publish/finalize/clear/recovery;
- recovery preflight валидирует artifact type, schema и snapshot identity до mutation;
- committed/uncommitted repeated recovery проверена как idempotent после cleanup;
- historical state backup допускает строгую versioned schema, malformed version strings отклоняются;
- combined publish/rollback/abort errors сохраняются в error chain;
- feature, quality, AppSec, coverage, installer и release workflow-файлы.

Платформенное состояние:

- [!] публичная GitHub Actions page не показывает workflow list/runs и отображает onboarding;
- [!] connector не возвращает push-run/status evidence для текущего HEAD;
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

cargo test -p athanor-core cancellation --locked
cargo test -p athanor-core prepared_publication --locked
cargo test -p athanor-app index_runtime_tests --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app index_publication_fault_tests --locked
cargo test -p athanor-app index_publication_finalize_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p athanor-app index_publication_content_tests --locked
cargo test -p athanor-app index_publication_combined_error_tests --locked
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
| Query/snapshot isolation | `[-]` | committed-only entity/fact/relation/diagnostic queries, exact/latest suites | Hosted remote evidence, generation visibility |
| Atomic publication | `[-]` | core capability, Memory/JSONL/Surreal boundaries, shared conformance, recovery matrix | Coordinator cutover, exact canonical recovery, generation pointer |
| Recovery safety | `[-]` | v1/v2 journal, type/content preflight, identity checks, idempotence | Exact canonical probe, cryptographic integrity, generation pointer |
| Concurrent writers | `[-]` | JSONL lock, atomic counter, embedded ownership | Hosted remote conflict evidence |
| Operation context | `[-]` | deadline, cancellation lease, daemon write path, atomic Busy retry | Read commands, CLI/MCP, in-flight SDK outcome semantics |
| Store transaction boundary | `[-]` | atomic data+marker for three backends, core prepared/fact protocols | Production lifecycle cutover and one generation pointer |
| Runtime maintainability | `[-]` | legacy index god-file deleted, focused runtime/coordinator/tests | Further daemon/CLI decomposition |
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

**Статус:** `[-]` — backend atomic data+marker matrix реализована; production index lifecycle ещё использует предварительную запись и prepare/commit compatibility path.

#### Shared backend contract

- [x] `SnapshotBatch → prepare → commit/abort` lifecycle.
- [x] Невидимость uncommitted/prepared snapshot.
- [x] Запрет abort committed snapshot.
- [x] Aborted snapshot не меняет `LatestCommitted`.
- [x] Memory, JSONL и embedded SurrealDB shared query suite.
- [x] Memory, JSONL и embedded SurrealDB shared lifecycle suite.
- [x] Core-owned `FactQuery` и additive `FactQueryStore`.
- [x] Subject/object/kind/extractor/limit filtering имеет одну blanket implementation.
- [x] Exact/latest fact queries включены в shared conformance.
- [-] Remote independent reader использует public `query_facts`, hosted run отсутствует.

#### SurrealDB writer safety

- [x] Process-local write gate для cloned handles.
- [x] Atomic counter и numeric sequence.
- [x] Embedded SurrealKV single-owner regression.
- [x] Confirmed lock/transaction conflicts → retryable `Busy`.
- [x] Data/duplicate/serialization failures → non-retryable adapter error.
- [x] Context-aware bounded retry.
- [-] Remote two-client/32-allocation test настроен, hosted evidence отсутствует.
- [ ] Детерминированно воспроизвести remote write conflict.
- [ ] Определить semantics уже выполняющегося/transport-ambiguous SDK request.

#### Cancellation bridge

- [x] Exclusive cancellation lease на active `operation_id`.
- [x] Duplicate active registration → `Conflict`.
- [x] Same-token/same-id bind идемпотентен.
- [x] Daemon index/generate/wiki/html используют operation-aware scheduler.
- [x] Atomic publication проверяет cancellation/deadline до durable boundary.
- [x] Rollback/recovery cancellation-independent.
- [ ] Read-only daemon commands перевести на cancellable lifecycle.
- [ ] Связать CLI и MCP cancellation с core handle.

#### Typed index journal и production coordinator

- [x] Journal v2 хранит serialized `PreparedSnapshot`.
- [x] Journal v1 читается и нормализуется в v2.
- [x] Unknown schema/fields fail-closed.
- [x] Atomic write/load/clear.
- [x] Journal paths и publication id валидируются.
- [x] `publish_prepared_index`: journal → read model/state prepare → typed publish → finalize.
- [x] `recover_interrupted_publication`: committed cleanup или uncommitted rollback/abort.
- [x] Public `index` module переключён на `index_runtime.rs`.
- [x] Runtime regressions сверяют canonical/read-model/index-state snapshot identity.
- [x] Legacy `crates/athanor-app/src/index.rs` удалён.
- [x] Prepare/publish/finalize/clear и combined cleanup failures покрыты.
- [x] Recovery валидирует type/schema/snapshot identity до mutation.
- [x] Read-model/state backups обязаны представлять одну предыдущую generation.
- [x] Historical state backup принимает строгую versioned `athanor.index_state.vN` schema.
- [x] Recovery без backups удаляет uncommitted current artifacts и prepared snapshot.
- [x] Committed и uncommitted repeated recovery после cleanup идемпотентны.
- [!] Hosted compile/test/fmt/Clippy evidence отсутствует из-за Actions blocker.

#### Canonical data + commit marker

- [-] Backend protocol выполнен, production coordinator cutover не завершён.
- [x] Core-owned additive `AtomicSnapshotPublication` без ложного compatibility fallback.
- [x] Context-aware default проверяет cancellation/deadline только до durable publish.
- [x] Memory одной mutex-секцией заменяет partial staging complete batch и ставит committed marker.
- [x] JSONL пишет complete generation, manifest declaration и `commit.json` в hidden staging.
- [x] JSONL одним directory rename публикует exact committed generation.
- [x] JSONL exact/latest reads fail-closed валидируют manifest и required marker.
- [x] JSONL malformed/missing/foreign marker regressions добавлены.
- [x] Legacy JSONL manifest без marker declaration остаётся читаемым.
- [x] JSONL pointer-finalization regression подтверждает non-abortable exact generation после ошибки.
- [x] SurrealDB одной transaction удаляет старые rows, вставляет complete batch и ставит committed marker.
- [x] SurrealDB facade retry’ит entire boundary только для классифицированного `Busy`.
- [x] SurrealDB duplicate failure откатывает rows/marker и допускает valid retry.
- [x] Общий atomic publication conformance подключён к Memory, JSONL и embedded SurrealDB.
- [-] Remote independent-reader atomic regression настроен, hosted run отсутствует.
- [ ] Production coordinator: передавать complete `SnapshotBatch` в atomic capability.
- [ ] Deferred pipeline: перестать записывать canonical rows до durable publication journal.
- [ ] Recovery: определять commit journal snapshot через exact canonical probe, не только `LatestCommitted`.
- [ ] Fault injection: committed exact generation + failed latest pointer + application finalize/recovery.

#### Следующий transactional layer

- [ ] Ввести immutable generation id для canonical/state/read models.
- [ ] Переключать один current pointer после подготовки всех artifacts.
- [ ] Расширить fault injection на pointer switch и generation cleanup.
- [ ] Добавить cryptographic/content checksum для application artifacts.

Последние implementation-коммиты:

- `a15ae020f336a5146f2f32dea4db9fb32eb3f743` — core `AtomicSnapshotPublication` capability;
- `e23a425031efd1046e09fb7472ced7f6221ccff2` / `37c1078af83b3557d8370327da674979764e6a86` — Memory implementation/regression;
- `761189751b6aef6b2bf67030caf1911969326dcb` — JSONL atomic exact-generation publisher;
- `2831028be6d3345ef6a3eceda6c24322a6bc0ff0` — atomic manifest marker declaration;
- `ccd20127d77c59719f3bafc040552efd584be058` — JSONL validating public wrapper;
- `1fc5290e061ddb8dd13a79bba64b99cdd8514555` — JSONL marker/legacy regressions;
- `ad9b662998f79006d3d3aef2b109e0a847bb4326` — SurrealDB rows+marker transaction;
- `4de437bf0120ea156197e52e0d50308fecc1a48f` — SurrealDB facade atomic Busy retry;
- `774e89c36fc398a1ec97737c3bc054a4c9a23cac` — embedded SurrealDB atomic regressions;
- `232c697e459fff38f5d47b23a76772258d141c6f` — remote atomic independent-reader test;
- `753aa0bf1412f030a54723b4bc2bb797abe54d43` — shared atomic publication contract;
- `79ba98fc5e7cba22568e894cae25e284405a217b` / `28010537225098dedcbade03c9f6d09e8b52d60e` / `919f86ab3865231ee46f0f987eb32c914706288e` — three-backend conformance registration.

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
2. P0.4 — production coordinator cutover на complete `SnapshotBatch` + atomic boundary.
3. P0.4 — exact canonical commit probe в recovery и pointer-failure fault suite.
4. P0.4 — immutable generation id и один current pointer.
5. P0.4 — remote conflict evidence и pointer-switch fault injection.
6. P1.5 — read-only daemon/CLI/MCP cancellation.
7. P0.3/P0.1/P1.4 — hosted AppSec, matrices, installers и tag evidence.
8. P1.1, daemon/CLI decomposition и performance budgets.
9. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный blocker:** `GitHub Actions activation/visibility`.

**Текущий безопасный кодовый срез до снятия blocker:** `P0.4 production atomic coordinator cutover`.

1. Сохранить complete `SnapshotBatch` в deferred index result до publication boundary.
2. Не вызывать `put_snapshot_with_context` до durable index publication journal.
3. После staging read-model/state вызвать `publish_snapshot_batch_with_context`.
4. Не abort’ить exact generation, если atomic publish завершился, а отдельный latest pointer/finalize шаг вернул ошибку.
5. Recovery должен проверять journal snapshot через exact canonical load/marker.
6. Добавить fault regression: exact canonical commit успешен, latest/finalize fault, recovery завершает application artifacts.
7. После cutover удалить production dependence от предварительного `prepare_publication` там, где cleanup authority больше не нужна.
8. Не считать protocol hosted-подтверждённым без Actions/remote run.

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

### 2026-07-15 — backend atomic publication matrix

- Добавлен core-owned `AtomicSnapshotPublication` без compatibility fallback.
- Memory реализует complete batch + committed marker одной mutex-секцией.
- JSONL публикует exact generation одним rename и валидирует declared marker при exact/latest reads.
- Legacy JSONL manifests без marker declaration остаются читаемыми по explicit compatibility policy.
- SurrealDB заменяет rows и ставит committed marker одной SurrealQL transaction; statement failure откатывает всё.
- Public SurrealDB facade retry’ит whole boundary только для классифицированного `Busy`.
- Shared atomic conformance подключён к Memory, JSONL и embedded SurrealDB.
- Remote independent-reader test переведён на atomic publication; hosted evidence отсутствует.
- Production coordinator и exact canonical recovery остаются следующим рабочим пакетом.

### 2026-07-14 — backend-neutral FactQuery

- Добавлен core-owned `FactQuery` и additive `FactQueryStore`.
- Blanket implementation читает только committed `CanonicalSnapshotStore` и применяет единые filters/limit.
- Shared conformance и отдельные backend tests проверяют exact/latest и uncommitted isolation.
- Remote independent reader настроен на публичный `query_facts`; hosted evidence отсутствует.

### 2026-07-14 — recovery content preflight и idempotence

- Current/staging/backup artifacts валидируются по типу, JSON schema и snapshot identity до mutation.
- Committed mismatch, uncommitted replaced-current mismatch и mixed backup generations fail-closed.
- Historical state backups допускают строгую предыдущую versioned schema для recovery после upgrade.
- Committed и uncommitted recovery безопасно повторяются после journal cleanup.

### 2026-07-14 — finalize и recovery fault matrix

- Добавлены deterministic read-model finalize, index-state finalize и journal-clear failures.
- Committed generation остаётся выбранной и завершается typed recovery после снятия transient fault.
- Missing backups удаляют только uncommitted current generation и prepared canonical snapshot.
- Combined canonical publish, application rollback и abort failures сохраняются в одной error chain.

### 2026-07-14 — production typed publication cutover

- Production index path переведён на `index_runtime.rs`.
- Legacy `index.rs` удалён после миграции broad regressions.
- Typed journal v2, guarded coordinator и backend publication matrix активны в runtime.

### 2026-07-13 — transaction, CI и security baseline

- Добавлены native batch transaction, atomic counter, embedded ownership, remote test configuration, feature matrix, coverage, AppSec и release integrity workflows.
