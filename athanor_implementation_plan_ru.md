# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `e9785a124577a7e971b386f34f7443484d3ab19d`  
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
3. Не назначать coverage thresholds без фактического измерения.
4. Не считать hosted/platform пункт выполненным без run/status evidence.
5. Server-dependent tests не входят неявно в self-contained `--all-features` graph.
6. Один `operation_id` имеет одну живую cancellation authority; остальные владельцы clone существующий handle.
7. Cancellation/deadline не блокируют rollback и recovery.
8. Journal v1 должен читаться после перехода writer на v2.
9. Recovery journal не управляет путями вне ожидаемых `.athanor` artifacts.
10. Store-level typed publication protocol принадлежит `athanor-core`.
11. Backend batch transaction не считается atomic generation publication, пока data, marker и application artifacts переключаются отдельно.

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
- typed coordinator с rollback/recovery;
- production index runtime через `index_runtime.rs` и `publish_prepared_index`;
- broad incremental/validation/cancellation tests мигрированы на production runtime;
- legacy `crates/athanor-app/src/index.rs` удалён;
- feature, quality, AppSec, coverage, installer и release workflow-файлы.

Платформенное состояние:

- [!] GitHub Actions page не показывает workflow list/runs и отображает onboarding;
- [!] connector не возвращает push-run/status evidence;
- [ ] проверить/включить Actions в repository или organization settings;
- [ ] после активации получить первый push-run и исправить реальные findings.

Локальные/hosted verification commands:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-core cancellation --locked
cargo test -p athanor-core prepared_publication --locked
cargo test -p athanor-app index_publication --locked
cargo test -p athanor-app index_runtime_tests --locked
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
| Atomic publication | `[-]` | typed production coordinator, staged read-model/state recovery | data+marker protocol, generation pointer, fault injection |
| Concurrent writers | `[-]` | JSONL lock, atomic counter, embedded ownership | Hosted remote conflict evidence |
| Operation context | `[-]` | deadline, cancellation lease, daemon write path, typed publish context | Read commands, CLI/MCP, in-flight SDK interruption |
| Store transaction boundary | `[-]` | native batch transaction, core prepared protocol, backend matrix, typed runtime | Fact contract, full lifecycle transaction |
| Runtime maintainability | `[-]` | legacy index monolith deleted, focused runtime/coordinator/tests | Further daemon/CLI decomposition |
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

**Статус:** `[-]` — typed production path завершён; lifecycle ещё не atomic на generation boundary.

#### Shared backend contract

- [x] `SnapshotBatch → prepare → commit/abort` lifecycle.
- [x] Невидимость uncommitted/prepared snapshot.
- [x] Запрет abort committed snapshot.
- [x] Aborted snapshot не меняет `LatestCommitted`.
- [x] Memory, JSONL и embedded SurrealDB shared suite.
- [x] Typed lifecycle regressions для всех трёх backends.
- [-] Remote entity+fact test настроен, hosted run отсутствует.
- [ ] Общий backend-neutral Fact query contract.

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
- [x] Coordinator error guard проверяет latest committed перед дополнительным abort.
- [x] Production regression сверяет canonical/read-model/index-state snapshot identity.
- [x] Cancellation, incremental update/remove, relinking, equivalence и validate-only tests используют production runtime.
- [x] Test-only legacy module registration удалена.
- [x] Legacy `crates/athanor-app/src/index.rs` удалён.
- [!] Hosted compile/test/fmt/Clippy evidence отсутствует из-за Actions blocker.

#### Следующий transactional layer

- [ ] Fault injection: journal write, read-model prepare/finalize, state prepare/finalize, canonical publish, recovery.
- [ ] Перенести pre-journal abort guarantee непосредственно в coordinator и добавить regression.
- [ ] Объединить canonical data и commit marker в один backend protocol.
- [ ] Добавить backend-neutral Fact verification.
- [ ] Ввести immutable generation id для canonical/state/read models.
- [ ] Переключать один current pointer после подготовки всех artifacts.

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
- [ ] Read-only daemon jobs/queries.
- [ ] CLI/MCP cancellation lifecycle.
- [ ] Cancellation queued/running reads.
- [ ] Cooperative/isolated synchronous projectors.

### P1.6. Performance budgets

- [ ] Aggregate byte budget, peak RSS и phase timings.
- [ ] 10k/50k/100k scenarios.
- [ ] Incremental/full ratio matrix.

## 5. P2 — governance и contributor experience

- [ ] Branch protection и required checks после появления hosted runs.
- [ ] Issue на каждый открытый P0/P1 package.
- [ ] Acceptance criteria, verification commands и commit links.
- [ ] Conventional commit lint.
- [ ] Automated changelog/release notes.
- [ ] Version/tag/Cargo consistency.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. `[!]` Проверить/включить GitHub Actions и получить compile/test/fmt/Clippy evidence.
2. P0.4 — coordinator fault injection и pre-journal abort regression.
3. P0.4 — объединить canonical data и commit marker.
4. P0.4 — детерминированный remote transaction conflict.
5. P1.5 — read-only daemon/CLI/MCP cancellation.
6. P0.4 — Fact conformance, generation pointer и recovery fault injection.
7. P0.3/P0.1/P1.4 — hosted AppSec, matrices, installers и tag evidence.
8. P1.1, daemon/CLI decomposition и performance budgets.
9. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный пункт:** `P0.4 coordinator fault injection and pre-journal cleanup`.

Следующий кодовый срез:

1. сделать journal persistence injectable в unit test или добавить deterministic filesystem failure fixture;
2. доказать, что journal-write failure вызывает `abort_prepared`;
3. проверить combined error, если cleanup тоже падает;
4. добавить fault points для read-model/state prepare и finalize;
5. не начинать data+marker redesign до реального compile evidence либо полного статического proof по затронутым interfaces.

## 8. Журнал актуализаций

### 2026-07-14 — legacy index cleanup

- Broad runtime regressions перенесены в `index_runtime_tests.rs`.
- Production API покрывает cancellation, incremental add/change/remove, frontmatter relinking, incremental/full equivalence и validate-only.
- Test-only `legacy_index` registration удалена.
- Старый `crates/athanor-app/src/index.rs` удалён.

### 2026-07-14 — typed production cutover

- `index_runtime.rs` использует typed v1/v2 recovery и `publish_prepared_index`.
- Production path не содержит local v1 journal или direct commit.
- Добавлен runtime guard для cleanup ошибок до/после coordinator boundary.

### 2026-07-14 — core protocol и backend matrix

- `PreparedSnapshot`/`PreparedSnapshotPublication` перенесены в `athanor-core`.
- Memory, JSONL и embedded SurrealDB используют один typed lifecycle.

### 2026-07-13 — transaction, remote и security slices

- Добавлены native batch transaction, rollback regression, atomic counter, embedded ownership, remote two-client tests, AppSec, release integrity, coverage measurement и feature matrix.
