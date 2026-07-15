# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `66bcc6c8642234c6dc7bc05d13f5ba93bdc5d56c`  
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
3. Hosted/platform пункт не выполнен без run/status evidence.
4. Coverage threshold не назначается без измерения.
5. Cancellation/deadline не блокируют rollback/recovery.
6. Journal v1 читается после перехода writer на v2.
7. Recovery fail-closed проверяет path/type/schema/snapshot identity до mutation.
8. Queries читают только committed canonical snapshots.
9. Data+marker atomic только внутри одного backend boundary.
10. Durable success не превращается в post-commit cancellation error.
11. До durable application journal допустима allocation authority, но не rows/prepared marker.
12. Backend atomic publication ещё не равна одному generation pointer.

## 1. Baseline

Реализовано:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default и opt-in embedded/remote SurrealDB;
- shared Memory/JSONL/SurrealDB query/lifecycle/atomic conformance;
- core-owned `FactQuery`, `AtomicSnapshotPublication`, compatibility `PreparedSnapshot`;
- backend atomic data+marker boundaries;
- process-local deferred canonical batch barrier;
- active atomic coordinator и exact canonical recovery;
- journal v2/v1 compatibility и recovery fault matrix;
- SurrealDB context-owned allocation metadata и bounded orphan cleanup;
- active snapshot-native journal writer и `SnapshotId` coordinator API с прежним v1/v2 wire format;
- feature, coverage, AppSec, installer и release workflows.

Платформенное состояние:

- [!] GitHub Actions page показывает onboarding вместо runs;
- [!] connector не возвращает push-run/status evidence;
- [!] локальная среда без Rust toolchain, DNS clone заблокирован;
- [ ] проверить/включить Actions;
- [ ] получить hosted run и исправить реальные findings.

Verification commands:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings

cargo test -p athanor-app --test prepared_publication --locked
cargo test -p athanor-app --test deferred_canonical_buffer --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app index_publication_journal --locked
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
| Query isolation | `[-]` | committed exact/latest contracts | Hosted remote evidence, one generation view |
| Atomic publication | `[-]` | backend boundaries, data barrier, snapshot-native publisher, exact recovery | Runtime/recovery transition, one pointer |
| Allocation recovery | `[-]` | metadata, 24h cutoff, bounded conditional cleanup, tests | Legacy untagged policy, hosted evidence |
| Recovery safety | `[-]` | journal/preflight/exact probe/idempotence | Checksums, one pointer |
| Concurrent writers | `[-]` | JSONL lock, counter, embedded ownership | Remote conflict evidence |
| Operation context | `[-]` | cancellation lease, Busy retry, daemon writes | Reads, CLI/MCP, ambiguous in-flight request |
| Runtime maintainability | `[-]` | focused runtime/coordinator | Remove prepared/legacy transition layer |
| Hosted CI | `[!]` | workflow files | Enable Actions, runs, required checks |
| AppSec | `[-]` | configured gates/SBOM | Hosted evidence, push protection |
| Release integrity | `[-]` | fail-closed installers/signing configured | Hosted/tag evidence |
| Default build | `[x]` | remote SurrealDB opt-in | Maintain boundary |

## 3. P0 — release-blocking quality and security

### P0.1. CI feature matrix

- [x] default/no-default, `store-surreal`, `js-ts-precision`, `--all-features`.
- [x] locked check/test/Clippy, SHA-pinned actions, Rust `1.95.0`.
- [!] Включить/подтвердить Actions.
- [ ] Hosted slices и required checks.

### P0.2. Coverage

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV/JSON/HTML artifact.
- [!] Получить hosted artifact.
- [ ] Выбрать floor по измерению.

### P0.3. AppSec/supply chain

- [x] Dependency review, CodeQL, Gitleaks, blocking Zizmor.
- [x] Immutable pins, SBOM, checksum, Sigstore, provenance.
- [x] Release verify блокирует publish.
- [!] Hosted AppSec/tag evidence.
- [ ] Secret push protection.

### P0.4. Store conformance и transactional publication

**Статус:** `[-]` — backend/data/allocation boundaries и snapshot-native publisher реализованы; runtime/recovery transition и один generation pointer открыты.

