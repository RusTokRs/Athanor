# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Назначение: рабочий implementation plan для последовательного улучшения Athanor  
> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `08bacdf75952156e77bb6803ec99475718ae12fc`  
> Дата актуализации: 2026-07-13  
> Статус: active implementation plan

## 0. Правила исполнения

Статусы:

- `[x]` — реализовано и подтверждено воспроизводимой проверкой либо доступным hosted evidence;
- `[-]` — реализация частичная либо ожидает hosted evidence/enforcement;
- `[ ]` — не начато;
- `[!]` — блокирует релиз или следующий обязательный этап.

Правила:

1. Один коммит — одна логически завершённая часть.
2. Не назначать числовые thresholds без фактического измерения.
3. Не переводить hosted/platform пункт в `[x]`, пока нет доступного run/status evidence.
4. Изменения архитектуры, release, CI и security одновременно отражать в документации и этом плане.
5. Пока разрешены прямые коммиты в `main`, сохранять Conventional Commits и воспроизводимые проверки. После branch protection перейти на PR.
6. Отсутствие push-triggered workflow run в ответе GitHub-коннектора не является ни успехом, ни ошибкой.
7. Process-local serialization не считается cross-process lease.
8. Одна backend transaction для `SnapshotBatch` не считается атомарной publication всей generation, пока commit marker и read models переключаются отдельно.
9. Не отмечать object-kind visibility подтверждённой, если public contract не позволяет прочитать этот kind независимо.
10. Embedded exclusive ownership и remote multi-writer semantics считать разными контрактами и тестировать отдельно.
11. Server-dependent tests не должны неявно запускаться в self-contained `--all-features` graph.

## 1. Текущий baseline

Подтверждено в коде и workflow:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in SurrealDB;
- snapshot-aware query/publication contracts;
- shared query и snapshot-lifecycle conformance для Memory, JSONL и embedded SurrealDB;
- отдельная `Store Conformance` matrix для embedded backend и dedicated remote job;
- process-local async write gate для клонированных SurrealDB store handles;
- atomic SurrealDB counter increment и numeric snapshot sequence;
- native SurrealQL transaction для полного `SnapshotBatch` с statement-error checking и rollback test;
- отдельный `prepared` state и запрет late writes после prepare;
- persistent SurrealKV exclusive-owner regression с двумя независимыми `connect()`;
- stable retry mapping: lock contention и подтверждённые transaction conflicts → `Busy`, data/statement failures → non-retryable `AdapterExecution`;
- deadline-bounded retry schedule 10/25/50/100 ms для context-aware SurrealDB writes/publication;
- opt-in `remote` WebSocket feature и ignored-by-default two-connection tests;
- Docker job с SurrealDB `2.6.5`, remote allocation и cross-connection canonical visibility;
- Linux/Windows/macOS default quality matrix и Linux optional-feature matrix;
- measurement-only source coverage job;
- AppSec workflow: dependency review, CodeQL v4, Gitleaks и Zizmor;
- immutable SHA pinning GitHub Actions;
- signed platform archives, per-binary checksums, fail-closed installers;
- signed CycloneDX SBOM и release verification перед publish.

Базовые команды:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test -p athanor-store-memory --locked
cargo test -p athanor-store-jsonl --locked
cargo test -p athanor-store-surrealdb --locked
ATHANOR_SURREAL_REMOTE_URI=ws://127.0.0.1:8000 \
  cargo test -p athanor-store-surrealdb --features remote --test remote --locked -- --ignored
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 2. Сводный статус ранее выявленных проблем

