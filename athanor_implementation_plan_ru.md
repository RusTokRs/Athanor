# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `26a320e28cfd47ddf72707bc111fbda69bf12cd5`  
> Дата актуализации: 2026-07-15  
> Статус: active implementation plan

## 0. Правила исполнения

Статусы:

- `[x]` — реализация и воспроизводимая проверка добавлены;
- `[-]` — реализация частичная либо отсутствует hosted evidence;
- `[ ]` — не начато;
- `[!]` — внешний blocker.

Правила:

1. Коммиты идут только напрямую в `main`, пока пользователь явно не изменит режим.
2. Один коммит — одна логически завершённая часть.
3. Hosted/platform пункт не считается выполненным без run/status evidence.
4. Coverage threshold не назначается без фактического измерения.
5. Cancellation/deadline не блокируют rollback и recovery.
6. Journal v1 читается после перехода writer на v2.
7. Recovery fail-closed проверяет path/type/schema/snapshot identity до destructive mutation.
8. Backend-neutral queries читают только committed canonical snapshots.
9. Data и committed marker atomic только внутри одного backend-specific boundary.
10. Успешный durable publish не превращается в post-commit cancellation error.
11. До durable application journal допустима allocation authority, но не canonical rows/prepared marker.
12. Backend atomic data+marker ещё не равен одному generation pointer для canonical/read-model/state.

## 1. Baseline

Реализовано:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in embedded/remote SurrealDB;
- shared query/lifecycle/atomic conformance для Memory, JSONL и embedded SurrealDB;
- core-owned `FactQuery`, `AtomicSnapshotPublication`, `PreparedSnapshot` compatibility type;
- Memory atomic replacement;
- JSONL hidden staging, declared commit marker, exact rename и fail-closed reads;
- SurrealDB rows+marker одной transaction и bounded Busy retry;
- process-local deferred canonical batch barrier;
- active atomic publication coordinator и exact canonical recovery;
- journal v2 с v1 compatibility;
- recovery type/content/identity preflight и fault matrix;
- SurrealDB context-owned allocation metadata и bounded stale orphan cleanup;
- feature, coverage, AppSec, installer и release workflow-файлы.

Платформенное состояние:

- [!] публичная GitHub Actions page показывает onboarding вместо runs;
- [!] connector не возвращает push-run/status evidence;
- [!] локальная среда не содержит Rust toolchain, DNS clone заблокирован;
- [ ] проверить/включить Actions в repository/organization settings;
- [ ] получить первый hosted run и исправить фактические findings.

Verification commands:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings

cargo test -p athanor-app --test prepared_publication --locked
cargo test -p athanor-app --test deferred_canonical_buffer --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-app index_publication_fault_tests --locked
cargo test -p athanor-app index_publication_finalize_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p athanor-app index_publication_content_tests --locked
cargo test -p athanor-app index_publication_combined_error_tests --locked

cargo test -p athanor-store-memory --test conformance --locked
cargo test -p athanor-store-jsonl --test conformance --locked
cargo test -p athanor-store-surrealdb --test conformance --locked
cargo test -p athanor-store-surrealdb --test allocation_recovery --locked

ATHANOR_SURREAL_REMOTE_URI=ws://127.0.0.1:8000 \
  cargo test -p athanor-store-surrealdb --features remote --test remote --locked -- --ignored

cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 2. Сводный статус

| Область | Статус | Подтверждено | Остаётся |
| --- | --- | --- | --- |
| Query/snapshot isolation | `[-]` | committed exact/latest entity/fact/relation/diagnostic contracts | Hosted remote evidence, generation visibility |
| Atomic publication | `[-]` | three backend boundaries, deferred data barrier, active coordinator, exact recovery | Remove prepared transition layer, one generation pointer |
| Allocation recovery | `[-]` | Surreal metadata, 24h cutoff, bounded conditional cleanup, tests | Legacy untagged policy, hosted evidence |
| Recovery safety | `[-]` | v1/v2 journal, preflight, exact probe, idempotence | Checksums, one pointer |
| Concurrent writers | `[-]` | JSONL lock, atomic counter, embedded ownership | Hosted remote conflict evidence |
| Operation context | `[-]` | cancellation lease, Busy retry, daemon write jobs | Reads, CLI/MCP, ambiguous in-flight request |
| Runtime maintainability | `[-]` | focused runtime/coordinator | Remove prepared/legacy transition modules |
| Hosted CI governance | `[!]` | workflow-файлы существуют | Enable Actions, runs, required checks |
| AppSec | `[-]` | configured CodeQL/dependency/Gitleaks/Zizmor/SBOM | Hosted evidence, push protection |
| Release integrity | `[-]` | fail-closed installers and signed assets configured | Hosted/tag evidence |
| Default build | `[x]` | remote SurrealDB opt-in | Maintain boundary |

## 3. P0 — release-blocking quality and security