#### Shared backend contract

- [x] Query/lifecycle/atomic suites для Memory/JSONL/SurrealDB.
- [x] Uncommitted/prepared невидимы.
- [x] Commit exact/latest visible; abort не меняет latest.
- [x] Backend-neutral `FactQuery`.
- [-] Remote reader configured, hosted evidence отсутствует.

#### Atomic coordinator/recovery

- [x] Journal v2/v1 compatibility и atomic persistence.
- [x] Active publisher маршрутизирован через `index_publication_snapshot.rs`.
- [x] Journal/staging предшествуют final canonical data mutation.
- [x] Explicit complete batch через atomic capability.
- [x] Exact probe перед rollback/abort.
- [x] Exact committed generation не abort’ится.
- [x] Recovery exact-probes journal snapshot.
- [x] Pointer/finalize/clear/combined-error regressions.
- [x] Recovery preflight и repeated recovery.
- [-] Legacy guard/inner coordinator остаются transition layer.
- [!] Hosted compile/test/fmt/Clippy отсутствует.

#### Deferred canonical barrier

- [x] Context batch process-local.
- [x] Pending prepare не создаёт marker.
- [x] Deferred pipeline без exact/prepared JSONL generation.
- [x] Coordinator batch заменяет pending contents.
- [x] Compatibility commit atomic-flush’ит pending batch.
- [x] Abort очищает pending batch/allocation.
- [x] Facade и pipeline regressions.

#### Snapshot allocation recovery

- [x] Core context-allocation/bounded-recovery capability.
- [x] Application trait-object routing.
- [x] JSONL/Memory no-op без durable empty record.
- [x] SurrealDB operation id + timestamp allocation metadata.
- [x] Automatic same-repo sweep до новой context allocation.
- [x] Minimum age 24h; cap 128.
- [x] Destructive delete повторяет repo/age/committed/prepared.
- [x] Prepared/committed защищены.
- [x] Explicit cutoff/limit и embedded regressions.
- [x] Idempotence.
- [x] Regression исправлен под `load_snapshot -> Ok(None)` для удалённой записи.
- [-] Legacy untagged records не удаляются автоматически.
- [!] Hosted embedded/remote evidence отсутствует.

#### Snapshot-native journal/cutover

- [x] Новый production journal хранит `SnapshotId`, не `PreparedSnapshot`.
- [x] v2 wire сохраняет поле `prepared: "<snapshot-id>"`.
- [x] v1 raw `snapshot` читается и нормализуется в v2.
- [x] Unknown fields/schema и неверные paths/id fail-closed.
- [x] Focused v1/v2 serialization tests добавлены.
- [x] Snapshot-native journal writer активен в production publisher path.
- [x] Основной `publish_index_snapshot` принимает `SnapshotId`.
- [x] Journal-write/staging/publish error cleanup использует plain `abort_snapshot`.
- [x] Pointer-failure regression вызывает direct snapshot API и проверяет v2 wire.
- [-] `publish_prepared_index` остаётся compatibility adapter для runtime/fault fixtures.
- [-] Exact recovery временно переиспользует прежний atomic module и typed abort authority.
- [ ] Runtime передаёт `output.snapshot.clone()` и не импортирует `PreparedSnapshotPublication`.
- [ ] Recovery использует snapshot-native journal type и plain `abort_snapshot`.
- [ ] Перенести remaining fault fixtures.
- [ ] Удалить legacy journal/coordinator modules.

#### Следующий generation layer

- [ ] Immutable generation id для canonical/read-model/state.
- [ ] Один current pointer после подготовки artifacts.
- [ ] Backend latest-pointer repair через generation protocol.
- [ ] Pointer switch/cleanup fault injection.
- [ ] Checksums application artifacts.

## 4. P1 — execution safety и maintainability

### P1.1. External process sandbox

- [ ] OS-level filesystem/network/CPU/memory/process limits.
- [ ] Windows Job Objects; Linux launcher/sandbox.
- [ ] Timeout/cancellation/orphan/limit tests.

### P1.2. CLI decomposition

