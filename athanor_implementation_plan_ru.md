# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Назначение: рабочий implementation plan для последовательного улучшения Athanor  
> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `c64717cf056c2385fc3442a76202406d6b2c8bc7`  
> Дата актуализации: 2026-07-13  
> Статус: active implementation plan

## 0. Как пользоваться планом

План является исполнимым backlog, а не описанием желаемой архитектуры. Каждый пункт должен завершаться кодом, тестами, документацией и проверяемым результатом.

Статусы:

- `[x]` — выполнено и подтверждено кодом или CI;
- `[-]` — частично выполнено, остаются перечисленные критерии;
- `[ ]` — не начато или не подтверждено;
- `[!]` — блокирует релиз или следующие этапы.

Правила исполнения:

1. Работать сверху вниз внутри одного приоритета, если нет явной зависимости.
2. Один коммит — одна логически завершённая часть плана.
3. Не отмечать пункт выполненным без теста или другого воспроизводимого доказательства.
4. При изменении архитектуры одновременно обновлять `docs/development/roadmap-status.md`, соответствующие architecture docs и этот план.
5. Пока разработка ведётся прямыми коммитами в `main`, перед каждым коммитом обязателен локальный эквивалент required checks. После включения branch protection работа должна перейти на PR.

## 1. Текущий baseline

На момент сверки проект уже имеет зрелую основу:

- Rust workspace с CLI `ath`, daemon `athd`, adapters, stores, search и projectors;
- canonical snapshot model и snapshot-aware query contract;
- JSONL как default backend, SurrealDB как opt-in feature;
- daemon/MCP bounded queries и typed error model;
- production, release и nightly security workflows;
- Sigstore signing, archive checksums и provenance attestations;
- strict config validation, `ath config validate` и `ath config doctor`;
- governance-файлы, Dependabot и документационные gates.

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
| 5.1 Hosted CI governance | `[ ]` | Workflows существуют | Branch protection и required checks не подтверждены |
| 5.2 Coverage gate | `[ ]` | Есть продуктовая команда/reporting для coverage | Нет `cargo llvm-cov` CI no-regression gate |
| 5.3 AppSec automation | `[-]` | `cargo-deny`, nightly `cargo-audit`, unsafe policy, signing/provenance | Нет PR-level dependency review, SAST, secret scan и SBOM gate |
| 5.4 Strict config | `[x]` | Strict parsing и validate/doctor реализованы | Поддерживать contract tests |
| 5.5 Governance | `[-]` | Templates, policies и Dependabot добавлены | Commit lint, changelog/release policy и issue traceability |

## 3. P0 — release-blocking quality gates

### P0.1. CI feature matrix

**Проблема:** текущий `ci.yml` проверяет только default feature graph. Optional backends и precision modes могут ломаться незаметно.

**Работы:**

- [ ] Добавить отдельный Linux job с `fail-fast: false`.
- [ ] Проверять `--all-features`.
- [ ] Проверять `--no-default-features` там, где workspace contract это допускает.
- [ ] Проверять целевые срезы `store-surreal` и `js-ts-precision`.
- [ ] Для каждого среза запускать `cargo check`, тесты применимых crates и Clippy.
- [ ] Зафиксировать исключения и причины в `docs/development/ci.md`.
- [ ] Добавить weekly full-feature job, если PR matrix превышает разумный бюджет времени.

**Definition of Done:** optional feature regressions блокируют merge; documented matrix соответствует реальным Cargo features.

**Рекомендуемый коммит:** `ci: add feature matrix for optional backends and precision modes`.

### P0.2. Coverage baseline и no-regression gate

**Проблема:** coverage существует как продуктовый отчёт, но не как проверка исходного Rust-кода.

**Работы:**

- [ ] Добавить pinned installation `cargo-llvm-cov`.
- [ ] Сгенерировать machine-readable baseline по workspace.
- [ ] Ввести начальный общий floor без искусственного завышения.
- [ ] Включить no-regression относительно baseline.
- [ ] Добавить changed-lines gate для core/store/publication paths.
- [ ] Хранить HTML/LCOV как CI artifact.
- [ ] Исключения оформлять только через документированную allowlist с владельцем и сроком удаления.

**Definition of Done:** новый код не снижает общий coverage и покрывает изменённые критические строки; gate воспроизводим локально.

**Рекомендуемый коммит:** `ci: add llvm coverage baseline and no-regression gate`.

