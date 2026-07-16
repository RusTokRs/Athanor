# Athanor: план устранения архитектурных и реализационных проблем

> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Дата актуализации: 2026-07-16  
> Статус: active consolidated implementation plan

## 0. Правила исполнения

Статусы:

- `[x]` — код и воспроизводимые regressions добавлены;
- `[-]` — реализация частичная либо отсутствует hosted/platform evidence;
- `[ ]` — не начато;
- `[!]` — внешний blocker.

Правила:

1. Все изменения коммитятся напрямую в `main`, пока пользователь явно не изменит режим.
2. Один коммит — одна логически завершённая часть.
3. Hosted/platform пункт не закрывается без run/status/artifact evidence.
4. Coverage threshold не назначается без фактического измерения.
5. Cancellation/deadline не блокируют rollback и recovery.
6. Durable success не превращается в post-commit cancellation error.
7. Blocking worker не detach’ится: terminal error возвращается только после drain.
8. Read-only operation проверяет active state до работы, внутри поддерживаемых hot loops и перед success.
9. Derived rebuild выполняется через staging; current artifact меняется только после готовности staged artifact.
10. Rebuild-capable transport держит cancellation lease до worker/future cleanup.
11. Projector publication проверяет cancellation после полного staging и до mutation current output.
12. Operation-aware CPU builders используют bounded cooperative checkpoints; legacy pure APIs сохраняются.
13. Transactional MCP cancellation остаётся запрещённой без отдельного rollback review.
14. Queries читают только committed canonical snapshots.
15. Backend atomic publication не считается единым application generation pointer.
16. Recovery fail-closed проверяет path/type/schema/snapshot/generation/checksum identity до mutation.
17. `GenerationId` детерминированно выводится из уникального `SnapshotId`.
18. Canonical latest repair не может перемотать visibility на более старый committed snapshot.
19. Destructive retention требует token от точного dry-run plan.

## 1. Текущий baseline

На уровне кода реализованы:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default и opt-in embedded/remote SurrealDB;
- shared Memory/JSONL/SurrealDB query/lifecycle/atomic conformance;
- backend-neutral `FactQuery`, `AtomicSnapshotPublication`, `CanonicalLatestPointer`;
- immutable `GenerationId` во всех publication artifacts;
- checksum-bound `index-current.v2` и immutable application generations;
- transactional recovery, cleanup tombstones, retention и backend latest repair;
- daemon read/write operation context и stable typed errors;
- focused direct CLI lifecycle для стандартных reads, check, standard graph и manual Rustok surfaces;
- concurrent MCP read lifecycle с request cancellation registry;
- operation-aware drained MCP Search/Context routing;
- non-orphaning blocking workers для graph, strict API diff, Rustok reports, Context и Search rebuild;
- cooperative checkpoints внутри related/path/PageRank/cycle algorithms;
- cooperative filesystem discovery и file hashing в built-in source и diff scanner;
- staged operation-aware Tantivy rebuild с backup/rollback и bounded document checkpoints;
- outer staged publication guard для runtime wiki/html projectors;
- cooperative operation-aware Rustok architecture-context builder;
- cooperative FFA/FBA/Page Builder audit builders;
- cooperative FFA/FBA/Page Builder graph builders;
- feature, coverage, AppSec, installer и release workflows.

Платформенное состояние:

- [!] GitHub Actions page/run visibility не подтверждена;
- [!] connector не возвращает push-run/status evidence;
- [!] DNS checkout и локальный Rust toolchain недоступны в рабочей среде;
- [ ] включить/подтвердить Actions;
- [ ] получить hosted run и исправить реальные findings.

Основные verification commands:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings

cargo test -p ath --test direct_read_cli --locked
cargo test -p ath --test direct_graph_cli --locked
cargo test -p ath --test direct_check_cli --locked
cargo test -p ath direct_operation --locked
cargo test -p ath direct_context_cli --locked
cargo test -p ath direct_search_cli --locked
cargo test -p ath direct_rustok_cli --locked
cargo test -p ath direct_rustok_help --locked