- [ ] Bootstrap/parse/dispatch only в `main.rs`.
- [ ] Handlers/rendering modules.
- [ ] Help/exit-code/JSON tests.

### P1.3. Runtime decomposition

- [x] Index runtime отделён, legacy index god-file удалён.
- [-] Active coordinator focused, transition layer остаётся.
- [ ] Focused daemon contracts и injected cancellable ProcessRunner.
- [ ] Удалить остальные global registries.

### P1.4. Installer verification

- [x] Checksums/fail-closed/tamper/Sigstore/provenance configured.
- [!] Hosted smoke зависит от Actions.
- [ ] Подтвердить version tag.

### P1.5. Operation context E2E

- [x] Cancellation identity lease и stable error contract.
- [x] Busy retry, daemon writes, atomic boundary.
- [ ] Read-only daemon jobs/queries.
- [ ] CLI/MCP cancellation lifecycle.
- [ ] Synchronous projector cancellation semantics.

### P1.6. Performance budgets

- [ ] Aggregate byte/RSS/phase budgets.
- [ ] 10k/50k/100k scenarios.
- [ ] Incremental/full ratio matrix.

## 5. P2 — governance

- [ ] Branch protection/required checks/secret push protection.
- [ ] Issues and acceptance criteria for open P0/P1 packages.
- [ ] Conventional commit lint/changelog/version consistency.
- [ ] `justfile`, `xtask` или unified verification entrypoint.

## 6. Порядок реализации

1. `[!]` Включить Actions и получить compile/test/fmt/Clippy evidence.
2. P0.4 — runtime/recovery snapshot-native cutover.
3. P0.4 — перенести remaining fixtures и удалить transition modules.
4. P0.4 — immutable generation id и один current pointer.
5. P0.4 — remote conflict evidence/pointer fault injection.
6. P1.5 — read-only daemon/CLI/MCP cancellation.
7. Hosted AppSec/matrix/installer/tag evidence.
8. Sandbox, decomposition, performance и P2.

## 7. Текущий рабочий пакет

**Активный blocker:** `GitHub Actions activation/visibility`.

**Текущий безопасный кодовый срез:** `P0.4 active snapshot-native coordinator cutover`.

1. [x] Active publisher использует `index_publication_journal::IndexPublicationJournal`.
2. [x] Основной coordinator API принимает `SnapshotId`.
3. [x] Publication cleanup вызывает plain `abort_snapshot`.
4. [x] Direct pointer-failure test проверяет v2 journal wire и exact commit.
5. [ ] Runtime передаёт `output.snapshot.clone()` без prepared handle.
6. [ ] Recovery переходит с atomic compatibility module на snapshot-native journal/plain abort.
7. [ ] Удалить runtime import `PreparedSnapshotPublication`.
8. [ ] После fixture migration удалить compatibility wrapper/modules.
9. [!] Не считать hosted-подтверждённым без Actions.

## 8. Definition of Done

- Queries изолированы committed snapshot.
- Three-backend shared suites проходят.
- Publication atomic/crash-safe на generation boundary.
- Allocation recovery bounded/fail-closed/idempotent.
- Recovery fail-closed и идемпотентна.
- Busy retry bounded deadline/cancellation.
- Daemon/CLI/MCP cancellation эквивалентна.
- External adapters bounded/cancellable.
- CI/AppSec/install/release gates enforce regressions.
- Runtime/CLI не god modules.
- Plan/issues/release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-15 — allocation recovery и snapshot-native journal staging

- Core capability расширен allocation/recovery boundary.
- SurrealDB tagged allocations очищаются bounded/cutoff/predicate-safe.
- Embedded tests покрывают cutoff/limit/idempotence/protected states.
- Исправлено ожидание удалённой записи на `Ok(None)`.
- Добавлен SnapshotId-backed journal с прежним v1/v2 wire format.
- Production publisher переключён на snapshot-native journal writer.
- Основной coordinator API принимает `SnapshotId`; publication rollback использует plain abort.
- Pointer-failure regression вызывает direct API и проверяет journal wire.
- Runtime/recovery compatibility cutover остаётся следующим срезом.