### P0.3. PR-level AppSec и software supply chain

**Проблема:** security workflow запускается ночью и проверяет lockfile/unsafe policy, но не анализирует изменения на границе PR.

**Работы:**

- [ ] Добавить dependency review для pull request.
- [ ] Добавить CodeQL или эквивалентный поддерживаемый Rust SAST с SARIF upload.
- [ ] Добавить secret scanning workflow, например gitleaks.
- [ ] Добавить workflow lint/security audit, например zizmor/actionlint.
- [ ] Пиновать third-party actions по immutable SHA; Dependabot должен обновлять эти ссылки.
- [ ] Генерировать CycloneDX или SPDX SBOM для release artifacts.
- [ ] Подписывать или attest-ить SBOM вместе с release artifacts.
- [ ] Добавить release verification job, проверяющий checksum, signature/provenance и SBOM наличие.

**Definition of Done:** уязвимые новые зависимости, секреты, критические SAST findings и небезопасные workflow changes блокируют merge/release.

**Рекомендуемый коммит:** `security: add pr dependency review sast secrets and sbom gates`.

### P0.4. Store conformance и transactional publication

**Проблема:** основные контракты реализованы, но guarantees пока сильнее всего подтверждены для Memory/JSONL; SurrealDB и cross-artifact publication не закрыты полностью.

**Работы:**

- [ ] Выполнить shared conformance suite на реальном SurrealDB backend в CI.
- [ ] Добавить concurrent writer tests и transaction conflict tests.
- [ ] Ввести portable prepared transaction handle для canonical snapshot.
- [ ] Реализовать native SurrealDB begin/write/prepare/commit/abort.
- [ ] Ввести единый immutable generation id для canonical/read-model/state/manifests.
- [ ] Переключать один current pointer только после успешной подготовки всех артефактов.
- [ ] Расширить fault-injection matrix на каждую фазу prepare/commit/recovery.

**Definition of Done:** после crash видна только предыдущая или новая целая generation; backend semantics эквивалентны.

**Рекомендуемый коммит:** `feat(storage): add transactional publication conformance`.

## 4. P1 — безопасность исполнения и архитектурная сопровождаемость

### P1.1. External process sandbox hardening

- [ ] Сделать `clean_environment` default для production profile.
- [ ] Добавить явную allowlist переменных окружения.
- [ ] Реализовать Windows Job Objects для tree termination и resource limits.
- [ ] Определить Linux sandbox profile: namespaces/seccomp/cgroup или документированный launcher boundary.
- [ ] Ограничить network, writable filesystem, CPU, memory и process count там, где платформа позволяет.
- [ ] Добавить capability report, показывающий реально активные guarantees и деградации.
- [ ] Добавить contract tests для timeout, cancellation, orphan prevention и limit violations.

**Definition of Done:** production profile fail-closed; отсутствие platform guarantee явно отражается в diagnostics, а не замалчивается.

### P1.2. Декомпозиция CLI `apps/ath/src/main.rs`

**Проблема:** entrypoint превышает 4 000 строк, объединяет parsing, dispatch, formatting и command-specific orchestration.

**Целевая структура:**

```text
apps/ath/src/
  main.rs
  cli.rs
  dispatch.rs
  output/
  commands/
    index.rs
    query.rs
    graph.rs
    docs.rs
    repair.rs
    adapters.rs
    project.rs
```

- [ ] Оставить в `main.rs` только bootstrap, parse, dispatch и top-level error handling.
- [ ] Разнести command handlers по bounded modules.
- [ ] Вынести text/JSON rendering из orchestration.
- [ ] Добавить CLI snapshot/contract tests для help, exit codes и JSON schemas.
- [ ] Не менять пользовательскую семантику в рефакторинговом коммите.

**Definition of Done:** `main.rs` не более 800–1000 строк; command modules тестируются независимо.

**Рекомендуемый коммит:** `refactor(cli): split monolithic command entrypoint`.

### P1.3. Завершить декомпозицию runtime

**Проблема:** `crates/athanor-app/src/runtime.rs` всё ещё превышает 1 800 строк и содержит orchestration/test surface, несмотря на выделенные submodules.

- [ ] Перенести оставшиеся process adapter contracts и builders в focused modules.
- [ ] Вынести composition wiring из legacy compatibility paths.
- [ ] Инъектировать cancellation-aware `ProcessRunner` через composition.
- [ ] Удалить test-only дублирование production execution paths.
- [ ] Пометить legacy registry API deprecated, задать migration deadline и удалить после window.
- [ ] Добавить dependency-boundary tests, запрещающие обратные импорты.

