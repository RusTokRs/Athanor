# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `a55729512b85af5ffa6a735216992f2a0a469c62`  
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
3. Hosted/platform пункт не считается выполненным без run/status evidence.
4. Coverage threshold не назначается без фактического измерения.
5. Cancellation/deadline не должны блокировать rollback или recovery.
6. Journal v1 обязан читаться после перехода writer на v2.
7. Recovery не может изменять paths вне ожидаемых `.athanor` artifacts.
8. Recovery fail-closed проверяет type/schema/snapshot identity до destructive mutation.
9. Historical index-state backup может использовать валидную предыдущую `athanor.index_state.vN` schema.
10. Backend-neutral facts читаются только из committed canonical snapshot.
11. Data и committed marker считаются atomic только при одном backend-specific boundary.
12. Successful durable publish нельзя превращать в cancellation error повторной post-commit проверкой.
13. Backend atomic data+marker ещё не означает один generation pointer для canonical/read-model/state.
14. Store-level typed publication contracts принадлежат `athanor-core`.

## 1. Текущий baseline

Реализовано:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in embedded/remote SurrealDB;
- shared query/lifecycle/atomic conformance для Memory, JSONL и embedded SurrealDB;
- core-owned `FactQuery`/`FactQueryStore` и committed-only blanket implementation;
- core-owned additive `AtomicSnapshotPublication` capability;
- Memory: complete batch + committed marker одной mutex-секцией;
- JSONL: hidden staging, declared `commit.json`, exact directory rename и fail-closed exact/latest validation;
- legacy JSONL manifests без marker declaration читаются по explicit compatibility policy;
- JSONL latest-pointer failure оставляет exact committed generation readable/non-abortable;
- SurrealDB: complete replacement rows + `prepared/committed` marker одной SurrealQL transaction;
- SurrealDB facade retry’ит entire atomic boundary только для classified `Busy`;
- duplicate-record regression проверяет transaction rollback и valid retry;
- configured remote reader использует atomic publication и public fact query;
- `AthanorStore` передаёт atomic capability через production trait object;
- active production coordinator после journal/staging собирает complete `SnapshotBatch` и вызывает atomic boundary;
- publish-error и recovery exact-probe’ят journal snapshot вместо latest-only semantics;
- exact JSONL commit + failed `latest.json` regression сохраняет journal/application artifacts и запрещает abort;
- finalize и combined-error fault hooks перенесены на atomic boundary;
- transient application store поддерживает atomic replacement;
- atomic snapshot counter, numeric sequence, embedded ownership и bounded Busy retry;
- operation cancellation lease и daemon bridge для index/generate/wiki/html;
- `PreparedSnapshot`/`PreparedSnapshotPublication` остаются compatibility cleanup authority;
- journal v2/v1 compatibility, atomic persistence и path validation;
- focused production index runtime и publication/recovery fault matrix;
- recovery content/type/schema/identity preflight и repeated-recovery idempotence;
- feature, quality, AppSec, coverage, installer и release workflow-файлы.

Платформенное состояние:

- [!] публичная GitHub Actions page показывает onboarding вместо runs;
- [!] connector не возвращает push-run/status evidence для текущего HEAD;
- [!] локальная среда не содержит Rust toolchain, DNS-доступ к GitHub clone заблокирован;
- [ ] проверить/включить Actions в repository или organization settings;
- [ ] получить первый hosted run и исправить фактические findings.

Verification commands:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings

cargo test -p athanor-core fact_query --locked
cargo test -p athanor-app fact_query --locked
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
| Query/snapshot isolation | `[-]` | committed entity/fact/relation/diagnostic exact/latest contracts | Hosted remote evidence, generation visibility |
| Atomic publication | `[-]` | three backend boundaries, shared conformance, active atomic coordinator, exact recovery | Remove pre-journal canonical mutation, one generation pointer |
| Recovery safety | `[-]` | v1/v2 journal, content/type preflight, exact canonical probe, idempotence | Cryptographic integrity, one pointer |
| Concurrent writers | `[-]` | JSONL lock, atomic counter, embedded ownership | Hosted remote conflict evidence |
| Operation context | `[-]` | deadline/cancellation lease, atomic Busy retry, daemon write path | Reads, CLI/MCP, ambiguous in-flight SDK outcome |
| Store transaction boundary | `[-]` | atomic data+marker for three backends and production final boundary | Eliminate deferred prewrite, one generation pointer |
| Runtime maintainability | `[-]` | focused runtime, active atomic coordinator, isolated compatibility modules | Remove transition layer, further daemon/CLI decomposition |
| Hosted CI governance | `[!]` | workflow-файлы существуют | Enable Actions, runs, branch protection, required checks |
| AppSec | `[-]` | configured CodeQL/dependency/Gitleaks/Zizmor/SBOM | Hosted evidence, push protection |
| Release integrity | `[-]` | fail-closed installers, signed assets/SBOM configured | Hosted/tag evidence |
| Default build | `[x]` | remote SurrealDB opt-in | Maintain boundary |