### P0.1. CI feature matrix

**Статус:** `[!]` — implementation готов, hosted runs не видны.

- [x] default/no-default, `store-surreal`, `js-ts-precision`, `--all-features`.
- [x] Check/test/Clippy с `--locked`.
- [x] SHA-pinned actions и Rust `1.95.0`.
- [!] Проверить/включить GitHub Actions.
- [ ] Подтвердить hosted slices и required checks.

### P0.2. Coverage measurement

**Статус:** `[!]` — job настроен, artifact отсутствует.

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV, JSON summary и HTML artifact.
- [!] Получить hosted artifact.
- [ ] По данным выбрать floor/targeted gate.

### P0.3. AppSec и supply chain

**Статус:** `[!]` — gates настроены, hosted enforcement отсутствует.

- [x] Dependency review, CodeQL, Gitleaks, blocking Zizmor.
- [x] Immutable action pins.
- [x] CycloneDX SBOM, checksum, Sigstore и provenance.
- [x] Release verify блокирует publish.
- [!] Подтвердить hosted AppSec/tag release.
- [ ] Подтвердить secret push protection.

### P0.4. Store conformance и transactional publication

**Статус:** `[-]` — backend/data/allocation boundaries реализованы; transition handle и единый generation pointer ещё открыты.

#### Shared backend contract

- [x] Query, prepare/commit/abort и atomic publication suites для Memory/JSONL/SurrealDB.
- [x] Uncommitted/prepared snapshots невидимы.
- [x] Commit делает exact/latest visible; abort не меняет latest.
- [x] Backend-neutral `FactQuery` filters/limit.
- [-] Remote independent reader configured, hosted evidence отсутствует.

#### Atomic publication coordinator

- [x] Journal v2/v1 compatibility и atomic persistence.
- [x] Active alias `index_publication_atomic.rs`.
- [x] Coordinator после journal/staging строит complete `SnapshotBatch`.
- [x] Final boundary вызывает `publish_snapshot_batch_with_context`.
- [x] Publish error exact-probes journal snapshot.
- [x] Exact committed generation не abort’ится.
- [x] Uncommitted generation rollback/abort.
- [x] Recovery exact-probes canonical identity.
- [x] Latest-pointer/finalize/clear/combined-error regressions.
- [x] Recovery preflight и repeated recovery.
- [-] Legacy guard/inner coordinator остаются transition layer.
- [!] Hosted compile/test/fmt/Clippy evidence отсутствует.

#### Deferred canonical data barrier

- [x] Context-aware complete batch хранится только process-local.
- [x] Pending prepare не создаёт durable marker.
- [x] Deferred pipeline не создаёт exact/prepared JSONL generation.
- [x] Coordinator explicit batch заменяет pending compatibility contents.
- [x] Compatibility commit делает atomic flush.
- [x] Abort очищает pending batch и backend allocation.
- [x] Recording-store и JSONL pipeline regressions.

#### Snapshot allocation recovery

- [x] Core capability: backend-specific context allocation и bounded orphan recovery.
- [x] `AthanorStore` проводит capability через production trait object.
- [x] JSONL/Memory default recovery — no-op без durable empty allocation.
- [x] SurrealDB allocation atomically сохраняет operation id и Unix timestamp.
- [x] Automatic sweep выполняется перед следующей context allocation того же repo.
- [x] Automatic cutoff: минимум 24 часа.
- [x] Максимум 128 candidates за вызов.
- [x] Delete повторно проверяет repo/age/committed/prepared.
- [x] Prepared и committed records защищены.
- [x] Explicit cutoff/limit API и embedded tests.
- [x] Repeated cleanup idempotent.
- [-] Legacy untagged allocations не удаляются автоматически fail-closed.
- [!] Hosted embedded/remote evidence отсутствует.

#### Следующий transactional layer

- [ ] Удалить production dependence от `PreparedSnapshot` compatibility handle.
- [ ] Удалить legacy guard/inner coordinator после переноса remaining tests/types.
- [ ] Immutable generation id для canonical/read-model/state.
- [ ] Один current pointer после подготовки всех artifacts.
- [ ] Backend latest-pointer repair через generation protocol.
- [ ] Pointer-switch/generation-cleanup fault injection.
- [ ] Checksums application artifacts.

## 4. P1 — execution safety и maintainability

### P1.1. External process sandbox

- [ ] OS-level filesystem/network/CPU/memory/process limits.
- [ ] Windows Job Objects; Linux launcher/sandbox.
- [ ] Timeout/cancellation/orphan/limit tests.

### P1.2. CLI decomposition

- [ ] Оставить в `main.rs` bootstrap/parse/dispatch/top-level errors.
- [ ] Разнести handlers/rendering.
- [ ] Help/exit-code/JSON contract tests.

### P1.3. Runtime decomposition

