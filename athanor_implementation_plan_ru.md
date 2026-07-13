# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Назначение: рабочий implementation plan для последовательного улучшения Athanor  
> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `83efc933c7a5a0363114e3cda572d52521aa4905`  
> Дата актуализации: 2026-07-13  
> Статус: active implementation plan

## 0. Как пользоваться планом

План является исполнимым backlog, а не описанием желаемой архитектуры. Каждый пункт должен завершаться кодом, тестами, документацией и проверяемым результатом.

Статусы:

- `[x]` — выполнено и подтверждено кодом или воспроизводимой проверкой;
- `[-]` — частично выполнено, остаются перечисленные критерии;
- `[ ]` — не начато или не подтверждено;
- `[!]` — блокирует релиз или следующие этапы.

Правила исполнения:

1. Работать сверху вниз внутри одного приоритета, если нет явной зависимости.
2. Один коммит — одна логически завершённая часть плана.
3. Не отмечать пункт выполненным без теста или другого воспроизводимого доказательства.
4. При изменении архитектуры одновременно обновлять `docs/development/roadmap-status.md`, соответствующие architecture docs и этот план.
5. Пока разработка ведётся прямыми коммитами в `main`, перед каждым коммитом обязателен локальный эквивалент required checks. После включения branch protection работа должна перейти на PR.
6. Отсутствие push-triggered workflow run в ответе GitHub-коннектора не считается ни успехом, ни ошибкой: hosted результат подтверждается отдельным доступным run/status evidence.

## 1. Текущий baseline

На момент сверки проект уже имеет зрелую основу:

- Rust workspace с CLI `ath`, daemon `athd`, adapters, stores, search и projectors;
- canonical snapshot model и snapshot-aware query contract;
- JSONL как default backend, SurrealDB как opt-in feature;
- daemon/MCP bounded queries и typed error model;
- production, release и nightly security workflows;
- Sigstore signing, archive checksums и provenance attestations;
- strict config validation, `ath config validate` и `ath config doctor`;
- governance-файлы, Dependabot и документационные gates;
- Linux CI matrix для default, `store-surreal`, `js-ts-precision` и `--all-features`;
- source-coverage job с pinned `cargo-llvm-cov`, LCOV, JSON summary и HTML artifact.

Базовые проверки, которые должны оставаться зелёными:

```bash
cargo fmt --all -- --check
cargo test --workspace --quiet --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p ath --quiet --locked -- index .
cargo run -p ath --quiet --locked -- docs check
```

## 2. Сводный статус ранее выявленных проблем