| Пункт | Статус | Что подтверждено | Что остаётся |
| --- | --- | --- | --- |
| 3.1 Query contract по `EntityId` | `[-]` | ID-based queries, `EntityResolver`, shared embedded conformance, remote contract configured | Hosted remote evidence и persistent server path |
| 3.2 Snapshot isolation | `[-]` | exact/latest selectors, committed-only reads, prepared immutability, shared lifecycle | Полная transport matrix и generation-level visibility |
| 3.3 Atomic publication | `[-]` | JSONL staging/recovery; SurrealDB atomic batch rollback | Commit marker, read models и единый generation pointer |
| 3.4 Concurrent writers | `[-]` | JSONL lock/sequence; Surreal atomic counter; embedded exclusive ownership; bounded Busy retry; remote two-client test configured | Hosted remote evidence, observed write conflict и cancellation |
| 3.5 Async process runner | `[-]` | Tokio, bounded streams, cancellation, Unix process groups | Windows Job Objects |
| 4.1 Adapter trust/sandbox | `[-]` | digest binding, path checks, clean environment support | Resource/network/filesystem isolation |
| 4.2 Operation context | `[-]` | deadlines in main paths; deadline-bounded SurrealDB retry | Полная E2E matrix и cancellation primitive для store retry |
| 4.3 Pipeline memory | `[-]` | extraction concurrency and byte limits | Aggregate phase budget |
| 4.4 Incremental invalidation | `[-]` | file-local planning/dependency closure | add/remove/rename/cross-file matrix |
| 4.5 Storage transaction boundary | `[-]` | `SnapshotBatch`, shared lifecycle, JSONL prepare, native Surreal batch transaction, remote cross-connection canonical check configured | Backend-neutral Fact contract, portable prepared handle, full lifecycle transaction |
| 4.6 Runtime composition | `[-]` | `RuntimeComposition` in main paths | Remove global registry, inject process runner |
| 4.7 Public API boundaries | `[-]` | focused namespaces/exports | Retire wildcard compatibility surface |
| 4.8 Module decomposition | `[-]` | partial pipeline/plugin/daemon split | CLI/runtime god modules |
| 4.9 Typed errors/protocol | `[-]` | stable codes, daemon v3, MCP error data, Surreal Busy mapping/backoff | Full transport compatibility, cancellation и observed remote-conflict matrix |
| 4.10 Default build | `[x]` | SurrealDB excluded by default; remote protocol opt-in; server tests ignored by default | Maintain boundary |
| 5.1 Hosted CI governance | `[-]` | workflows and matrices exist | Hosted evidence, branch protection, required checks |
| 5.2 Coverage measurement | `[-]` | LCOV/JSON/HTML artifact job | Review real baseline before any gate |
| 5.3 AppSec automation | `[-]` | dependency review, CodeQL v4, Gitleaks, Zizmor, immutable pins, signed SBOM | Hosted evidence, blocking Zizmor, push protection |
| 5.4 Strict config | `[x]` | strict parsing and validate/doctor | Maintain contract tests |
| 5.5 Governance | `[-]` | templates, policies, Dependabot | Commit lint, release policy, issue traceability |

## 3. P0 — release-blocking quality and security gates

### P0.1. CI feature matrix

**Статус:** `[-]` — реализация и документация готовы; hosted run и required-check enforcement не подтверждены.

- [x] default/no-default, `store-surreal`, `js-ts-precision`, `--all-features`.
- [x] Для каждого slice: check/test/Clippy с `--locked`.
- [x] Remote server tests ignored by default и не ломают self-contained `--all-features`.
- [x] Документация в `docs/development/ci.md`.
- [ ] Подтвердить hosted run всех slices.
- [ ] Сделать feature matrix required check после branch protection.

Реализация: `d4b437b00fad2b5c2b078f9fb1c684d666e6122f`, `53a68dbba978c6ad6b172faf476df118ba679b9c`, `29ea91a4d46684971de88baa2a0018ccaefdbc57`, `027d6c0baa8cfa807ebc7a062e6c3036a0683ecb`.

### P0.2. Coverage measurement; blocking gate отложен

**Статус:** `[-]` — измерение полезно, но жёсткий no-regression gate без baseline и PR flow не обоснован.

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV, JSON summary и HTML artifact.
- [x] Локальные команды задокументированы.
- [ ] Получить и проверить первый hosted artifact.
- [ ] Решить по данным, нужен общий floor или targeted coverage для core/store/publication.

Реализация: `1d5516d8a612ab81a593b83a8b3352ff1bb2468f`, `83efc933c7a5a0363114e3cda572d52521aa4905`.

### P0.3. PR-level AppSec и software supply chain

**Статус:** `[-]` — основной технический срез реализован; hosted evidence и окончательное enforcement не подтверждены.

