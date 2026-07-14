# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `e96132f2bd1bcca8dc444fb433dcd06c5d709487`  
> Дата актуализации: 2026-07-14  
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
8. Recovery обязана fail-closed до destructive mutation при неверном типе artifact/backup.
9. Backend batch transaction не равна atomic generation publication, пока data, marker и application artifacts переключаются отдельно.
10. Store-level typed publication protocol принадлежит `athanor-core`.

## 1. Текущий baseline

Реализовано:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in embedded/remote SurrealDB;
- shared lifecycle conformance для Memory, JSONL и embedded SurrealDB;
- native SurrealQL transaction для полного `SnapshotBatch`;
- atomic snapshot counter, numeric sequence и prepared immutability;
- embedded SurrealKV single-owner contract и retryable `Busy` mapping;
- bounded Busy retry `10/25/50/100 ms` с deadline/cancellation polling;
- exclusive process-local cancellation identity lease;
- daemon cancellation bridge для index/generate/wiki/html;
- core-owned `PreparedSnapshot`/`PreparedSnapshotPublication`;
- typed lifecycle regressions для Memory, JSONL и embedded SurrealDB;
- `IndexPublicationJournal` v2 с v1 compatibility, atomic persistence и path validation;
- production index runtime через `index_runtime.rs`;
- typed coordinator с guarded publish, rollback и recovery;
- broad incremental/validation/cancellation tests на production runtime;
- legacy `crates/athanor-app/src/index.rs` удалён;
- deterministic publication fault suite для prepare/publish/finalize/clear/recovery;
- recovery preflight отклоняет неверные artifact types до mutation;
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

cargo test -p athanor-core cancellation --locked
cargo test -p athanor-core prepared_publication --locked
cargo test -p athanor-app index_runtime_tests --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app index_publication_fault_tests --locked
cargo test -p athanor-app index_publication_finalize_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
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
| Query/snapshot isolation | `[-]` | committed-only reads, prepare/abort lifecycle, embedded suites | Hosted remote evidence, generation visibility |
| Atomic publication | `[-]` | typed production coordinator, staged recovery, prepare/publish/finalize fault suite | data+marker protocol, generation pointer |
| Recovery safety | `[-]` | v1/v2 journal, type preflight, malformed/missing backup tests, committed recovery | content/schema validation, repeated-recovery idempotence |
| Concurrent writers | `[-]` | JSONL lock, atomic counter, embedded ownership | Hosted remote conflict evidence |
| Operation context | `[-]` | deadline, cancellation lease, daemon write path, typed publish context | Read commands, CLI/MCP, in-flight SDK interruption |
| Store transaction boundary | `[-]` | native batch transaction, core prepared protocol, backend matrix | Fact query contract, full lifecycle transaction |
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

**Статус:** `[-]` — typed production path и crash-recovery fault matrix реализованы; lifecycle ещё не atomic на generation boundary.

#### Shared backend contract

- [x] `SnapshotBatch → prepare → commit/abort` lifecycle.
- [x] Невидимость uncommitted/prepared snapshot.
- [x] Запрет abort committed snapshot.
- [x] Aborted snapshot не меняет `LatestCommitted`.
- [x] Memory, JSONL и embedded SurrealDB shared suite.
- [x] Typed lifecycle regressions для всех трёх backends.
- [-] Remote entity+fact test настроен, hosted run отсутствует.
- [ ] Общий backend-neutral `FactQuery` contract.

#### SurrealDB writer safety

- [x] Process-local write gate для cloned handles.
- [x] Atomic counter и numeric sequence.
- [x] Embedded SurrealKV single-owner regression.
- [x] Confirmed lock/transaction conflicts → retryable `Busy`.
- [x] Data/duplicate/serialization failures → non-retryable adapter error.
- [x] Context-aware bounded retry.
- [-] Remote two-client/32-allocation test настроен, hosted evidence отсутствует.
- [ ] Детерминированно воспроизвести remote write conflict.
- [ ] Определить semantics уже выполняющегося SDK request.

#### Cancellation bridge

- [x] Exclusive cancellation lease на active `operation_id`.
- [x] Duplicate active registration → `Conflict`.
- [x] Same-token/same-id bind идемпотентен.
- [x] Daemon index/generate/wiki/html используют operation-aware scheduler.
- [x] Typed index publish использует bound context.
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
- [x] Production path не содержит local journal v1 или direct `commit_snapshot`.
- [x] Runtime regressions сверяют canonical/read-model/index-state snapshot identity.
- [x] Legacy `crates/athanor-app/src/index.rs` удалён.
- [x] Guard abort’ит prepared snapshot при ошибке до durable journal.
- [x] Journal-write failure не оставляет stranded snapshot.
- [x] Read-model prepare failure очищает journal и abort’ит snapshot.
- [x] Index-state prepare failure откатывает read model, journal и snapshot.
- [x] Canonical publish cancellation восстанавливает прежние artifacts.
- [x] Read-model finalize failure оставляет committed generation recoverable.
- [x] Index-state finalize failure оставляет committed generation recoverable.
- [x] Journal-clear failure восстанавливается после возврата durable journal.
- [x] Recovery отклоняет file вместо read-model directory до mutation.
- [x] Recovery отклоняет directory вместо index-state file до mutation.
- [x] Recovery без backups удаляет uncommitted current artifacts и prepared snapshot.
- [x] Combined publish/rollback/abort failures сохраняют все причины.
- [!] Hosted compile/test/fmt/Clippy evidence отсутствует из-за Actions blocker.