## 3. P0 — release-blocking quality and security

### P0.1. CI feature matrix

**Статус:** `[!]` — implementation готов, hosted runs не видны.

- [x] default/no-default, `store-surreal`, `js-ts-precision`, `--all-features`.
- [x] Check/test/Clippy с `--locked`.
- [x] Remote server tests изолированы от обычного `--all-features`.
- [x] SHA-pinned actions и Rust `1.95.0`.
- [!] Проверить/включить GitHub Actions.
- [ ] Подтвердить hosted run всех slices.
- [ ] Сделать checks required после branch protection.

### P0.2. Coverage measurement

**Статус:** `[!]` — job настроен, artifact заблокирован Actions.

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV, JSON summary и HTML artifact.
- [!] Получить первый hosted artifact.
- [ ] По данным выбрать общий floor или targeted gate.

### P0.3. AppSec и supply chain

**Статус:** `[!]` — gates настроены, hosted enforcement отсутствует.

- [x] Dependency review `moderate`, CodeQL Rust security-extended, Gitleaks, blocking Zizmor.
- [x] Immutable action pins.
- [x] CycloneDX SBOM, checksum, Sigstore и provenance.
- [x] Release verify блокирует publish.
- [!] Подтвердить hosted AppSec/tag release.
- [ ] Подтвердить repository secret push protection.

### P0.4. Store conformance и transactional publication

**Статус:** `[-]` — backend matrix и active final coordinator atomic; deferred pipeline всё ещё пишет/prepare’ит canonical snapshot до durable journal.

#### Shared backend contract

- [x] Query, prepare/commit/abort и atomic publication suites для Memory, JSONL, embedded SurrealDB.
- [x] Uncommitted/prepared snapshots невидимы.
- [x] Commit делает generation exact/latest visible; abort не меняет `LatestCommitted`.
- [x] Core-owned FactQuery filters/limit имеют одну blanket implementation.
- [-] Remote independent reader configured, hosted evidence отсутствует.

#### SurrealDB writer safety

- [x] Process-local write gate, atomic counter, numeric sequence, embedded single-owner contract.
- [x] Confirmed conflict → retryable `Busy`; data/serialization errors non-retryable.
- [x] Context-aware bounded retry whole atomic boundary.
- [-] Remote atomic reader/32 allocations configured, hosted evidence отсутствует.
- [ ] Детерминированный remote write conflict.
- [ ] Semantics transport-ambiguous in-flight request.

#### Cancellation bridge

- [x] Exclusive cancellation identity lease, conflict/reuse/idempotent bind semantics.
- [x] Daemon index/generate/wiki/html используют operation-aware scheduler.
- [x] Atomic publish проверяет cancellation/deadline до boundary, не после durable success.
- [x] Rollback/recovery cancellation-independent.
- [ ] Read-only daemon commands.
- [ ] CLI/MCP cancellation lifecycle.

#### Typed journal и production atomic coordinator

- [x] Journal v2/v1 compatibility, unknown-field/path/id fail-closed, atomic write/load/clear.
- [x] Active production alias: `index_publication_atomic.rs`.
- [x] Coordinator после journal и application staging собирает complete `SnapshotBatch`.
- [x] Final canonical boundary вызывает `publish_snapshot_batch_with_context`.
- [x] Publish error exact-probes journal snapshot перед rollback/abort.
- [x] Exact committed generation сохраняет journal/application artifacts и не abort’ится.
- [x] Absent/uncommitted exact generation откатывает application artifacts и abort’ится.
- [x] Recovery использует exact canonical load/marker, не только `LatestCommitted`.
- [x] Exact commit + failed JSONL latest pointer + recovery regression.
- [x] Finalize sabotage и combined-error fixtures работают через atomic boundary.
- [x] Recovery preflight проверяет type/schema/snapshot identity и matching previous backups.
- [x] Repeated committed/uncommitted recovery после cleanup — no-op.
- [-] Legacy guard/inner coordinator остаются transition/journal compatibility layer.
- [!] Hosted compile/test/fmt/Clippy evidence отсутствует.

