# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки кода: `c45659488d3175ac73ab4d914fb6b14d0048f291`  
> Дата актуализации: 2026-07-16  
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
16. `index-current.v2` выдаёт selected paths только после проверки manifest/state и полного checksum chain.
17. Existing immutable generation не seal’ится recovery без совпадения с validated compatibility source.
18. Read-only compatibility service проверяет `OperationContext` до работы и перед выдачей success.
19. Daemon read request получает cancellable job lease и hard deadline независимо от cache/legacy path.
20. Direct CLI read command удерживает cancellation lease до terminal output и не выдаёт late success после Ctrl-C/deadline.
21. MCP notification cancellation применяется только к read tools; transactional tools не отменяются без отдельного rollback review.

## 1. Baseline

Реализовано:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default и opt-in embedded/remote SurrealDB;
- shared Memory/JSONL/SurrealDB query/lifecycle/atomic conformance;
- backend-neutral `FactQuery`, `AtomicSnapshotPublication` и `CanonicalLatestPointer`;
- backend-neutral read operation extension traits;
- backend atomic data+marker boundaries;
- process-local deferred canonical batch barrier;
- snapshot-native runtime publisher и recovery;
- journal v3 с чтением v2/v1;
- backend-neutral immutable `GenerationId`;
- generation identity в canonical marker/record, read-model manifest, index state и recovery journal;
- один checksum-bound `index-current.v2` для application artifacts;
- exact four-file JSONL digest map и immutable state digest;
- pointer-first index-state readers с legacy fallback только при отсутствии pointer;
- repair inspect/retention/publication recovery/cleanup recovery;
- backend-neutral canonical latest discovery, validation и repair;
- daemon Overview/Explain/Search/Context/ChangeMap с operation-aware cancellation/deadline routing;
- focused direct CLI lifecycle для Context/Explain/Overview/Impact/ChangeMap/Search;
- concurrent MCP read lifecycle с request registry и cancellation notifications;
- SurrealDB context-owned allocation metadata и bounded orphan cleanup;
- feature, coverage, AppSec, installer и release workflows.

Платформенное состояние:

- [!] GitHub Actions page показывает onboarding вместо runs;
- [!] connector не возвращает push-run/status evidence;
- [!] DNS clone и локальный Rust toolchain недоступны в рабочей среде;
- [ ] проверить/включить Actions;
- [ ] получить hosted run и исправить реальные findings.

Verification commands:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings

cargo test -p athanor-domain generation --locked
cargo test -p athanor-core latest_pointer --locked
cargo test -p athanor-core read_operation --locked
cargo test -p athanor-app artifact_checksum --locked
cargo test -p athanor-app index_current --locked
cargo test -p athanor-app index_state --locked
cargo test -p athanor-app index_current_runtime_tests --locked
cargo test -p athanor-app index_publication_atomic_tests --locked
cargo test -p athanor-app index_publication_recovery_fault_tests --locked
cargo test -p athanor-app repair_latest --locked
cargo test -p athanor-app repair_cleanup_recovery --locked
cargo test -p athanor-app search_operation --locked
cargo test -p athanor-app derived_read_operation --locked
cargo test -p athanor-app daemon_read_dispatch --locked
cargo test -p athanor-app daemon_derived_read_dispatch --locked
cargo test -p ath --test repair_cli --locked
cargo test -p ath --test direct_read_cli --locked
cargo test -p ath --test index_checksum_cli --locked
cargo test -p ath --test index_checksum_recovery_cli --locked
cargo test -p athanor-transport-mcp lifecycle --locked

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
| Atomic publication | `[-]` | backend boundaries, v2 application pointer, checksum chain, repair protocol | Hosted compile/test/Clippy evidence |
| Allocation recovery | `[-]` | metadata, 24h cutoff, bounded conditional cleanup, tests | Legacy untagged policy, hosted evidence |
| Recovery safety | `[-]` | journals, checksummed pointer recovery, cleanup recovery, idempotence | Hosted/Windows fault evidence |
| Concurrent writers | `[-]` | JSONL/application locks, embedded ownership | Remote conflict evidence |
| Operation context | `[-]` | daemon reads/writes, focused direct CLI reads, concurrent MCP read cancellation | Remaining CLI surfaces, synchronous worker boundary, hosted evidence |
| Runtime maintainability | `[-]` | focused runtime/coordinator, focused read dispatchers, repair/direct-read entry wrappers | Other runtime/daemon decomposition |
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