#### Следующий transactional layer

- [ ] Проверять schema и snapshot identity содержимого current/staging/backup artifacts до recovery mutation.
- [ ] Добавить repeated-recovery/idempotence regressions.
- [ ] Объединить canonical data и commit marker в один backend protocol.
- [ ] Добавить backend-neutral `FactQuery`.
- [ ] Ввести immutable generation id для canonical/state/read models.
- [ ] Переключать один current pointer после подготовки всех artifacts.
- [ ] Расширить fault injection на pointer switch и generation cleanup.

Последние implementation-коммиты:

- `30810a5c8f5064218707e7eb72556c08fd699175` — finalize и journal-clear fault suite;
- `bb3967b5a624555a07c7efab129b2bc559e00aa8` — регистрация finalize suite;
- `ca8225a109e977b199ed9991f4a9fcc81358c7cb` — recovery artifact-type preflight;
- `ca3c5d48a4e8ee5682c259640e472558a9c19edb` — malformed/missing backup regressions;
- `124e50e1de9143b9e27644cf79f2f57ede6dd001` — регистрация recovery suite;
- `b422da104a1c8f8fcad00668eee20b5edce30eae` — combined cleanup error regression;
- `e96132f2bd1bcca8dc444fb433dcd06c5d709487` — регистрация combined-error suite.

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
- [x] Typed index publication использует bound context.
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
- [x] Cancellation, runtime publication и recovery fault commands.
- [ ] Common CI failures для остальных workflows.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. `[!]` Включить/проверить GitHub Actions и получить compile/test/fmt/Clippy evidence.
2. P0.4 — content/schema preflight и repeated-recovery idempotence.
3. P0.4 — backend-neutral `FactQuery`.
4. P0.4 — canonical data + commit marker transaction protocol.
5. P0.4 — immutable generation id и один current pointer.
6. P0.4 — remote conflict evidence и pointer-switch fault injection.
7. P1.5 — read-only daemon/CLI/MCP cancellation.
8. P0.3/P0.1/P1.4 — hosted AppSec, matrices, installers и tag evidence.
9. P1.1, daemon/CLI decomposition и performance budgets.
10. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный blocker:** `GitHub Actions activation/visibility`.

**Следующий безопасный кодовый срез до снятия blocker:** `P0.4 recovery content preflight`.

1. Проверить, что current/read-model backup содержат parseable `manifest.json`.
2. Проверить, что current/index-state backup содержат parseable JSON state.
3. Сверить snapshot identity с journal перед restore/cleanup.
4. Не удалять и не переименовывать artifacts при schema/snapshot mismatch.
5. Добавить повторный recovery regression после частично завершённого cleanup.
6. Не переходить к data+marker protocol до hosted compile evidence текущего fault suite.

## 8. Definition of Done проекта

- Public queries изолированы committed snapshot.
- Memory, JSONL и SurrealDB проходят общий conformance suite.
- Publication crash-safe и atomic на generation boundary.
- Recovery fail-closed и идемпотентна при malformed/missing artifacts.
- Embedded ownership fail-closed; remote writers не теряют updates.
- Busy retry bounded attempts/deadline/cancellation.
- Daemon/CLI/MCP имеют эквивалентную cancellation semantics.
- External adapters bounded, cancellable и не оставляют orphan processes.
- Optional feature и AppSec gates блокируют regressions.
- Installers/release assets fail-closed, включая SBOM/provenance.
- CLI/runtime не остаются god modules.
- Roadmap, plan, issues и release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-14 — finalize и recovery fault matrix

- Добавлены deterministic read-model finalize, index-state finalize и journal-clear failures.
- Committed generation остаётся выбранной и завершается typed recovery после снятия transient filesystem fault.
- Recovery preflight отклоняет неверный тип current/staging/backup artifact до destructive mutation.
- Missing backups приводят к удалению только uncommitted current generation и prepared canonical snapshot.
- Combined canonical publish, application rollback и abort failures сохраняются в одной error chain.
- Hosted evidence не повышено: Actions runs/statuses всё ещё отсутствуют.

### 2026-07-14 — production typed publication cutover

- Production index path переведён на `index_runtime.rs`.
- Legacy `index.rs` удалён после миграции broad regressions.
- Typed journal v2, guarded coordinator и backend publication matrix активны в runtime.

### 2026-07-13 — transaction, CI и security baseline

- Добавлены native batch transaction, atomic counter, embedded ownership, remote test configuration, feature matrix, coverage, AppSec и release integrity workflows.