- [x] Dependency review: новые уязвимости `moderate` и выше.
- [x] CodeQL v4 для Rust, `security-extended`, locked all-features build.
- [x] Gitleaks full-history scan.
- [x] Pinned Zizmor `1.26.1` в offline strict-collection режиме.
- [-] Zizmor report-only до review первого hosted report.
- [x] Все текущие `uses:` pinned на immutable SHA.
- [x] CycloneDX 1.5 SBOM, checksum, Sigstore и provenance.
- [x] Release `verify` блокирует `publish` до проверки archives и SBOM.
- [x] Release permissions разделены по jobs.
- [ ] Подтвердить hosted AppSec jobs и tag release pipeline.
- [ ] Удалить `--no-exit-codes` после разбора Zizmor findings.

### P0.4. Store conformance и transactional publication

**Статус:** `[-]` — четвёртый remote/retry slice реализован; hosted evidence, наблюдаемый remote conflict и generation publication остаются.

#### Shared backend contract

- [x] Расширить `athanor-store-conformance` lifecycle-контрактом `SnapshotBatch → prepare → commit/abort`.
- [x] Проверять невидимость uncommitted/prepared snapshot.
- [-] Batch fixture включает entity/fact/relation/diagnostic; entity/relation/diagnostic visibility проверяется независимо, но `KnowledgeStore` пока не имеет fact-query.
- [x] Проверять запрет abort committed snapshot.
- [x] Проверять, что aborted snapshot удаляется и не меняет `LatestCommitted`.
- [x] Запускать query и lifecycle suites для Memory, JSONL и real embedded SurrealDB.
- [x] Добавить dedicated `Store Conformance` workflow matrix.
- [x] Добавить isolated remote job с SurrealDB `2.6.5` и WebSocket client feature.
- [-] Remote tests проверяют entity+fact через `CanonicalSnapshotStore`, но hosted run ещё не подтверждён.
- [x] Документировать локальные команды, доказанные гарантии и ограничения в `docs/development/ci.md`.
- [ ] Подтвердить успешные hosted embedded и remote jobs.

#### SurrealDB writer safety

- [x] Общий async write gate для clones одного `SurrealKnowledgeStore`.
- [x] Идемпотентно инициализировать counter record.
- [x] Заменить read-modify-write counter на atomic `UPDATE ONLY ... count += 1`.
- [x] Хранить numeric snapshot sequence и выбирать latest по sequence с fallback по historical id.
- [x] Добавить 32-way test с отдельными process-local writer gates поверх одного backend client.
- [x] Зафиксировать embedded SurrealKV как single-owner process model.
- [x] Добавить persistent-path regression: первый `connect()` успешен, второй к тому же каталогу fail-closed.
- [x] Классифицировать embedded lock contention как `CoreError::Busy` с `is_retryable() == true`.
- [x] Классифицировать подтверждённые сообщения `Transaction write conflict`, `Transaction retry required` и `Transaction conflict:` как retryable `Busy`.
- [x] Оставить duplicate/data/serialization/прочие statement failures non-retryable `AdapterExecution`.
- [x] Ввести bounded retry schedule 10/25/50/100 ms только для context-aware write/publication methods.
- [x] Проверять deadline перед попыткой и не спать, если следующий backoff не помещается в оставшийся budget.
- [x] Не retry-ить plain methods и non-retryable errors.
- [-] Добавить два независимых remote SDK connections и 32 concurrent allocations: код и CI job готовы, hosted evidence отсутствует.
- [ ] Детерминированно воспроизвести реальный remote write conflict и подтвердить public `Busy` mapping.
- [ ] Добавить cancellation-aware retry после расширения `OperationContext`.

#### Native SurrealDB batch transaction

- [x] Locked backend: SurrealDB `2.6.5`.
- [x] Сериализовать все object arrays до открытия backend transaction.
- [x] Выполнять non-empty entity/fact/relation/diagnostic inserts внутри одного `BEGIN`/`COMMIT` query.
- [x] Вызывать response `check()` и не считать outer request success доказательством успешных statements.
- [x] Добавить rollback regression: duplicate ID обязан завершить batch ошибкой, после чего clean retry с тем же ID обязан пройти.
- [x] Добавить отдельный `prepared` state и запрещать individual/batch writes после prepare.
- [-] Counter allocation atomic, но snapshot-record creation идёт отдельным request и может оставить sequence gap при crash.
- [-] Batch data transaction не включает последующий commit marker.
- [-] Abort удаляет canonical rows transactionally, но snapshot metadata удаляется отдельным request.

#### Transactional publication