**Статус:** `[-]` — generation-layer code complete; release-grade статус блокируют hosted feature/OS/backend evidence и branch policy.

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

#### Application artifact checksums — завершено на уровне кода

- [x] `index-current.v2` связывает generation с manifest/state SHA-256 digests.
- [x] Manifest содержит exact map четырёх JSONL файлов.
- [x] Readers проверяют manifest digest, exact file set, каждый data digest и state digest.
- [x] Missing/extra/nested/symlink/non-regular entries fail closed.
- [x] Repair сообщает `index_current_checksum_mismatch`; cleanup блокируется.
- [x] Bridge recovery обновляет v1/pre-checksum generation до v2 идемпотентно.
- [x] Existing immutable target обязан совпадать с compatibility source до sealing.
- [x] Tamper, missing-file, extra-file, state corruption и repeated recovery regressions.
- [x] `index-current.v1` остаётся временно readable только как identity-only migration format.
- [!] Hosted Linux/Windows compile/test/Clippy evidence отсутствует.

#### Generation layer — code complete

- [x] Immutable generation id для canonical/read-model/state/journal.
- [x] Один current pointer после подготовки artifacts.
- [x] Backend latest-pointer repair через generation protocol.
- [x] Pointer switch/cleanup fault injection и recovery.
- [x] Checksums application artifacts.

## 4. P1 — execution safety и maintainability

### P1.1. External process sandbox

- [ ] OS-level filesystem/network/CPU/memory/process limits.
- [ ] Windows Job Objects; Linux launcher/sandbox.
- [ ] Timeout/cancellation/orphan/limit tests.

### P1.2. CLI decomposition

- [-] Transactional repair и direct read команды вынесены в focused entry/parser modules.
- [ ] Bootstrap/parse/dispatch only в legacy `main.rs`.
- [ ] Остальные handlers/rendering modules.
- [ ] Полная help/exit-code/JSON matrix.

### P1.3. Runtime decomposition

- [x] Index runtime отделён, legacy index god-file удалён.
- [x] Publication coordinator focused, transition modules удалены.
- [-] Focused daemon write/read dispatchers и concurrent MCP lifecycle добавлены; legacy dispatchers ещё содержат остальные команды.
- [ ] Injected cancellable ProcessRunner для synchronous/adapter boundaries.
- [ ] Удалить остальные global registries.

### P1.4. Installer verification

- [x] Checksums/fail-closed/tamper/Sigstore/provenance configured.
- [!] Hosted smoke зависит от Actions.
- [ ] Подтвердить version tag.

### P1.5. Operation context E2E

- [x] Cancellation identity lease и stable error contract.
- [x] Busy retry, daemon writes, atomic boundary.
- [x] Core read extension traits с preflight/postflight active checks.
- [x] Overview/Explain/Search/cached Context получают explicit daemon `OperationContext`.
- [x] `Context --diff` и `ChangeMap` проходят через compatible operation wrappers.
- [x] Read jobs получают cancellable registry token и hard deadline.
- [x] Mid-flight cancel/expired deadline не возвращают ложный successful response.
- [x] Structured `cancelled`/`deadline_exceeded` daemon errors и terminal job status.
- [x] Direct CLI Context/Explain/Overview/Impact/ChangeMap/Search получают stable operation id, optional deadline и Ctrl-C cancellation.
- [x] MCP читает requests concurrently, удерживает read cancellation lease до terminal response и принимает cancellation notifications.
- [x] MCP EOF отменяет outstanding reads; transactional `index` остаётся вне notification cancellation scope.
- [x] CLI/MCP regressions покрывают help, malformed deadline, preflight, postflight, cancellation registry и lease cleanup.
- [!] Hosted fmt/test/Clippy/stdio/Ctrl-C evidence отсутствует.
- [ ] Direct CLI lifecycle для `check`, graph queries и manual Rustok read commands.
- [ ] Synchronous worker/projector cancellation semantics.

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

- весь roadmap: ориентировочно `77–81%`;
- generation-layer код P0.4: `100%` по текущему scope;
- daemon read-only operation context: code complete по текущему command scope;
- direct CLI/MCP read lifecycle: code complete по первому focused scope;
- release-grade P0.4 и P1.5 остаются `[-]` до hosted evidence;
- до release-grade P0 остаются 5 hosted/platform evidence packages и policy work;
- после P0 остаются remaining CLI surfaces, sandbox, synchronous worker boundary, decomposition, performance и P2.