| Пункт | Статус | Что подтверждено | Что остаётся |
| --- | --- | --- | --- |
| 3.1 Query contract по `EntityId` | `[-]` | ID-based storage queries и `EntityResolver` реализованы | Завершить conformance всех backend, включая реальный SurrealDB |
| 3.2 Snapshot isolation | `[-]` | `SnapshotSelector`, committed-only public reads и snapshot-aware cache paths реализованы | Полная contract matrix всех backend и transport paths |
| 3.3 Atomic publication | `[-]` | staging, deferred canonical commit, rollback и durable recovery journal реализованы | Единый generation pointer и backend-transactional prepare/commit |
| 3.4 Concurrent writers | `[-]` | JSONL writer lock, persistent sequence и application publication lock реализованы | SurrealDB transaction/lease model и coordinated final pointers |
| 3.5 Async process runner | `[-]` | Tokio runner, bounded streams, timeout, cancellation и Unix process groups реализованы | Windows Job Objects и полный process lifecycle contract |
| 4.1 Adapter trust/sandbox | `[-]` | manifest/executable digest and size binding, re-check before launch, clean environment profile | Network/filesystem/CPU/process-count isolation и production-safe default |
| 4.2 Operation context | `[-]` | deadline/cancellation проходят через indexing, daemon, bounded queries и MCP | E2E matrix и управляемое прерывание synchronous projector work |
| 4.3 Pipeline memory | `[-]` | extraction concurrency, byte semaphore и batch limits | Общий memory budget всех pipeline phases и benchmark thresholds |
| 4.4 Incremental invalidation | `[-]` | file-local planning и dependency closure частично реализованы | Полная regression matrix add/remove/rename/cross-file changes |
| 4.5 Storage transaction boundary | `[-]` | `SnapshotBatch`, abort/prepare и JSONL prepared promotion | Portable transaction-handle protocol и native SurrealDB implementation |
| 4.6 Runtime composition | `[-]` | `RuntimeComposition` используется основными CLI/daemon paths | Удалить legacy global registry и внедрить cancellation-aware process runner port |
| 4.7 Public API boundaries | `[-]` | Focused namespaces и explicit exports появились | Убрать legacy wildcard root surface после migration window |
| 4.8 Module decomposition | `[-]` | Pipeline, plugin и daemon logic частично разнесены | Декомпозиция оставшихся крупных CLI/runtime orchestration modules |
| 4.9 Typed errors/protocol | `[-]` | Stable core codes, daemon v3 и MCP error data реализованы | Полная E2E compatibility/retryability matrix |
| 4.10 Default build | `[x]` | SurrealDB не входит в default build | Поддерживать feature boundary в CI и release docs |
| 5.1 Hosted CI governance | `[-]` | Workflows и optional feature matrix существуют | Hosted runs, branch protection и required checks не подтверждены |
| 5.2 Coverage gate | `[-]` | Pinned `cargo-llvm-cov`, LCOV/JSON/HTML generation и artifact добавлены | Зафиксировать измеренный baseline, floor, no-regression и changed-lines gate |
| 5.3 AppSec automation | `[-]` | `cargo-deny`, nightly `cargo-audit`, unsafe policy, signing/provenance | Нет PR-level dependency review, SAST, secret scan и SBOM gate |
| 5.4 Strict config | `[x]` | Strict parsing и validate/doctor реализованы | Поддерживать contract tests |
| 5.5 Governance | `[-]` | Templates, policies и Dependabot добавлены | Commit lint, changelog/release policy и issue traceability |

## 3. P0 — release-blocking quality gates

### P0.1. CI feature matrix

**Статус:** `[-]` — реализация и документация добавлены; hosted выполнение и required-check enforcement ещё не подтверждены.

**Проблема:** default feature graph не должен быть единственным проверяемым вариантом. Optional backends и precision modes могут ломаться независимо.

**Работы:**

- [x] Добавить отдельный Linux job с `fail-fast: false`.
- [x] Проверять `--all-features`.
- [x] Проверять default/no-default boundary. У workspace и приложений пустые default features, поэтому slice `default` эквивалентен поддерживаемому `--no-default-features` graph и не дублируется отдельным matrix entry.
- [x] Проверять целевые срезы `store-surreal` и `js-ts-precision`.
- [x] Для каждого среза запускать `cargo check`, workspace tests и Clippy с `--locked`.
- [x] Зафиксировать матрицу, команды и исключения в `docs/development/ci.md`.
- [-] Оставить full-feature slice в PR/push matrix; перенести его в weekly job только при подтверждённом превышении разумного hosted budget.
- [ ] Подтвердить успешный hosted run всех четырёх slices.
- [ ] Сделать feature-matrix required check после включения branch protection.

**Реализация:**

- `8197a6f8afd295ed38dc07f90e960a7457def692` — первоначальная правка matrix job;
- `d4b437b00fad2b5c2b078f9fb1c684d666e6122f` — документированный optional feature matrix;
- `53a68dbba978c6ad6b172faf476df118ba679b9c` — документация feature matrix.

**Definition of Done:** optional feature regressions блокируют merge; documented matrix соответствует реальным Cargo features; hosted run подтверждён.

### P0.2. Coverage baseline и no-regression gate

**Статус:** `[-]` — измерительный CI job добавлен, но baseline и blocking thresholds ещё не установлены.

**Проблема:** coverage существует как продуктовый отчёт, но до текущего пакета не измерял покрытие исходного Rust-кода в CI.

