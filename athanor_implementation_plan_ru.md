# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки кода: `249613c546c24027cdc8ababe0814efba826511c`  
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
7. Recovery fail-closed проверяет path/type/schema/snapshot/generation identity до mutation.
8. Queries читают только committed canonical snapshots.
9. Data+marker atomic только внутри одного backend boundary.
10. Durable success не превращается в post-commit cancellation error.
11. До durable application journal допустима allocation authority, но не rows/prepared marker.
12. Backend atomic publication ещё не равна одному application generation pointer.
13. `GenerationId` детерминированно выводится из уникального `SnapshotId` и не имеет отдельного allocator/counter.
14. Canonical latest repair не может перемотать visibility на более старый committed snapshot.
15. Destructive index retention требует token от точного dry-run plan.

## 1. Baseline

Реализовано:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default и opt-in embedded/remote SurrealDB;
- shared Memory/JSONL/SurrealDB query/lifecycle/atomic conformance;
- backend-neutral `FactQuery`, `AtomicSnapshotPublication` и `CanonicalLatestPointer`;
- backend atomic data+marker boundaries;
- process-local deferred canonical batch barrier;
- snapshot-native runtime publisher и recovery;
- journal v3 с чтением v2/v1;
- backend-neutral immutable `GenerationId`;
- generation identity в canonical marker/record, read-model manifest, index state и recovery journal;
- один validated `index-current.json` для application artifacts;
- pointer-first index-state readers с legacy fallback только при отсутствии pointer;
- repair inspect/retention/publication recovery/cleanup recovery;
- backend-neutral canonical latest discovery, validation и repair;
- SurrealDB context-owned allocation metadata и bounded orphan cleanup;
- feature, coverage, AppSec, installer и release workflows.

Платформенное состояние:

- [!] GitHub Actions page показывает onboarding вместо runs;
- [!] connector не возвращает push-run/status evidence;
- [!] DNS clone заблокирован в локальной среде;
- [ ] проверить/включить Actions;
- [ ] получить hosted run и исправить реальные findings.

Verification commands:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings

cargo test -p athanor-domain generation --locked
cargo test -p athanor-core latest_pointer --locked
cargo test -p athanor-app index_current --locked
cargo test -p athanor-app index_state --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p athanor-app repair_latest --locked
cargo test -p athanor-app repair_cleanup_recovery --locked
cargo test -p ath --test repair_cli --locked

cargo test -p athanor-store-memory latest_pointer --locked
cargo test -p athanor-store-jsonl latest --locked
cargo test -p athanor-store-jsonl --test atomic_publication --locked
cargo test -p athanor-store-surrealdb latest_pointer --locked
cargo test -p athanor-store-surrealdb --test atomic_publication --locked

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
| Query isolation | `[-]` | committed exact/latest contracts, generation identity | Hosted remote evidence |
| Atomic publication | `[-]` | backend boundaries, application pointer, repair protocol | Artifact checksums |
| Allocation recovery | `[-]` | metadata, 24h cutoff, bounded conditional cleanup, tests | Legacy untagged policy, hosted evidence |
| Recovery safety | `[-]` | journals, pointer recovery, cleanup recovery, idempotence | Artifact checksums, hosted evidence |
| Concurrent writers | `[-]` | JSONL/application locks, embedded ownership | Remote conflict evidence |
| Operation context | `[-]` | cancellation lease, Busy retry, daemon writes | Reads, CLI/MCP, ambiguous in-flight request |
| Runtime maintainability | `[-]` | focused runtime/coordinator, repair command wrapper | Other runtime/daemon decomposition |
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

**Статус:** `[-]` — transactional code path почти завершён; остался один generation-layer code slice и hosted evidence.

#### Shared backend contract

- [x] Query/lifecycle/atomic suites для Memory/JSONL/SurrealDB.
- [x] Uncommitted/prepared невидимы.
- [x] Commit exact/latest visible; abort не меняет latest.
- [x] Backend-neutral `FactQuery`.
- [x] Backend-neutral `CanonicalLatestIdentity` и `CanonicalLatestPointer`.
- [x] Memory и SurrealDB derived-latest validation запрещает rewind.
- [x] JSONL authoritative discovery не доверяет mutable `latest.json`.
- [x] JSONL repair target обязан иметь marker v2 и корректный `GenerationId`.
- [-] Remote reader configured, hosted evidence отсутствует.

#### Atomic coordinator/recovery

- [x] Journal v3/v2/v1 compatibility и atomic persistence.
- [x] Active publisher и runtime используют direct `SnapshotId` API.
- [x] Journal/staging предшествуют final canonical data mutation.
- [x] Explicit complete batch через atomic capability.
- [x] Exact probe перед rollback/abort.
- [x] Exact committed generation не abort’ится.
- [x] Recovery использует plain `abort_snapshot`.
- [x] Recovery path/type/schema/snapshot/generation preflight.
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
- [x] JSONL canonical commit marker writer использует schema v2 с `generation`.
- [x] JSONL reader продолжает принимать legacy marker v1.
- [x] Surreal atomic commit сохраняет generation в snapshot record.
- [x] Read-model manifest и persisted index state содержат snapshot + generation.
- [x] Index state fail-closed отклоняет несовпадающую пару.
- [x] Recovery journal writer v3; v1/v2 читаются и нормализуются.