- [x] Index runtime отделён, legacy index god-file удалён.
- [-] Active focused coordinator есть, transition layer остаётся.
- [ ] Focused daemon contracts/builders и injected cancellable ProcessRunner.
- [ ] Удалить остальные legacy global registries.

### P1.4. Installer verification

- [x] Checksums, fail-closed installers, tamper smoke, Sigstore/provenance docs.
- [!] Hosted smoke зависит от Actions.
- [ ] Подтвердить version tag.

### P1.5. Operation context E2E

- [x] Cancellation handle/identity lease и stable error contract.
- [x] SurrealDB bounded retry, daemon write propagation, atomic boundary.
- [ ] Read-only daemon jobs/queries.
- [ ] CLI/MCP cancellation lifecycle.
- [ ] Queued/running read cancellation и synchronous projector semantics.

### P1.6. Aggregate performance budgets

- [ ] Aggregate byte budget, peak RSS и phase timings.
- [ ] 10k/50k/100k scenarios.
- [ ] Incremental/full ratio matrix.

## 5. P2 — governance и contributor experience

### P2.1. Branch protection

- [ ] После bootstrap gates запретить force push/deletion/direct push.
- [ ] Required quality/feature/store/AppSec/docs checks.
- [ ] Stale-review dismissal и secret push protection.

### P2.2. Traceability

- [ ] Issue на каждый открытый P0/P1 package.
- [ ] Acceptance criteria, commands и commit links.

### P2.3. Commit/release policy

- [ ] Conventional commit lint.
- [ ] Automated changelog/release notes.
- [ ] Version/tag/Cargo consistency.

### P2.4. Локальная воспроизводимость

- [x] Feature/coverage/AppSec/release/store/cancellation/recovery commands.
- [ ] Common CI failures для остальных workflows.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. `[!]` Включить/проверить GitHub Actions и получить compile/test/fmt/Clippy evidence.
2. P0.4 — удалить production dependence от prepared compatibility handle.
3. P0.4 — удалить transition coordinator modules.
4. P0.4 — immutable generation id и один current pointer.
5. P0.4 — remote conflict evidence и pointer-switch fault injection.
6. P1.5 — read-only daemon/CLI/MCP cancellation.
7. Hosted AppSec/matrices/installers/tag evidence.
8. Sandbox, daemon/CLI decomposition, performance budgets и P2 governance.

## 7. Текущий рабочий пакет

**Активный blocker:** `GitHub Actions activation/visibility`.

**Следующий безопасный кодовый срез:** `P0.4 remove production PreparedSnapshot dependency`.

1. Journal writer хранит snapshot identity напрямую, сохраняя v1/v2 read compatibility.
2. Active coordinator принимает `SnapshotId`, а не compatibility prepared handle.
3. Pre-journal failures abort allocation по snapshot identity.
4. Recovery abort использует backend allocation authority, не typed prepare state.
5. Перенести remaining fault fixtures с prepared handle.
6. Удалить production imports/trait bounds `PreparedSnapshotPublication`.
7. Не удалять legacy journal reader до migration tests.
8. Не считать package hosted-подтверждённым без Actions.

## 8. Definition of Done

- Public queries изолированы committed snapshot.
- Memory/JSONL/SurrealDB проходят shared query/lifecycle/atomic suites.
- Publication crash-safe и atomic на generation boundary.
- Allocation recovery bounded/fail-closed/idempotent.
- Recovery fail-closed при malformed/missing/mismatched artifacts.
- Busy retry bounded attempts/deadline/cancellation.
- Daemon/CLI/MCP имеют эквивалентную cancellation semantics.
- External adapters bounded/cancellable и не оставляют orphan processes.
- Optional feature/AppSec gates блокируют regressions.
- Installers/release assets fail-closed с SBOM/provenance.
- CLI/runtime не остаются god modules.
- Plan/issues/release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-15 — allocation recovery

- Core atomic capability расширен context-aware allocation и bounded orphan recovery.
- `AthanorStore` маршрутизирует allocation через backend capability.
- SurrealDB context allocation сохраняет operation id/timestamp atomically.
- Старые uncommitted/unprepared tagged records удаляются по cutoff/limit с повторным predicate check.
- Embedded tests покрывают cutoff, limit, idempotence и защиту prepared/committed.

### 2026-07-15 — deferred canonical barrier

- Context-aware full batch стал process-local до journal.
- Deferred pipeline не создаёт canonical rows/prepared marker.
- Coordinator публикует explicit complete batch atomic boundary.

### 2026-07-15 — backend atomic publication

- Memory, JSONL и SurrealDB реализовали backend-specific data+marker boundaries.
- Active coordinator и recovery используют exact canonical probe.

### 2026-07-14 — recovery hardening

- Добавлены FactQuery, recovery content/schema/identity preflight и idempotence.
- Finalize/clear/combined cleanup failures покрыты deterministic regressions.