#### Canonical data + commit marker

- [x] Core additive capability без non-atomic fallback.
- [x] Memory atomic replacement.
- [x] JSONL hidden staging + marker + exact rename + validation + legacy policy.
- [x] JSONL pointer failure сохраняет exact generation non-abortable.
- [x] SurrealDB rows+marker одной transaction; statement failure откатывает всё.
- [x] Three-backend shared atomic conformance.
- [-] Remote atomic regression configured, hosted run отсутствует.
- [x] Production final coordinator передаёт complete batch в atomic capability.
- [x] Recovery exact-probes canonical commit.
- [x] Pointer-failure application recovery fault regression.
- [ ] Deferred pipeline не должен вызывать `put_snapshot_with_context` до journal.
- [ ] Deferred pipeline не должен вызывать `prepare_publication` до journal.
- [ ] Snapshot allocation/cleanup authority должны иметь crash-safe pre-journal semantics.
- [ ] После удаления prewrite убрать production dependence от prepared compatibility handle.

#### Следующий transactional layer

- [ ] Immutable generation id для canonical/read-model/state.
- [ ] Один current pointer после подготовки всех artifacts.
- [ ] Backend latest pointer recovery через generation protocol.
- [ ] Pointer-switch/generation-cleanup fault injection.
- [ ] Cryptographic/content checksums application artifacts.

Последние implementation-коммиты:

- `a15ae020f336a5146f2f32dea4db9fb32eb3f743` — core atomic capability;
- `ccd20127d77c59719f3bafc040552efd584be058` / `1fc5290e061ddb8dd13a79bba64b99cdd8514555` — JSONL marker validation/regressions;
- `ad9b662998f79006d3d3aef2b109e0a847bb4326` / `4de437bf0120ea156197e52e0d50308fecc1a48f` — SurrealDB transaction/facade;
- `753aa0bf1412f030a54723b4bc2bb797abe54d43` — shared atomic conformance;
- `8676713cb01cd637dca4796332fdd7d0a67f1b0e` — application atomic trait-object boundary;
- `75e7b108bc9b9591b0cb980bdb67c95c1841631f` — active atomic coordinator и exact recovery;
- `68a867eedb06726452af39fe3962ed718ed6e77f` — production alias cutover;
- `d4d8d1e7caeefb02f272edf192e4b1bff66d7cff` / `22d87a9cf6d97f7d2ae7bf33ee132e95c34dd72a` — pointer-failure regression/registration;
- `a7a4058dd6b57b237c9b05dccf61998d7eeaca6a` — combined-error atomic fixture;
- `f0b256ce47db2220aca63a4bebe5d8feb33158ad` — finalize fault injection preserved;
- `e166fc9bea564c656a94b4a33ec8a0061b4ff3eb` — transient atomic store;
- `5325edcf515a6b0d6ba6890848f924e1397acf2f` — transition dead-code policy.

## 4. P1 — execution safety и maintainability

### P1.1. External process sandbox

- [ ] Production clean environment и allowlist.
- [ ] Windows Job Objects; Linux launcher/sandbox.
- [ ] Network/filesystem/CPU/memory/process limits.
- [ ] Timeout/cancellation/orphan/limit tests.

### P1.2. CLI decomposition

- [ ] Оставить в `main.rs` bootstrap/parse/dispatch/top-level errors.
- [ ] Разнести handlers/rendering.
- [ ] Help/exit-code/JSON contract tests.

### P1.3. Runtime decomposition

- [x] Index runtime отделён от coordinator/tests; legacy index god-file удалён.
- [-] Active atomic coordinator есть, transition inner layer остаётся.
- [ ] Focused daemon contracts/builders и cancellation-aware ProcessRunner.
- [ ] Удалить остальные legacy global registries.

### P1.4. Installer verification

- [x] Checksums, fail-closed installers, tamper smoke, Sigstore/provenance docs.
- [!] Hosted smoke зависит от Actions.
- [ ] Подтвердить version tag.

