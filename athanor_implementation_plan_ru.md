# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки кода: `f6da2b7ad3a0b499c652718018abdc189e1e754b`  
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
6. Journal v1/v2 читается после перехода writer на v3.
7. Recovery fail-closed проверяет path/type/schema/snapshot identity до mutation.
8. Queries читают только committed canonical snapshots.
9. Data+marker atomic только внутри одного backend boundary.
10. Durable success не превращается в post-commit cancellation error.
11. До durable application journal допустима allocation authority, но не rows/prepared marker.
12. Backend atomic publication ещё не равна одному generation pointer.
13. `GenerationId` детерминированно выводится из уникального `SnapshotId` и не имеет отдельного allocator/counter.

## 1. Baseline

Реализовано:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default и opt-in embedded/remote SurrealDB;
- shared Memory/JSONL/SurrealDB query/lifecycle/atomic conformance;
- backend-neutral `FactQuery` и `AtomicSnapshotPublication`;
- backend atomic data+marker boundaries;
- process-local deferred canonical batch barrier;
- snapshot-native runtime publisher и recovery;
- journal v3 с чтением v2/v1;
- backend-neutral immutable `GenerationId`;
- generation identity в JSONL canonical marker, Surreal snapshot record, read-model manifest, index state и recovery journal;
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

cargo test -p athanor-domain generation --locked
cargo test -p athanor-app index_publication_journal --locked
cargo test -p athanor-app read_model --locked
cargo test -p athanor-app index_state --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-store-jsonl --test atomic_publication --locked
cargo test -p athanor-store-surrealdb --test atomic_publication --locked

cargo test -p athanor-app --test prepared_publication --locked
cargo test -p athanor-app --test deferred_canonical_buffer --locked
cargo test -p athanor-app index_publication_fault_tests --locked
cargo test -p athanor-app index_publication_finalize_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p athanor-app index_publication_content_tests --locked
cargo test -p athanor-app index_publication_combined_error_tests --locked
cargo test -p athanor-app index_runtime_tests --locked

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
| Atomic publication | `[-]` | backend boundaries, data barrier, immutable generation identity | One current pointer и repair protocol |
| Allocation recovery | `[-]` | metadata, 24h cutoff, bounded conditional cleanup, tests | Legacy untagged policy, hosted evidence |
| Recovery safety | `[-]` | journal/preflight/exact probe/idempotence, plain abort, generation-bearing journal | Checksums, one pointer |
| Concurrent writers | `[-]` | JSONL lock, counter, embedded ownership | Remote conflict evidence |
| Operation context | `[-]` | cancellation lease, Busy retry, daemon writes | Reads, CLI/MCP, ambiguous in-flight request |
| Runtime maintainability | `[-]` | focused runtime/coordinator, legacy coordinator removed | Other runtime/daemon decomposition |
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

**Статус:** `[-]` — backend/data/allocation boundaries, snapshot-native runtime/recovery и immutable generation identity реализованы; осталось 4 generation-layer среза.

#### Shared backend contract

- [x] Query/lifecycle/atomic suites для Memory/JSONL/SurrealDB.
- [x] Uncommitted/prepared невидимы.
- [x] Commit exact/latest visible; abort не меняет latest.
- [x] Backend-neutral `FactQuery`.
- [-] Remote reader configured, hosted evidence отсутствует.

#### Atomic coordinator/recovery

- [x] Journal v3/v2/v1 compatibility и atomic persistence.
- [x] Active publisher и runtime используют direct `SnapshotId` API.
- [x] Journal/staging предшествуют final canonical data mutation.
- [x] Explicit complete batch через atomic capability.
- [x] Exact probe перед rollback/abort.
- [x] Exact committed generation не abort’ится.
- [x] Recovery использует plain `abort_snapshot`.
- [x] Recovery path/type/schema/snapshot preflight.
- [x] Pointer/finalize/clear/combined-error regressions.
- [x] Repeated recovery идемпотентен.
- [x] Compatibility publisher и legacy coordinator modules удалены.
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

#### Immutable generation identity — завершено

- [x] Domain newtype `GenerationId` не зависит от backend.
- [x] Identity детерминированно выводится как `gen_<SnapshotId>`.
- [x] JSONL canonical commit marker writer перешёл на schema v2 с `generation`.
- [x] JSONL reader продолжает принимать legacy marker v1.
- [x] Surreal atomic commit сохраняет generation в snapshot record.
- [x] Read-model `manifest.json` содержит snapshot + generation.
- [x] Persisted index state wire содержит snapshot + generation и читает legacy state без generation.
- [x] Index state fail-closed отклоняет несовпадающую пару snapshot/generation.
- [x] Recovery journal writer перешёл на v3; v1/v2 читаются и нормализуются.
- [x] Pointer-failure recovery regression проверяет сохранение одного generation во journal/read-model/state.
- [!] Hosted compile/test/fmt/Clippy evidence отсутствует.

#### Generation layer — осталось 4 code slices

- [x] Immutable generation id для canonical/read-model/state/journal.
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
- [x] Publication coordinator focused, transition modules удалены.
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

## 6. Оценка готовности

Оценка остаётся диапазоном, а не release assertion:

- весь roadmap до текущего среза: `67–72%`;
- после завершения immutable generation identity: ориентировочно `68–73%`;
- код P0.4 до текущего среза: `88–90%`;
- после завершения immutable generation identity: ориентировочно `91–93%`;
- до release-grade P0 остаются 4 кодовых generation-layer среза и 5 hosted/platform evidence packages;
- после P0 остаются 17 крупных P1-пунктов и 4 P2-пункта.

## 7. Порядок реализации

1. `[!]` Включить Actions и получить compile/test/fmt/Clippy evidence.
2. P0.4 — один current pointer.
3. P0.4 — backend latest-pointer repair.
4. P0.4 — pointer switch/cleanup fault injection.
5. P0.4 — checksums application artifacts.
6. P0.4 — remote conflict evidence.
7. P1.5 — read-only daemon/CLI/MCP cancellation.
8. Hosted AppSec/matrix/installer/tag evidence.
9. Sandbox, decomposition, performance и P2.

## 8. Текущий рабочий пакет

**Активный blocker:** `GitHub Actions activation/visibility`.

**Завершённый кодовый срез:** `P0.4 immutable generation identity`.

**Следующий безопасный кодовый срез:** `P0.4 one current pointer`.

Целевой результат следующего среза:

- canonical, read-model и state сначала подготавливаются под immutable `GenerationId`;
- один application-level pointer атомарно выбирает generation только после готовности всех artifacts;
- readers разрешают latest через этот pointer, а не через независимые mutable locations;
- legacy locations остаются readable на migration window;
- rollback до pointer switch удаляет только непубличную generation;
- recovery после pointer switch завершает cleanup, не откатывая durable success.

## 9. Журнал актуализаций

### 2026-07-15 — immutable generation identity

- Добавлен backend-neutral domain type `GenerationId`.
- Выбрано deterministic mapping `gen_<SnapshotId>` без нового allocator/counter.
- JSONL canonical marker переведён на v2; v1 остаётся readable.
- Surreal atomic commit сохраняет generation в snapshot record.
- Read-model manifest и index-state wire сохраняют generation.
- Index state отклоняет mismatched snapshot/generation и читает legacy state без generation.
- Recovery journal переведён на v3 с чтением v2/v1.
- Добавлены marker, state, manifest, journal и pointer-failure recovery regressions.
- Generation-layer backlog уменьшен с пяти до четырёх code slices.