**Definition of Done:** runtime root является facade; нет скрытой process-global mutation в production paths.

### P1.4. Installer verification

**Проблема:** release подписывает и checksum-ит архив, но `install.sh`/`install.ps1` безусловно копируют извлечённые бинарники.

- [ ] Включать per-binary SHA-256 manifests внутрь release archive.
- [ ] Проверять checksum `ath` и `athd` до копирования.
- [ ] Fail closed при отсутствии или несовпадении checksum.
- [ ] Добавить installer smoke tests на Linux и Windows.
- [ ] Документировать ручную проверку archive checksum, Sigstore bundle и provenance.

**Definition of Done:** повреждённый или подменённый бинарник не устанавливается.

**Рекомендуемый коммит:** `release: verify packaged binaries in installers`.

### P1.5. Operation context и protocol E2E matrix

- [ ] Проверить deadline propagation для CLI→app, daemon→app и MCP→app.
- [ ] Проверить stable code/retryable mapping каждой `CoreError` category.
- [ ] Проверить backward compatibility daemon v2/v3 payloads.
- [ ] Добавить cancellation tests для queued/running write jobs и read-only queries.
- [ ] Изолировать synchronous projector execution или ввести cooperative contract.
- [ ] Не применять request deadline к rollback/recovery cleanup.

**Definition of Done:** transport не меняет семантику ошибок, cancellation и deadlines.

### P1.6. Aggregate performance budgets

- [ ] Расширить byte budget за пределы extraction на merge, linking, checking, storage и projection.
- [ ] Добавить измерения peak RSS и phase timings в benchmark report.
- [ ] Зафиксировать regression thresholds для 10k/50k/100k files.
- [ ] Добавить large-file и high-relation-density scenarios.
- [ ] Проверять incremental/full ratio для single-file, add/remove и rename cases.

**Definition of Done:** performance regressions выше установленного бюджета видимы и блокируют соответствующий gate.

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
- [ ] Добавить feature matrix commands.
- [ ] Добавить coverage, security, docs и release verification commands.
- [ ] Описать common CI failures и troubleshooting.
- [ ] Добавить `justfile`, `xtask` или эквивалентный единый verification entrypoint.
- [ ] Синхронизировать команды с `docs/development/ci.md` и Definition of Done.

## 6. Порядок реализации

Работа выполняется следующими вертикальными срезами:

1. **P0.1 CI feature matrix** — сначала обеспечить видимость regressions optional features.
2. **P0.2 Coverage gate** — зафиксировать baseline до крупных рефакторингов.
3. **P0.3 PR-level AppSec** — закрыть вход новых security/supply-chain проблем.
4. **P1.4 Installer verification** — короткий изолированный security improvement.
5. **P0.4 Store conformance/publication** — завершить самые важные data guarantees.
6. **P1.1 Sandbox hardening** — усилить external execution boundary.
7. **P1.2 CLI decomposition** — проводить после coverage baseline.
8. **P1.3 Runtime decomposition** — проводить после coverage и protocol matrix.
9. **P1.5/P1.6 Contracts and performance** — закрепить эксплуатационные гарантии.
10. **P2 Governance/DX** — включить enforceable process после появления стабильных gates.

## 7. Текущий рабочий пакет

**Активный следующий пункт:** `P0.1 CI feature matrix`.

Минимальный первый коммит должен:

- добавить matrix job в `.github/workflows/ci.yml`;
- не менять продуктовый код;
- использовать `--locked`;
- документировать фактически поддерживаемые feature slices;
- пройти существующие default checks;
- обновить этот раздел и `docs/development/ci.md` после подтверждения hosted run.

После его завершения активным становится `P0.2 Coverage baseline и no-regression gate`.

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

### 2026-07-13

- План сверён с `main` на commit `c64717cf056c2385fc3442a76202406d6b2c8bc7`.
- Подтверждено отсутствие CI feature matrix и source coverage gate.
- Подтверждено отсутствие PR-level dependency review, SAST, secret scan и SBOM gate.
- Добавлены отдельные work packages для CLI decomposition, installer verification, traceability и contributor DX.
- Приоритеты перераспределены: сначала измеримые CI/security gates, затем крупные рефакторинги.
- Следующим активным пунктом назначен `P0.1 CI feature matrix`.