- [-] Remote `CanonicalSnapshotStore` test независимо читает entity и fact после commit; общий backend-neutral Fact conformance ещё отсутствует.
- [ ] Ввести portable prepared transaction handle вместо неявного lifecycle по `SnapshotId`.
- [ ] Объединить batch data и commit marker в один проверяемый publication protocol.
- [ ] Ввести единый immutable generation id для canonical/state/read models.
- [ ] Переключать один current pointer после подготовки всех artifacts.
- [ ] Добавить fault injection для counter/create/write/prepare/commit/pointer/recovery.

#### Реализация

Первый срез:

- `1f1555c992a02db60eb1fbf7af38afa9e6e59610` — SurrealDB process-local write coordination wrapper;
- `bba33de8faba9ec9c8d0daedcdbf29ece61c1ce2` — Tokio sync primitives;
- `b91eb5c1089af8b4f94902bc730a43f57e2b6e40` — serialized facade как crate entrypoint;
- `d9bafc703de583cc70c8ce0fcea2714a447e05e2` — shared lifecycle conformance suite;
- `5ca54c6ff251fb8e50f3d8198a9f0a43a6c072c4` — Memory lifecycle integration;
- `1f824e56813893d8cf4b312ea3f54465d464e530` — JSONL lifecycle integration;
- `ea57e7ffc2d7733a2623569c46fe3cc99d14c28f` — SurrealDB lifecycle и concurrent allocation tests;
- `615f4f7b2237385930ef53b9f8c8d6ff30a0443c` — backend conformance CI matrix.

Второй срез:

- `664ee557526060fc5bce85111aa4f262d75606c5` — native batch transaction, atomic counter, rollback и prepared immutability tests;
- `5162733283dfd18c298e8b47be09808095f0ac39` — architecture transaction boundaries;
- `301dddc73b7390002e482252bb8b19d7587b813c` — CI test contract и troubleshooting.

Третий срез:

- `c716bd35982c49f56212b4de02615f085e3464f0` — public retry-classification facade;
- `f61824d710e67ee0cc9bd0d869e7d22c87c8f062` — facade как active crate root;
- `036d79f02b7481f00678b14336b2f579550285e5`, `9adfe0e4b7bb5a4f133bfd16c71394d51c9cdbf3`, `feea066330266fff79d2dbe31dfd00eff63a6c22` — persistent lock regression и production endpoint alignment;
- `47a6504b32a4bc8750062874cf6b65d9891e8213` — embedded ownership/retry architecture;
- `5c74aea6caa9acb90cab8f355cce121ed8e8c32d` — CI contract и troubleshooting.

Четвёртый срез:

- `7f86d8e180c036c6a4962512636bbff849094925` — opt-in remote WebSocket feature;
- `8a05d3ae1155d12800dcd191b4e7589dd87b260c` — deadline-bounded Busy retry;
- `d3f3c421b00b7b4df8b32708b1afb5968802f105` — remote two-connection allocation/publication tests;
- `7c95fcb8049641f984d2200118edc6521bf04ffb` — Docker-backed remote CI job;
- `29ea91a4d46684971de88baa2a0018ccaefdbc57`, `027d6c0baa8cfa807ebc7a062e6c3036a0683ecb` — isolation from normal all-features and explicit `--ignored` execution;
- `c866b0e17f3930ce9b8279f856032dceb1d638c8` — removal of timing-sensitive retry assertion;
- `eb327e1296fa5c05b0f2ad562f17b066858e8ab1` — remote/retry architecture boundaries;
- `08bacdf75952156e77bb6803ec99475718ae12fc` — remote CI commands and troubleshooting.

**Definition of Done:** Memory, JSONL и persistent SurrealDB имеют эквивалентный observable lifecycle; все canonical object kinds независимо проверяемы; embedded ownership fail-closed; remote independent writers не теряют updates; retry bounded deadline/cancellation; partial writes не публикуются; после crash видна только предыдущая или новая целая generation.

## 4. P1 — безопасность исполнения и архитектурная сопровождаемость

### P1.1. External process sandbox hardening

- [ ] `clean_environment` как production default и environment allowlist.
- [ ] Windows Job Objects.
- [ ] Linux sandbox/launcher boundary.
- [ ] Network/filesystem/CPU/memory/process limits.
- [ ] Capability report и timeout/cancellation/orphan/limit tests.

### P1.2. Декомпозиция CLI