## 7. Порядок реализации

1. `[!]` Включить Actions и получить compile/test/fmt/Clippy evidence.
2. P0.4 — hosted JSONL/embedded/remote conflict/latest/checksum evidence.
3. Hosted AppSec/matrix/installer/tag evidence и required checks.
4. P1.5 — remaining direct CLI read surfaces и synchronous worker boundary.
5. P1.1 — OS-level sandbox и resource limits.
6. Остальная decomposition, performance и P2.

## 8. Текущий рабочий пакет

**Активный blocker:** `GitHub Actions activation/visibility`.

**Завершённый кодовый срез:** `P1.5 focused direct CLI and MCP read operation lifecycle`.

**Следующий безопасный кодовый срез:** `P1.5 remaining direct read surfaces and synchronous worker isolation`.

Целевой результат следующего среза:

- `check`, graph queries и manual Rustok read commands используют тот же direct CLI operation lifecycle;
- expensive synchronous discovery/search-index/graph work переносится за cancellable worker boundary либо получает bounded polling points;
- transactional MCP cancellation остаётся запрещённой до explicit rollback review;
- structured cancellation/deadline contract сохраняется во всех entrypoints;
- regressions покрывают worker cancel, deadline, orphan prevention и successful completion.

## 9. Журнал актуализаций

### 2026-07-16 — direct CLI и MCP operation lifecycle

- MCP stdio loop переведён на concurrent request tasks и единый response writer.
- Read requests регистрируются по JSON-RPC id и отменяются через `notifications/cancelled`/`$/cancelRequest`.
- EOF отменяет все outstanding read leases и дожидается terminal task cleanup.
- Direct CLI Context/Explain/Overview/Impact/ChangeMap/Search вынесены в focused parser/runner.
- Добавлены `--deadline-unix-ms`, `ATHANOR_DEADLINE_UNIX_MS` и Ctrl-C cancellation.
- Legacy text/JSON rendering и non-read dispatcher сохранены.
- Transactional MCP tools намеренно не включены в notification cancellation.
- Добавлены unit/process regressions; hosted evidence остаётся внешним blocker.

### 2026-07-16 — read-only operation context

- Добавлены backend-neutral extension traits для context-aware canonical/query/search reads.
- Daemon Overview, Explain, Search и cached Context переведены на explicit `OperationContext`.
- `Context --diff` и `ChangeMap` обёрнуты совместимыми preflight/postflight operation boundaries.
- Read jobs регистрируют cancellation lease, используют hard deadline и stable structured errors.
- Cancellation/deadline больше не проглатываются search fallback’ом и не возвращают ложный success.
- Добавлены core, search и daemon regressions для pre-cancel, mid-flight cancel и deadline.
- Hosted fmt/test/Clippy evidence остаётся внешним blocker.

### 2026-07-15 — application artifact checksums

- Writer `index-current` переведён на schema v2 с manifest/state SHA-256 digests.
- Immutable manifest содержит deterministic exact map четырёх JSONL data files.
- Reader и repair проверяют полный checksum chain до выдачи selected paths.
- Missing, extra, nested, symlinked и modified artifacts отклоняются fail-closed.
- Bridge recovery сравнивает reused generation с compatibility source, seal’ит и обновляет v1 до v2.
- Добавлены CLI regressions для tamper, missing/extra files, state corruption и repeated migration recovery.
- Generation-layer backlog закрыт на уровне кода; hosted evidence остаётся открытым.

### 2026-07-15 — backend latest repair и transactional repair CLI

- Реализованы immutable application generations и один validated `index-current.json`.
- Добавлены bridge journal, standalone publication recovery и cleanup tombstone recovery.
- Добавлен explicit retention protocol с SHA-256 confirmation token.
- CLI получил `index-retention`, `recover-index`, `recover-index-cleanup`, `repair-latest`.
- Добавлен backend-neutral `CanonicalLatestPointer`.
- JSONL latest writer перешёл на generation-bearing schema; legacy pointer query-compatible.
- Authoritative discovery не доверяет mutable pointer и не откатывается к старому generation.
- Memory и SurrealDB сохраняют derived-latest semantics.