### P1.5. Operation context E2E

- [x] Cloneable cancellation handle, stable JSON/error contract, exclusive identity lease.
- [x] SurrealDB bounded retry, daemon write propagation, atomic operation boundary.
- [ ] Read-only daemon jobs/queries.
- [ ] CLI/MCP cancellation lifecycle.
- [ ] Cancellation queued/running reads и synchronous projectors.

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
- [ ] Acceptance criteria, verification commands, commit/PR links.

### P2.3. Commit/release policy

- [ ] Conventional commit lint.
- [ ] Automated changelog/release notes.
- [ ] Version/tag/Cargo consistency.

### P2.4. Локальная воспроизводимость

- [x] Feature, coverage, AppSec, release, store, cancellation и recovery commands.
- [ ] Common CI failures для остальных workflows.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. `[!]` Включить/проверить GitHub Actions и получить compile/test/fmt/Clippy evidence.
2. P0.4 — убрать canonical put/prepare из deferred pipeline до durable journal.
3. P0.4 — crash-safe snapshot allocation/abort authority без prepared pre-journal data.
4. P0.4 — удалить production dependence от transition prepared coordinator.
5. P0.4 — immutable generation id и один current pointer.
6. P0.4 — remote conflict evidence и pointer-switch fault injection.
7. P1.5 — read-only daemon/CLI/MCP cancellation.
8. Hosted AppSec/matrices/installers/tag evidence.
9. Sandbox, daemon/CLI decomposition, performance budgets и P2 governance.

## 7. Текущий рабочий пакет

**Активный blocker:** `GitHub Actions activation/visibility`.

**Текущий безопасный кодовый срез:** `P0.4 remove deferred canonical prewrite`.

1. Разделить snapshot allocation от записи canonical rows и prepare lifecycle.
2. Deferred pipeline возвращает complete batch и cleanup authority без `put_snapshot_with_context`.
3. До durable journal разрешены только allocation и полностью обратимое metadata state.
4. После journal/staging coordinator выполняет единственную data+marker mutation.
5. Ошибка до journal удаляет allocated empty snapshot.
6. Crash между extraction и journal не оставляет prepared/staged canonical generation.
7. Обновить pipeline/runtime regressions и удалить compatibility calls из production path.
8. Не считать package hosted-подтверждённым без Actions/remote run.

## 8. Definition of Done проекта

- Public queries изолированы committed snapshot.
- Memory, JSONL и SurrealDB проходят общий query/lifecycle/atomic suite.
- Publication crash-safe и atomic на generation boundary.
- Recovery fail-closed и идемпотентна при malformed/missing/mismatched artifacts.
- Embedded ownership fail-closed; remote writers не теряют updates.
- Busy retry bounded attempts/deadline/cancellation.
- Daemon/CLI/MCP имеют эквивалентную cancellation semantics.
- External adapters bounded, cancellable и не оставляют orphan processes.
- Optional feature/AppSec gates блокируют regressions.
- Installers/release assets fail-closed, включая SBOM/provenance.
- CLI/runtime не остаются god modules.
- Roadmap, plan, issues и release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-15 — production atomic coordinator и exact recovery

- `AthanorStore` и transient store получили atomic capability.
- Active coordinator пишет journal/stages artifacts, затем публикует complete batch atomic boundary.
- Publish errors и recovery используют exact journal snapshot probe.
- Failed JSONL latest pointer не делает exact commit abortable; recovery сохраняет application artifacts.
- Finalize/combined-error injections перенесены на atomic path.
- Deferred canonical put/prepare до journal остаётся следующим crash-safety пакетом.

### 2026-07-15 — backend atomic publication matrix

- Memory, JSONL и SurrealDB реализовали backend-specific data+marker boundaries.
- JSONL валидирует declared marker и сохраняет legacy compatibility.
- SurrealDB statement failure откатывает rows и marker.
- Shared atomic conformance подключён к трём backends; remote evidence отсутствует.

### 2026-07-14 — FactQuery и recovery hardening

- Добавлены backend-neutral fact queries, recovery content/schema/identity preflight и idempotence.
- Finalize, journal-clear и combined cleanup failures покрыты deterministic regressions.

### 2026-07-13 — transaction, CI и security baseline

- Добавлены native batch transaction, atomic counter, ownership, remote configuration, CI, coverage, AppSec и release integrity workflows.