- [ ] Оставить в `main.rs` bootstrap, parse, dispatch и top-level errors.
- [ ] Разнести command handlers и rendering.
- [ ] Добавить help/exit-code/JSON contract tests.
- [ ] Не менять пользовательскую семантику.

### P1.3. Декомпозиция runtime

- [ ] Focused process contracts/builders.
- [ ] Composition wiring вне legacy paths.
- [ ] Cancellation-aware `ProcessRunner` через composition.
- [ ] Deprecate/remove legacy registry и добавить dependency-boundary tests.

### P1.4. Installer verification

**Статус:** `[-]` — реализация готова; hosted Windows/Linux evidence и tag packaging отсутствуют.

- [x] Per-binary `SHA256SUMS`, fail-closed Linux/Windows installers и positive/tamper CI smoke.
- [x] Archive/Sigstore/provenance verification задокументирована.
- [ ] Подтвердить hosted smoke jobs и следующий version tag.

### P1.5. Operation context и protocol E2E matrix

- [ ] Deadline propagation CLI→app, daemon→app, MCP→app.
- [-] SurrealDB context-aware writes имеют deadline-bounded Busy retry.
- [ ] Добавить transport-neutral cancellation primitive в `OperationContext`.
- [ ] Stable code/retryable mapping и daemon v2/v3 compatibility.
- [ ] Cancellation queued/running writes and reads.
- [ ] Cooperative or isolated synchronous projectors.
- [ ] Rollback/recovery outside request deadline.

### P1.6. Aggregate performance budgets

- [ ] Aggregate byte budget, peak RSS и phase timings.
- [ ] 10k/50k/100k thresholds и large-file/high-relation scenarios.
- [ ] Incremental/full ratio matrix.

## 5. P2 — governance и contributor experience

### P2.1. Branch protection

- [ ] Disable force push/deletion и после bootstrap gates запретить direct push.
- [ ] Required: quality, feature matrix, store conformance, AppSec, docs.
- [ ] Stale review dismissal, up-to-date policy и secret push protection.

### P2.2. Traceability

- [ ] Issue на каждый открытый P0/P1 package.
- [ ] Acceptance criteria и verification commands в issue.
- [ ] Commit/PR links и синхронизация issue status с планом.

### P2.3. Commit/release policy

- [ ] Conventional commit lint и запрет `fix`, `changes`, `wip`.
- [ ] Automated changelog/release notes и readiness checklist.
- [ ] Version/tag/Cargo consistency.

### P2.4. Локальная воспроизводимость

- [x] Feature matrix, coverage measurement, installer/release и AppSec commands.
- [x] Embedded и remote store conformance commands/troubleshooting в `docs/development/ci.md`.
- [ ] Common CI failures для остальных workflows.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. P0.4 — получить hosted embedded/remote conformance и исправить реальные compile/format/API findings.
2. P0.4 — детерминированно воспроизвести remote transaction conflict и проверить public `Busy` mapping/backoff.
3. P1.5/P0.4 — добавить cancellation primitive и остановку retry между попытками.
4. P0.4 — объединить batch data с commit-marker publication protocol.
5. P0.4 — добавить общий backend-neutral Fact conformance и portable prepared handle.
6. P0.3 — hosted AppSec evidence и blocking Zizmor.
7. P0.1/P1.4 — hosted matrix, installer и release tag evidence.
8. P0.4 — единая generation publication и fault injection.
9. P1.1 sandbox, coverage decision, decomposition и performance budgets.
10. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный пункт:** `P0.4 Store conformance и transactional publication`.

Четвёртый срез внесён в код, но без hosted run не считается полностью подтверждённым. Следующий срез:

1. получить hosted results для embedded matrix и `surrealdb-remote` job;
2. проверить `cargo fmt`, package tests, all-features и Clippy;
3. исправить реальные compile/API/Docker findings без ослабления transaction/retry contracts;
4. после успешного remote visibility test создать детерминированный conflict scenario;
5. сверить фактическую ошибку сервера с `CoreError::Busy` и bounded backoff;
6. не переходить к commit-marker/generation redesign, пока remote job не даёт воспроизводимый зелёный evidence.

## 8. Definition of Done проекта

