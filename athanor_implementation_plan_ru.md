# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки кода: `cb5800d9bf57c8d64c2eb83a63812bea1f2602e2`  
> Дата актуализации: 2026-07-15  
> Статус: active implementation plan

## 0. Правила исполнения

Статусы:

- `[x]` — реализация и воспроизводимая проверка добавлены;
- `[-]` — реализация частичная либо отсутствует hosted evidence;
- `[ ]` — не начато;
- `[!]` — внешний blocker.

Правила:

1. Все коммиты идут напрямую в `main`, пока пользователь явно не изменит режим.
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
- backend-neutral `FactQuery` и `AtomicSnapshotPublication`;
- backend atomic data+marker boundaries;
- process-local deferred canonical batch barrier;
- active snapshot-native publisher и recovery entrypoint;
- journal v2/v1 compatibility и recovery fault matrix;
- SurrealDB context-owned allocation metadata и bounded orphan cleanup;
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
```

## 2. Сводный статус

| Область | Статус | Подтверждено | Остаётся |
| --- | --- | --- | --- |
| Query isolation | `[-]` | committed exact/latest contracts | Hosted remote evidence, one generation view |
| Atomic publication | `[-]` | backend boundaries, data barrier, snapshot publisher/recovery entry | Runtime cleanup, legacy internals, one pointer |
| Allocation recovery | `[-]` | metadata, 24h cutoff, bounded conditional cleanup, tests | Legacy untagged policy, hosted evidence |
| Recovery safety | `[-]` | journal/preflight/exact probe/idempotence | Remove compatibility internals, checksums, one pointer |
| Concurrent writers | `[-]` | JSONL lock, counter, embedded ownership | Remote conflict evidence |
| Operation context | `[-]` | cancellation lease, Busy retry, daemon writes | Reads, CLI/MCP, ambiguous in-flight request |
| Runtime maintainability | `[-]` | focused runtime/coordinator, direct fault coverage | Remove prepared/legacy transition layer |
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

**Статус:** `[-]` — backend/data/allocation boundaries и snapshot-native publisher/recovery entry реализованы; осталось 2 transition-cleanup и 5 generation-layer срезов.

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
- [x] Active recovery entrypoint загружает и валидирует snapshot-native journal.
- [x] Pointer/finalize/clear/combined-error regressions.
- [x] Recovery preflight и repeated recovery.
- [-] Artifact cleanup/recovery internals временно делегируются legacy atomic module.
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
- [-] Legacy untagged records не удаляются автоматически.
- [!] Hosted embedded/remote evidence отсутствует.

#### Snapshot-native transition — осталось 2 code slices

- [x] Production journal хранит `SnapshotId`; v2 wire сохраняет `prepared`.
- [x] v1 raw `snapshot` читается и нормализуется в v2.
- [x] Unknown fields/schema и неверные paths/id fail-closed.
- [x] Основной `publish_index_snapshot` принимает `SnapshotId`.
- [x] Publication cleanup использует plain `abort_snapshot`.
- [x] Active recovery entrypoint использует snapshot-native journal type.
- [x] Prepare/cancellation, combined-error и finalize fault suites имеют direct snapshot API coverage.
- [ ] Runtime передаёт `output.snapshot.clone()` и не импортирует `PreparedSnapshotPublication`.
- [ ] Удалить compatibility wrapper, duplicate compatibility tests и legacy journal/coordinator modules.

#### Generation layer — осталось 5 code slices

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
2. P0.4 — runtime direct `SnapshotId` cutover.
3. P0.4 — удалить compatibility/legacy transition layer.
4. P0.4 — 5 generation-layer срезов.
5. P0.4 — remote conflict evidence/pointer fault injection.
6. P1.5 — read-only daemon/CLI/MCP cancellation.
7. Hosted AppSec/matrix/installer/tag evidence.
8. Sandbox, decomposition, performance и P2.

## 7. Текущий рабочий пакет

**Активный blocker:** `GitHub Actions activation/visibility`.

**Текущий безопасный кодовый срез:** `P0.4 runtime direct SnapshotId cutover`.

1. Runtime вызывает `publish_index_snapshot` с `output.snapshot.clone()`.
2. Runtime cleanup exact-probe’ит snapshot и вызывает plain `abort_snapshot`.
3. Удалить runtime imports `PreparedSnapshot`/`PreparedSnapshotPublication`.
4. После runtime cutover удалить compatibility wrapper, duplicate compatibility tests и legacy modules.
5. Не считать hosted-подтверждённым без Actions.

## 8. Сколько осталось

- До завершения текущего transactional P0.4: **7 code slices** — 2 transition-cleanup + 5 generation-layer.
- До release-grade P0: те же 7 code slices плюс **5 hosted/platform evidence packages**, заблокированных Actions/settings.
- После P0 остаются **17 крупных P1 items** и **4 P2 governance items**.
- По самостоятельным рабочим пакетам проект находится примерно на **62–67%** от текущего плана; по коду P0.4 — примерно на **82–85%**, но hosted доказательство пока отсутствует.

## 9. Definition of Done

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

## 10. Журнал актуализаций

### 2026-07-15 — direct snapshot fault migration

- Prepare/read-model/state/cancellation fault fixtures переведены на `publish_index_snapshot`.
- Combined publish/rollback/abort error fixture переведён на direct snapshot API.
- Finalize/read-model/index-state/journal-clear faults получили параллельную direct snapshot suite.
- Compatibility finalize tests остаются только до удаления transition layer.
- Осталось 2 transition-cleanup и 5 generation-layer code slices.