**Работы:**

- [x] Добавить pinned installation `cargo-llvm-cov` версии `0.8.7`.
- [-] Генерировать machine-readable JSON summary по workspace; первый успешный hosted artifact должен быть проверен и сохранён как утверждённый baseline.
- [ ] Ввести начальный общий line floor без искусственного завышения.
- [ ] Включить no-regression относительно baseline.
- [ ] Добавить changed-lines gate для core/store/publication paths.
- [x] Хранить HTML, LCOV и JSON summary как CI artifact `rust-source-coverage`.
- [ ] Исключения оформлять только через документированную allowlist с владельцем и сроком удаления.
- [ ] Добавить локальный единый verification command после появления baseline tooling.

**Реализация:**

- `1d5516d8a612ab81a593b83a8b3352ff1bb2468f` — coverage job, pinned tool и artifacts;
- `83efc933c7a5a0363114e3cda572d52521aa4905` — локальные команды и baseline policy в CI documentation.

**Следующий шаг:** получить `coverage/summary.json` из первого успешного hosted run, определить фактический line coverage и committed baseline, после чего включить `--fail-under-lines` и no-regression comparison отдельным коммитом.

**Definition of Done:** новый код не снижает общий coverage и покрывает изменённые критические строки; gate воспроизводим локально.

### P0.3. PR-level AppSec и software supply chain

**Статус:** `[ ]`.

- [ ] Добавить dependency review для pull request.
- [ ] Добавить CodeQL или эквивалентный поддерживаемый Rust SAST с SARIF upload.
- [ ] Добавить secret scanning workflow, например gitleaks.
- [ ] Добавить workflow lint/security audit, например zizmor/actionlint.
- [ ] Пиновать third-party actions по immutable SHA; Dependabot должен обновлять эти ссылки.
- [ ] Генерировать CycloneDX или SPDX SBOM для release artifacts.
- [ ] Подписывать или attest-ить SBOM вместе с release artifacts.
- [ ] Добавить release verification job, проверяющий checksum, signature/provenance и SBOM наличие.

**Definition of Done:** уязвимые новые зависимости, секреты, критические SAST findings и небезопасные workflow changes блокируют merge/release.

### P0.4. Store conformance и transactional publication

**Статус:** `[ ]` для нового вертикального среза, при существующей частичной реализации базовых контрактов.

- [ ] Выполнить shared conformance suite на реальном SurrealDB backend в CI.
- [ ] Добавить concurrent writer tests и transaction conflict tests.
- [ ] Ввести portable prepared transaction handle для canonical snapshot.
- [ ] Реализовать native SurrealDB begin/write/prepare/commit/abort.
- [ ] Ввести единый immutable generation id для canonical/read-model/state/manifests.
- [ ] Переключать один current pointer только после успешной подготовки всех артефактов.
- [ ] Расширить fault-injection matrix на каждую фазу prepare/commit/recovery.

**Definition of Done:** после crash видна только предыдущая или новая целая generation; backend semantics эквивалентны.

## 4. P1 — безопасность исполнения и архитектурная сопровождаемость

### P1.1. External process sandbox hardening

- [ ] Сделать `clean_environment` default для production profile.
- [ ] Добавить явную allowlist переменных окружения.
- [ ] Реализовать Windows Job Objects для tree termination и resource limits.
- [ ] Определить Linux sandbox profile: namespaces/seccomp/cgroup или документированный launcher boundary.
- [ ] Ограничить network, writable filesystem, CPU, memory и process count там, где платформа позволяет.
- [ ] Добавить capability report, показывающий реально активные guarantees и деградации.
- [ ] Добавить contract tests для timeout, cancellation, orphan prevention и limit violations.

### P1.2. Декомпозиция CLI `apps/ath/src/main.rs`