cargo test -p athanor-source-fs --locked
cargo test -p athanor-search-tantivy --locked
cargo test -p athanor-runtime-defaults projector_operation --locked
cargo test -p athanor-app graph_operation --locked
cargo test -p athanor-app graph_cooperative --locked
cargo test -p athanor-app rustok_architecture_cooperative --locked
cargo test -p athanor-app rustok_audit_cooperative --locked
cargo test -p athanor-app rustok_graph_cooperative --locked
cargo test -p athanor-app rustok_operation --locked
cargo test -p athanor-app derived_read_operation --locked
cargo test -p athanor-app search_operation --locked
cargo test -p athanor-app daemon_derived_read_dispatch --locked
cargo test -p athanor-transport-mcp lifecycle --locked

cargo test -p athanor-store-memory --test conformance --locked
cargo test -p athanor-store-jsonl --test conformance --locked
cargo test -p athanor-store-surrealdb --test conformance --locked
```

## 2. Сводный статус

| Область | Статус | Подтверждено кодом | Остаётся |
| --- | --- | --- | --- |
| Query isolation | `[-]` | committed exact/latest, generation identity | Hosted remote evidence |
| Transactional publication | `[-]` | generation layer code complete, repair/checksums/fault regressions | Hosted backend/OS evidence |
| Allocation recovery | `[-]` | metadata, cutoff, bounded cleanup | Legacy untagged policy, hosted evidence |
| Concurrent writers | `[-]` | JSONL/application locks, embedded ownership | Hosted remote conflict evidence |
| Operation context | `[-]` | daemon, CLI, MCP reads, graph/discovery/search/projector/Rustok polling, worker/future drain | Hosted evidence, transactional MCP review |
| Runtime maintainability | `[-]` | focused coordinator/read/graph/check/Rustok/Context/Search modules | ProcessRunner, remaining legacy decomposition |
| Hosted CI | `[!]` | workflow files | Actions runs, artifacts, required checks |
| AppSec | `[-]` | configured gates/SBOM/signing | Hosted evidence, push protection |
| Release integrity | `[-]` | fail-closed installers/provenance | Hosted/tag evidence |
| Default build | `[x]` | remote SurrealDB opt-in | Maintain boundary |

## 3. P0 — release-blocking quality and security

### P0.1. CI feature matrix

- [x] default/no-default, `store-surreal`, `js-ts-precision`, `--all-features`.
- [x] locked check/test/Clippy, SHA-pinned actions, Rust `1.95.0`.
- [!] Включить/подтвердить Actions.
- [ ] Hosted slices и required checks.

### P0.2. Coverage

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV/JSON/HTML artifacts.
- [!] Получить hosted artifact.
- [ ] Выбрать baseline/floor по измерению.
- [ ] Включить no-regression и changed-lines gate.

### P0.3. AppSec и supply chain

- [x] Dependency review, CodeQL, Gitleaks, blocking Zizmor.
- [x] Immutable action pins, SBOM, checksums, Sigstore, provenance.
- [x] Release verification блокирует publish.
- [!] Hosted AppSec/tag evidence.
- [ ] Secret push protection.

### P0.4. Store conformance и transactional publication

**Статус:** `[-]` — generation-layer code complete; release-grade статус блокируют hosted feature/OS/backend evidence и branch policy.

#### Shared backend contract

- [x] Query/lifecycle/atomic suites для Memory/JSONL/SurrealDB.
- [x] Uncommitted/prepared snapshots невидимы.
- [x] Commit exact/latest visible; abort не меняет latest.
- [x] Backend-neutral `FactQuery` и latest pointer contracts.
- [x] JSONL authoritative discovery не доверяет mutable latest pointer.
- [x] Memory/SurrealDB derived-latest validation запрещает rewind.
- [-] Remote reader configured; hosted evidence отсутствует.

#### Atomic coordinator/recovery

- [x] Journal v3 с v2/v1 compatibility.
- [x] Journal/staging предшествуют final canonical mutation.
- [x] Explicit complete batch через atomic capability.
- [x] Exact probe перед rollback/abort.
- [x] Exact committed generation не abort’ится.
- [x] Recovery path/type/schema/snapshot/generation preflight.
- [x] Pointer/finalize/clear/combined-error regressions.
- [x] Repeated recovery идемпотентен.
- [!] Hosted compile/test/fmt/Clippy отсутствует.

#### Generation layer — code complete

- [x] Immutable generation id для canonical/read-model/state/journal.
- [x] Один atomic application current pointer после подготовки artifacts.
- [x] Backend latest-pointer repair через generation protocol.
- [x] Pointer switch/cleanup fault injection и recovery.
- [x] Checksums application artifacts.
- [x] Explicit retention и checksum confirmation token.
- [x] Tamper/missing/extra/state corruption regressions.

#### Release-grade P0 evidence packages

- [ ] Hosted feature matrix.
- [ ] Hosted coverage artifact и выбранный floor.
- [ ] Hosted AppSec/SBOM/signing verification.
- [ ] Hosted installer/tag smoke.
- [ ] Hosted JSONL/embedded/remote transactional evidence.

## 4. P1 — execution safety и maintainability

### P1.1. External process sandbox

- [ ] OS-level filesystem/network/CPU/memory/process limits.
- [ ] Windows Job Objects и Linux launcher/sandbox.
- [ ] Timeout/cancellation/orphan/limit platform tests.

### P1.2. CLI decomposition

- [-] Transactional repair, focused reads, Context, Search, check, standard graph и manual Rustok вынесены в entry/parser modules.
- [ ] Оставить в legacy `main.rs` только bootstrap/parse/dispatch compatibility.
- [ ] Вынести остальные handlers и rendering modules.
- [ ] Полная help/exit-code/JSON matrix.

### P1.3. Runtime decomposition

- [x] Index runtime отделён; legacy index god-file удалён.
- [x] Publication coordinator focused; transition modules удалены.
- [-] Focused daemon read/write dispatchers и concurrent MCP lifecycle добавлены.
- [x] Graph, strict API diff, Rustok reports и search rebuild имеют non-orphaning blocking boundary.
- [x] Graph, discovery, search, projector publication и все operation-aware Rustok builders имеют cooperative polling/guard.
- [x] Wiki/html runtime projector publication имеет final cancellation guard до current-output swap.
- [x] Operation-aware search factory добавлен без изменения legacy `RuntimeComposition::new`.
- [x] Diff Context не использует drop-on-timeout вокруг operation-aware rebuild.
- [x] MCP Search/Context используют drained operation dispatch вместо compatibility future drop.
- [ ] Injected cancellable `ProcessRunner` для adapter boundaries.
- [ ] Удалить остальные global registries.

### P1.4. Installer verification

- [x] Checksums, fail-closed tamper detection, Sigstore и provenance configured.
- [!] Hosted Linux/Windows smoke зависит от Actions.
- [ ] Подтвердить version tag.

### P1.5. Operation context E2E

- [x] Exclusive cancellation identity lease и stable error contract.
- [x] Daemon writes/reads, busy retry и atomic boundary.
- [x] Backend-neutral read extension traits с preflight/postflight.
- [x] Direct Context/Explain/Overview/Impact/ChangeMap/Search/Check lifecycle.
- [x] Standard graph export/related/path/hubs/PageRank/cycles lifecycle.
- [x] Manual Rustok audit/context/FFA/FBA/Page Builder graph lifecycle.
- [x] `--deadline-unix-ms`, environment deadline и Ctrl-C cancellation.
- [x] Graph/strict API/Rustok/Context/Search blocking workers drained before terminal response.
- [x] Cooperative checkpoints внутри related BFS, shortest-path BFS, PageRank и cycle DFS.
- [x] Cooperative polling внутри filesystem traversal, file reads и hash chunks.
- [x] Cooperative polling внутри search document conversion и Tantivy rebuild batches.
- [x] Staged search rebuild сохраняет current index при cancel/error и удерживает backup до successful reopen.
- [x] Daemon освобождает stale cached index handle до directory swap.
- [x] Diff Context deadline не drop’ит search rebuild future до cleanup.
- [x] MCP Search/Context держат registry lease до operation-aware cleanup.
- [x] MCP terminal cancellation/deadline имеет приоритет над simultaneous worker error после drain.
- [x] MCP concurrent read cancellation, EOF cleanup и stable structured errors.
- [x] Runtime wiki/html projectors строят output во внешнем staging и проверяют late cancellation до swap.
- [x] Failed projector swap восстанавливает предыдущий output; post-swap backup cleanup не отменяет durable success.
- [x] Operation-aware Rustok architecture context проверяет active state между bounded work batches.
- [x] Architecture parity regression сохраняет legacy output schema и selection semantics.
- [x] Cooperative polling внутри FFA/FBA/Page Builder audit builders.
- [x] Cooperative polling внутри Rustok FFA/FBA/Page Builder graph builders.
- [x] Audit parity regressions сохраняют legacy schemas, sorting и completion calculations.
- [x] Graph parity regression покрывает все девять Rustok graph builders.
- [x] Deterministic architecture/audit/graph cancellation regressions останавливают работу после нескольких batches.
- [x] Parser/process/worker regressions для help, malformed/expired deadline, preflight, drain и success.
- [x] Deterministic search regression отменяет rebuild после нескольких batches.
- [!] Hosted fmt/test/Clippy/stdio/Ctrl-C/disconnect/directory-swap/worker evidence отсутствует.
- [ ] Transactional MCP notification cancellation — только после rollback review.

### P1.6. Performance budgets

- [ ] Aggregate byte/RSS/phase budgets.
- [ ] 10k/50k/100k scenarios.
- [ ] Incremental/full ratio matrix.

## 5. P2 — governance

- [ ] Branch protection, required checks и secret push protection.
- [ ] Issues/acceptance criteria для открытых P0/P1 packages.
- [ ] Conventional commit lint, changelog и version consistency.
- [ ] `justfile`, `xtask` или unified verification entrypoint.

## 6. Оценка готовности

Оценка остаётся диапазоном, а не release assertion:

- весь roadmap: ориентировочно `83–87%`;
- generation-layer код P0.4: `100%` по текущему scope;
- P1.5 operation lifecycle: ориентировочно `98–99%` на уровне кода;
- release-grade P0.4/P1.5 остаются `[-]` до hosted evidence;
- до release-grade P0 остаются пять hosted/platform evidence packages и policy work;
- после P0 остаются ProcessRunner/sandbox, decomposition, performance и P2;
- transactional MCP notification cancellation остаётся отдельным review-gated решением, а не автоматическим следующим изменением.

## 7. Порядок реализации

1. `[!]` Включить Actions и получить compile/test/fmt/Clippy evidence.
2. Получить P0 hosted JSONL/embedded/remote conflict/latest/checksum evidence.
3. Получить hosted AppSec/matrix/installer/tag evidence и включить required checks.
4. P1.3/P1.1 — injectable cancellable `ProcessRunner` и безопасная external-process boundary.
5. P1.1 — OS-level sandbox и resource limits поверх ProcessRunner.
6. Остальная CLI/runtime decomposition.
7. Performance budgets.
8. P2 governance.
9. Transactional MCP cancellation — только после отдельного rollback/durable-success review.

## 8. Текущий рабочий пакет

**Активный внешний blocker:** `GitHub Actions activation/visibility`.

**Завершённый кодовый срез:** `P1.5 cooperative Rustok audit and graph builders`.

Завершённый результат:

- operation-aware FFA/FBA/Page Builder audit paths используют dedicated cooperative builders;
- checkpoints добавлены в entity/relation/fact/diagnostic/module aggregation loops;
- FBA dependency resolution и completion summary считаются с bounded polling;
- operation-aware FFA, FBA и Page Builder graph paths используют cooperative variants;
- checkpoints добавлены в root lookup, selection, evidence extraction, edge assembly и final compaction;
- legacy pure audit/graph APIs и output schemas не изменены;
- audit parity regressions сравнивают три cooperative reports с legacy builders;
- graph parity regression сравнивает все девять cooperative graphs с legacy builders;
- deterministic cancellation regressions останавливают audit и graph builders после нескольких batches;
- outer Rustok worker drain остаётся обязательной terminal boundary.

**Следующий безопасный кодовый срез:** `P1.3 injectable cancellable ProcessRunner foundation`.

Целевой результат следующего среза:

- external adapter process запуск инкапсулирован за injected `ProcessRunner`;
- operation deadline и cancellation передаются в process lifecycle;
- timeout/cancel завершает process tree, а runner дожидается cleanup;
- stdout/stderr имеют bounded capture и стабильную error classification;
- production default и test fake runner не требуют global mutable registry;
- legacy adapter behavior и command arguments сохраняются;
- regressions покрывают success, spawn failure, timeout, cancellation, output limit и orphan prevention.

## 9. Журнал актуализаций

### 2026-07-16 — cooperative Rustok audit и graph builders

- Добавлены dedicated cooperative FFA/FBA/Page Builder audit builders.
- Добавлены cooperative variants для девяти Rustok graph builders.
- Operation-aware wrappers переключены на cooperative implementations.
- Checkpoints охватывают indexing, selection, evidence, edge assembly и compaction.
- Legacy pure builders и serialized schemas сохранены.
- Добавлены parity regressions для трёх audits и девяти graphs.
- Добавлены multi-batch cancellation regressions для audit и graph workloads.
- Hosted build/fmt/test/Clippy evidence не заявлено.

### 2026-07-16 — projector publication guard и cooperative architecture context

- Runtime wiki/html projectors обёрнуты во внешний staged publication guard.
- Late cancellation проверяется после inner build и до current-output swap.
- Previous output сохраняется при cancel и swap failure.
- Post-swap cleanup не может превратить durable success в operation error.
- Добавлен cooperative Rustok architecture-context builder с bounded checkpoints.
- Operation-aware architecture wrapper переключён на cooperative variant.
- Legacy projector и architecture APIs сохранены.
- Добавлены late-cancel, parity и multi-batch cancellation regressions.
- Hosted build/fmt/test/Clippy evidence не заявлено.

### 2026-07-16 — cooperative discovery и search-index polling

- Built-in filesystem source реализует настоящий `discover_with_context`.
- Directory traversal переведён на iterative stack с checkpoints на entries, reads и hash chunks.
- Application diff Context использует отдельный operation-aware scanner.
- Добавлен optional operation-aware `SearchIndexFactory` без изменения legacy constructor.
- Production и legacy runtime регистрируют cancellable Tantivy rebuild.
- Direct Context/Search вынесены в focused drained interceptors.
- Context, daemon cache и MCP Search/Context paths используют operation-aware rebuild.
- Tantivy rebuild выполняется через staging/backup/rollback; stale daemon handle освобождается до swap.
- Daemon diff Context больше не drop’ит rebuild future через outer timeout.
- MCP Search/Context держат cancellation lease до cleanup и применяют terminal-state precedence.
- Mid-batch cancellation regression сохраняет current index и удаляет staging.
- Hosted build/fmt/test/Clippy evidence не заявлено.

### 2026-07-16 — manual Rustok lifecycle и cooperative graph polling

- Добавлены operation-aware wrappers для Rustok architecture context, FFA/FBA/Page Builder audits и graphs.
- Manual Rustok CLI surfaces вынесены в focused interceptor с deadline, environment fallback и Ctrl-C lifecycle.
- Legacy text/JSON rendering и существующие arguments сохранены.
- Related/path/PageRank/cycle algorithms получили bounded cooperative checkpoints.
- Outer worker drain сохранён как защита от orphan и late success.
- Help/process/unit regressions обновлены под новый direct contract.
- Hosted build/fmt/test/Clippy evidence не заявлено.

### 2026-07-16 — graph/check worker lifecycle

- Standard graph commands и strict API diff вынесены в operation-aware non-orphaning workers.
- Canonical snapshot загружается через context-aware store read.
- Cancel/deadline отклоняет late success только после worker drain.

### 2026-07-16 — direct CLI и MCP operation lifecycle

- MCP stdio loop переведён на concurrent request tasks и cancellation notifications.
- Direct standard reads вынесены в focused parser/runner.
- Transactional MCP tools намеренно исключены из notification cancellation.

### 2026-07-15 — generation layer

- Завершены immutable generations, единый current pointer, backend latest repair, fault recovery и checksums.
- Generation-layer backlog закрыт на уровне кода; hosted evidence остаётся открытым.