#### One current pointer, repair и retention — завершено на уровне кода

- [x] Immutable read-model/state generations.
- [x] Один atomic `index-current.json` после готовности обоих artifacts.
- [x] Pointer-first state readers; present invalid pointer не скрывается fallback’ом.
- [x] Durable bridge journal и standalone publication recovery.
- [x] `repair inspect` различает current/recoverable/stale/incomplete/orphaned generations.
- [x] Explicit index retention с exact dry-run confirmation token.
- [x] Pointed и journal-referenced generations не удаляются.
- [x] Cleanup tombstone recovery: finalize, rollback, conflict, idempotence.
- [x] CLI help/JSON/exit-code tests для transactional repair commands.

#### Backend latest-pointer repair — завершено на уровне кода

- [x] Authoritative backend discovery отделён от mutable latest selection.
- [x] Explicit target обязан совпадать с newest committed generation.
- [x] JSONL `latest.json` writer использует schema/snapshot/generation.
- [x] Legacy snapshot-only pointer остаётся query-compatible, но repair-port требует normalization.
- [x] Missing/corrupt/stale pointer ремонтируется только из validated marker v2 generation.
- [x] Memory/SurrealDB validation-only repair сохраняет derived-latest semantics.
- [x] Repeated repair идемпотентен; rewind отклоняется `Conflict`.
- [x] CLI `ath repair repair-latest` поддерживает dry-run, JSON и explicit snapshot.
- [!] Hosted JSONL/embedded/remote evidence отсутствует.

#### Generation layer — остался 1 code slice

- [x] Immutable generation id для canonical/read-model/state/journal.
- [x] Один current pointer после подготовки artifacts.
- [x] Backend latest-pointer repair через generation protocol.
- [x] Pointer switch/cleanup fault injection и recovery.
- [ ] Checksums application artifacts.

## 4. P1 — execution safety и maintainability

### P1.1. External process sandbox

- [ ] OS-level filesystem/network/CPU/memory/process limits.
- [ ] Windows Job Objects; Linux launcher/sandbox.
- [ ] Timeout/cancellation/orphan/limit tests.

### P1.2. CLI decomposition

- [-] Transactional repair команды вынесены в focused entry/parser module.
- [ ] Bootstrap/parse/dispatch only в legacy `main.rs`.
- [ ] Остальные handlers/rendering modules.
- [ ] Полная help/exit-code/JSON matrix.

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

- весь roadmap: ориентировочно `72–76%`;
- код P0.4: ориентировочно `96–98%`;
- до release-grade P0 остаются 1 кодовый generation-layer срез и 5 hosted/platform evidence packages;
- после P0 остаются 17 крупных P1-пунктов и 4 P2-пункта.

## 7. Порядок реализации

1. `[!]` Включить Actions и получить compile/test/fmt/Clippy evidence.
2. P0.4 — checksums application artifacts.
3. P0.4 — remote conflict/latest repair evidence.
4. Hosted AppSec/matrix/installer/tag evidence.
5. P1.5 — read-only daemon/CLI/MCP cancellation.
6. Sandbox, decomposition, performance и P2.

## 8. Текущий рабочий пакет

**Активный blocker:** `GitHub Actions activation/visibility`.

**Завершённый кодовый срез:** `P0.4 backend latest-pointer repair`.

**Следующий безопасный кодовый срез:** `P0.4 checksums application artifacts`.

Целевой результат следующего среза:

- immutable read-model manifest содержит checksums всех опубликованных JSONL файлов;
- immutable index-state pointer/документ содержит checksum state payload;
- `index-current.json` связывает generation с ожидаемыми manifest/state digests;
- readers и repair проверяют digests до открытия selected artifacts;
- checksum mismatch, missing file, path substitution и repeated recovery имеют regressions;
- legacy generation без checksums остаётся readable только в явно ограниченном migration window.

## 9. Журнал актуализаций

### 2026-07-15 — backend latest repair и transactional repair CLI

- Реализованы immutable application generations и один validated `index-current.json`.
- Добавлены bridge journal, standalone publication recovery и cleanup tombstone recovery.
- Добавлен explicit retention protocol с SHA-256 confirmation token.
- CLI получил `index-retention`, `recover-index`, `recover-index-cleanup`, `repair-latest`.
- Добавлен backend-neutral `CanonicalLatestPointer`.
- JSONL latest writer перешёл на generation-bearing schema; legacy pointer query-compatible.
- Authoritative discovery не доверяет mutable pointer и не откатывается к старому generation.
- Memory и SurrealDB сохраняют derived-latest semantics.
- Generation-layer backlog уменьшен до одного code slice: application artifact checksums.