- [ ] Оставить в `main.rs` только bootstrap, parse, dispatch и top-level error handling.
- [ ] Разнести command handlers по bounded modules.
- [ ] Вынести text/JSON rendering из orchestration.
- [ ] Добавить CLI snapshot/contract tests для help, exit codes и JSON schemas.
- [ ] Не менять пользовательскую семантику в рефакторинговом коммите.

**Definition of Done:** `main.rs` не более 800–1000 строк; command modules тестируются независимо.

### P1.3. Завершить декомпозицию runtime

- [ ] Перенести оставшиеся process adapter contracts и builders в focused modules.
- [ ] Вынести composition wiring из legacy compatibility paths.
- [ ] Инъектировать cancellation-aware `ProcessRunner` через composition.
- [ ] Удалить test-only дублирование production execution paths.
- [ ] Пометить legacy registry API deprecated, задать migration deadline и удалить после window.
- [ ] Добавить dependency-boundary tests, запрещающие обратные импорты.

### P1.4. Installer verification

- [ ] Включать per-binary SHA-256 manifests внутрь release archive.
- [ ] Проверять checksum `ath` и `athd` до копирования.
- [ ] Fail closed при отсутствии или несовпадении checksum.
- [ ] Добавить installer smoke tests на Linux и Windows.
- [ ] Документировать ручную проверку archive checksum, Sigstore bundle и provenance.

### P1.5. Operation context и protocol E2E matrix

- [ ] Проверить deadline propagation для CLI→app, daemon→app и MCP→app.
- [ ] Проверить stable code/retryable mapping каждой `CoreError` category.
- [ ] Проверить backward compatibility daemon v2/v3 payloads.
- [ ] Добавить cancellation tests для queued/running write jobs и read-only queries.
- [ ] Изолировать synchronous projector execution или ввести cooperative contract.
- [ ] Не применять request deadline к rollback/recovery cleanup.

### P1.6. Aggregate performance budgets

- [ ] Расширить byte budget за пределы extraction на merge, linking, checking, storage и projection.
- [ ] Добавить измерения peak RSS и phase timings в benchmark report.
- [ ] Зафиксировать regression thresholds для 10k/50k/100k files.
- [ ] Добавить large-file и high-relation-density scenarios.
- [ ] Проверять incremental/full ratio для single-file, add/remove и rename cases.

## 5. P2 — governance, release discipline и contributor experience

### P2.1. Branch protection и required checks

- [ ] Подтвердить настройки default branch через GitHub settings.
- [ ] Запретить force push и deletion `main`.
- [ ] После завершения bootstrap-gates запретить direct push.
- [ ] Сделать required: quality matrix, feature matrix, coverage, AppSec и docs check.
- [ ] Включить stale review dismissal и up-to-date branch policy при командной разработке.

### P2.2. Traceability плана

- [ ] Создать GitHub issue для каждого открытого P0/P1 work package.
- [ ] В issue указывать ID этого плана, acceptance criteria и verification commands.
- [ ] Коммиты и PR должны ссылаться на issue/work package.
- [ ] Закрытие issue автоматически отражать в status table этого документа.
- [ ] Не использовать сам документ как единственный task tracker.

### P2.3. Commit, changelog и release policy

- [ ] Добавить commitlint/conventional-commit gate.
- [ ] Запретить сообщения `fix`, `changes`, `wip` через CI policy.
- [ ] Выбрать и внедрить автоматизированный changelog/release notes flow.
- [ ] Добавить release readiness checklist.
- [ ] Проверять version/tag/Cargo metadata consistency.
- [ ] Описывать feature/license boundary SurrealDB в release notes.

### P2.4. CONTRIBUTING и локальная воспроизводимость

- [ ] Описать toolchain prerequisites и supported platforms.
- [x] Добавить feature matrix commands в `docs/development/ci.md`.
- [-] Добавить coverage commands; security и release verification commands остаются.
- [ ] Описать common CI failures и troubleshooting.
- [ ] Добавить `justfile`, `xtask` или эквивалентный единый verification entrypoint.
- [ ] Синхронизировать команды с `docs/development/ci.md` и Definition of Done.