- Public queries изолированы committed snapshot.
- Memory, JSONL и SurrealDB проходят общий conformance suite.
- Publication crash-safe и atomic на generation boundary.
- Embedded persistent ownership fail-closed и диагностируется как retryable Busy.
- Remote concurrent writers безопасны между независимыми процессами/connections.
- Busy retries ограничены attempts, deadline и cancellation.
- Все canonical object kinds доступны проверяемому backend-neutral contract.
- External adapters bounded, cancellable и не оставляют orphan processes.
- Optional feature graphs и AppSec gates блокируют regressions.
- Coverage остаётся измеренной метрикой и становится gate только при доказанной пользе.
- Installers и release assets fail-closed проверяются, включая signed SBOM/provenance.
- CLI/runtime не остаются god modules.
- Typed errors/deadlines/cancellation эквивалентны во всех transports.
- Roadmap, plan, issues и release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-13 — четвёртый P0.4 remote/retry slice

- Добавлен opt-in crate feature `remote` с SurrealDB WebSocket protocol.
- Server-dependent remote tests помечены `ignored` и не запускаются в обычном `--all-features`.
- Dedicated CI job поднимает `surrealdb/surrealdb:v2.6.5`, ждёт `/health` и явно запускает ignored tests.
- Два независимых SDK connections выполняют 32 concurrent allocations с проверкой уникальности IDs.
- Writer публикует entity+fact batch; independent reader загружает canonical snapshot после commit.
- Context-aware writes/publication retry-ят только `Busy` с backoff 10/25/50/100 ms.
- Deadline проверяется перед попыткой; retry не спит за пределами оставшегося budget.
- Plain methods и non-retryable errors не retry-ятся.
- Удалён timing-sensitive тест с deadline 1 ms.
- P0.4 остаётся `[-]` до hosted evidence, observed remote conflict и cancellation-aware retry.

### 2026-07-13 — третий P0.4 embedded conflict/retry slice

- Active crate root переведён на public facade над transactional backend.
- Подтверждённые transient SurrealDB сообщения переводятся из `Adapter` в retryable `Busy`.
- К retryable отнесены embedded directory lock contention, `Transaction write conflict` и `Transaction retry required`.
- Data, duplicate-record, serialization и прочие statement failures остаются non-retryable.
- Добавлен persistent `surrealkv://` regression с двумя независимыми `connect()` к одному каталогу.
- Второе подключение обязано fail-closed с `CoreErrorCode::Busy` и `is_retryable() == true`.
- Embedded SurrealKV зафиксирован как single-owner process model.

### 2026-07-13 — второй P0.4 SurrealDB transaction slice

- SurrealDB `put_snapshot` переведён на один `BEGIN`/bulk `INSERT`/`COMMIT` query.
- Statement-level failures проверяются через response `check()`.
- Добавлен rollback regression с duplicate ID и clean retry того же ID.
- Counter инициализируется идемпотентно и увеличивается одним atomic `UPDATE ONLY`.
- Latest committed snapshot выбирается по numeric sequence.
- Добавлен 32-way allocation test с раздельными process-local writer gates.
- Snapshot metadata получил `prepared`; late writes после prepare запрещены.
- Явно зафиксированы ограничения: snapshot create, commit marker и metadata delete пока не входят в одну общую transaction.

### 2026-07-13 — первый P0.4 store conformance slice

- Shared conformance расширен lifecycle `SnapshotBatch → prepare → commit/abort`.
- Memory, JSONL и embedded SurrealDB подключены к одной suite.
- Добавлен dedicated Store Conformance workflow matrix.
- SurrealDB получил process-local async write gate, общий для cloned handles.
- Добавлен initial concurrent snapshot allocation test.
- Local commands и troubleshooting добавлены в CI documentation.
- Уточнено ограничение: Fact входит в batch fixture, но не имеет независимого `KnowledgeStore` query contract.
- Process-local gate явно не объявляется native transaction или cross-process lease.

### 2026-07-13 — AppSec, immutable actions и signed SBOM

- Добавлены dependency review, CodeQL v4, Gitleaks, Zizmor, immutable pins и signed CycloneDX SBOM.
- Release verify блокирует publish; permissions сужены по jobs.
- P0.3 оставлен `[-]` до hosted evidence и blocking enforcement.

### 2026-07-13 — installer integrity и coverage decision

- Coverage сохранён как measurement artifact без выдуманного threshold.
- Release archives получили per-binary checksums; installers проверяют binaries до копирования.

### 2026-07-13 — feature matrix и первичный аудит

- Добавлена optional feature matrix.
- План сверён с архитектурой, workflows, installers и крупными modules.