## 6. Порядок реализации

1. **P0.1 CI feature matrix** — реализация завершена, ожидается hosted evidence и required-check enforcement.
2. **P0.2 Coverage gate** — активный пакет: получить измеренный baseline и включить no-regression.
3. **P0.3 PR-level AppSec** — закрыть вход новых security/supply-chain проблем.
4. **P1.4 Installer verification** — короткий изолированный security improvement.
5. **P0.4 Store conformance/publication** — завершить самые важные data guarantees.
6. **P1.1 Sandbox hardening** — усилить external execution boundary.
7. **P1.2 CLI decomposition** — проводить после coverage baseline.
8. **P1.3 Runtime decomposition** — проводить после coverage и protocol matrix.
9. **P1.5/P1.6 Contracts and performance** — закрепить эксплуатационные гарантии.
10. **P2 Governance/DX** — включить enforceable process после появления стабильных gates.

## 7. Текущий рабочий пакет

**Активный пункт:** `P0.2 Coverage baseline и no-regression gate`.

Текущий срез уже сделал следующее:

- добавил Linux coverage job в `.github/workflows/ci.yml`;
- использует Rust toolchain с `llvm-tools-preview`;
- устанавливает pinned `cargo-llvm-cov 0.8.7`;
- запускает workspace tests с `--locked`;
- генерирует LCOV, machine-readable JSON summary и HTML;
- сохраняет отчёты как artifact на 14 дней;
- документирует локальный эквивалент в `docs/development/ci.md`.

Следующий коммит после получения hosted artifact должен:

1. сохранить утверждённое baseline-значение и источник измерения;
2. добавить реалистичный `--fail-under-lines`;
3. реализовать no-regression comparison без зависимости от непроверенного внешнего сервиса;
4. обновить этот план и CI documentation;
5. только после этого перевести P0.2 из `[-]` в `[x]`.

## 8. Definition of Done проекта

Athanor можно считать инженерно зрелым для стабильного релизного цикла, когда одновременно выполняются условия:

- все public queries изолированы committed snapshot;
- Memory, JSONL и SurrealDB проходят единый conformance suite;
- publication crash-safe и атомарна на уровне generation;
- concurrent writers не повреждают storage;
- external adapters ограничены, отменяемы и не оставляют orphan processes;
- optional feature graphs проверяются CI;
- coverage, AppSec, dependency и supply-chain gates блокируют regressions;
- installers fail-closed проверяют поставляемые бинарники;
- CLI/runtime не содержат неуправляемых god modules;
- typed errors, deadlines и cancellation эквивалентны во всех transports;
- roadmap, implementation plan, issues и release notes не противоречат фактическому коду.

## 9. Журнал актуализаций

### 2026-07-13 — feature matrix и начало coverage gate

- P0.1 реализован в CI: Linux matrix проверяет default, `store-surreal`, `js-ts-precision` и `--all-features` через check/test/Clippy.
- Default/no-default boundary документирован: default features пусты, отдельный дублирующий matrix entry не нужен.
- P0.1 оставлен в статусе `[-]` до подтверждения hosted run и включения required check.
- Добавлен P0.2 measurement job с pinned `cargo-llvm-cov 0.8.7`.
- Coverage job создаёт `lcov.info`, `summary.json` и HTML report, публикуемые artifact `rust-source-coverage`.
- Coverage floor намеренно не выдуман до первого фактического hosted measurement.
- Активным следующим действием назначена фиксация baseline и no-regression threshold.

### 2026-07-13 — первичный расширенный аудит

- План сверён с `main` на commit `c64717cf056c2385fc3442a76202406d6b2c8bc7`.
- Подтверждено отсутствие исходной CI feature matrix и source coverage gate.
- Подтверждено отсутствие PR-level dependency review, SAST, secret scan и SBOM gate.
- Добавлены отдельные work packages для CLI decomposition, installer verification, traceability и contributor DX.
- Приоритеты перераспределены: сначала измеримые CI/security gates, затем крупные рефакторинги.
