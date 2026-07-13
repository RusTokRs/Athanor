# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Назначение: рабочий implementation plan для последовательного улучшения Athanor  
> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `0af9a93fc610305cc63e53824eb7e939b1b0e788`  
> Дата актуализации: 2026-07-13  
> Статус: active implementation plan

## 0. Правила исполнения

Статусы:

- `[x]` — реализовано и подтверждено воспроизводимой локальной проверкой либо доступным hosted evidence;
- `[-]` — реализация частичная либо ожидает hosted evidence/enforcement;
- `[ ]` — не начато;
- `[!]` — блокирует релиз или следующий обязательный этап.

Правила:

1. Один коммит — одна логически завершённая часть.
2. Не назначать числовые thresholds без фактического измерения.
3. Не переводить hosted/platform пункт в `[x]`, пока нет доступного run/status evidence.
4. Изменения release, CI и security одновременно отражать в соответствующей документации и этом плане.
5. Пока разрешены прямые коммиты в `main`, сохранять Conventional Commits и воспроизводимые проверки. После branch protection перейти на PR.
6. Отсутствие push-triggered workflow run в ответе GitHub-коннектора не является ни успехом, ни ошибкой.
7. Security gate не считается blocking, пока он работает в report-only режиме или не включён в required checks.

## 1. Текущий baseline

Подтверждено в коде и workflow:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in SurrealDB;
- snapshot-aware query/publication contracts;
- Linux/Windows/macOS default quality matrix;
- Linux feature matrix: default, `store-surreal`, `js-ts-precision`, `--all-features`;
- measurement-only source coverage job с pinned `cargo-llvm-cov 0.8.7`;
- PR dependency review и Rust CodeQL v4 configuration;
- Gitleaks full-history scan и Zizmor workflow audit;
- immutable SHA pinning всех текущих GitHub Actions references;
- signed platform archives, archive SHA-256 и provenance attestations;
- per-binary `SHA256SUMS` и fail-closed Linux/Windows installers;
- CycloneDX 1.5 workspace SBOM, checksum, Sigstore bundles и provenance;
- release verification job, блокирующий публикацию до проверки checksum/signature/provenance/SBOM presence;
- least-privilege release permissions: signing/attestation только build/SBOM jobs, publish permission только publish job.

Базовые команды:

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
| 3.1 Query contract по `EntityId` | `[-]` | ID-based queries и `EntityResolver` | Реальный SurrealDB conformance |
| 3.2 Snapshot isolation | `[-]` | selectors, committed-only reads, cache paths | Полная backend/transport matrix |
| 3.3 Atomic publication | `[-]` | staging, rollback, recovery journal | Единый generation pointer и backend transactions |
| 3.4 Concurrent writers | `[-]` | JSONL lock/sequence/publication lock | SurrealDB conflict/lease model |
| 3.5 Async process runner | `[-]` | Tokio, bounded streams, cancellation, Unix process groups | Windows Job Objects |
| 4.1 Adapter trust/sandbox | `[-]` | digest binding, path checks, clean environment support | Resource/network/filesystem isolation |
| 4.2 Operation context | `[-]` | deadlines/cancellation in main paths | Полная E2E matrix |
| 4.3 Pipeline memory | `[-]` | extraction concurrency and byte limits | Aggregate phase budget |
| 4.4 Incremental invalidation | `[-]` | file-local planning/dependency closure | add/remove/rename/cross-file matrix |
| 4.5 Storage transaction boundary | `[-]` | `SnapshotBatch`, abort/prepare, JSONL promotion | Portable transaction handle, native SurrealDB |
| 4.6 Runtime composition | `[-]` | `RuntimeComposition` in main paths | Remove global registry, inject process runner |
| 4.7 Public API boundaries | `[-]` | focused namespaces/exports | Retire wildcard compatibility surface |
| 4.8 Module decomposition | `[-]` | partial pipeline/plugin/daemon split | CLI/runtime god modules |
| 4.9 Typed errors/protocol | `[-]` | stable codes, daemon v3, MCP error data | Full compatibility/retryability matrix |
| 4.10 Default build | `[x]` | SurrealDB excluded by default and covered by feature matrix | Maintain boundary |
| 5.1 Hosted CI governance | `[-]` | workflows and feature matrix exist | Hosted evidence, branch protection, required checks |
| 5.2 Coverage measurement | `[-]` | LCOV/JSON/HTML artifact job | Review real baseline before any gate |
| 5.3 AppSec automation | `[-]` | dependency review, Rust CodeQL v4, Gitleaks, Zizmor, immutable pins, signed SBOM, release verify job | Hosted evidence, blocking Zizmor, push protection и required checks |
| 5.4 Strict config | `[x]` | strict parsing and validate/doctor | Maintain contract tests |
| 5.5 Governance | `[-]` | templates, policies, Dependabot | Commit lint, release policy, issue traceability |

## 3. P0 — release-blocking quality and security gates

### P0.1. CI feature matrix

**Статус:** `[-]` — реализация и документация готовы; hosted run и required-check enforcement не подтверждены.

- [x] Linux job с `fail-fast: false`.
- [x] default/no-default boundary.
- [x] `store-surreal`.
- [x] `js-ts-precision`.
- [x] `--all-features`.
- [x] Для каждого slice: check/test/Clippy с `--locked`.
- [x] Документация в `docs/development/ci.md`.
- [ ] Подтвердить hosted run всех slices.
- [ ] Сделать feature matrix required check после branch protection.

Реализация: `d4b437b00fad2b5c2b078f9fb1c684d666e6122f`, `53a68dbba978c6ad6b172faf476df118ba679b9c`.

### P0.2. Coverage measurement; blocking gate отложен

**Статус:** `[-]` — измерение полезно, но жёсткий no-regression gate сейчас не обоснован.

Решение от 2026-07-13:

- coverage artifact остаётся наблюдаемой инженерной метрикой;
- percentage floor не назначается без первого успешного hosted `summary.json`;
- strict no-regression не является текущим release blocker, пока нет branch protection/PR flow;
- после крупных рефакторингов baseline должен быть измерен повторно, а решение о gate принято по фактической стабильности и стоимости CI.

- [x] Pinned `cargo-llvm-cov 0.8.7`.
- [x] LCOV, JSON summary и HTML artifact.
- [x] Локальные команды задокументированы.
- [ ] Получить и проверить первый hosted artifact.
- [ ] Решить, нужен ли общий floor или только targeted coverage для core/store/publication.
- [ ] При включении gate добавить documented allowlist с owner и сроком удаления.

Реализация: `1d5516d8a612ab81a593b83a8b3352ff1bb2468f`, `83efc933c7a5a0363114e3cda572d52521aa4905`.

### P0.3. PR-level AppSec и software supply chain

**Статус:** `[-]` — основной технический срез реализован; hosted evidence и окончательное enforcement ещё не подтверждены.

#### PR и source security

- [x] Dependency review для pull request с блокировкой новых уязвимостей уровня `moderate` и выше.
- [x] Current CodeQL v4 для Rust с `security-extended`, locked all-features build и SARIF/code-scanning upload.
- [x] Gitleaks full-history scan на push, PR, weekly schedule и manual dispatch.
- [x] Учтено, что публичный репозиторий также получает GitHub secret scanning; platform push protection остаётся отдельной настройкой.
- [ ] Подтвердить успешные hosted jobs dependency-review, CodeQL и Gitleaks.
- [ ] Подтвердить и включить GitHub push protection, если настройка доступна репозиторию.

#### Workflow security

- [x] Добавить pinned `zizmor 1.26.1` в offline strict-collection режиме.
- [-] Высокие/high-confidence findings публикуются как annotations; первый rollout намеренно использует `--no-exit-codes`.
- [x] Все текущие `uses:` в CI, AppSec, production, audit и release workflows pinned на immutable commit SHA.
- [x] Сохранить human-readable version comments рядом с SHA.
- [x] Dependabot для `github-actions` остаётся включён для reviewed updates pins.
- [x] Release permissions разделены по jobs; publish/write scope отсутствует у build, SBOM и verify.
- [ ] Проверить первый hosted Zizmor report, оформить только обоснованные исключения и удалить `--no-exit-codes`.

#### SBOM и release verification

- [x] Генерировать CycloneDX 1.5 JSON из locked all-features workspace для всех targets через pinned `cargo-cyclonedx 0.5.9`.
- [x] Упаковывать per-crate BOMs в `athanor-workspace-cyclonedx.tar.gz`.
- [x] Создавать SHA-256 для SBOM archive.
- [x] Подписывать SBOM archive и checksum через Sigstore.
- [x] Создавать GitHub provenance attestation для SBOM archive.
- [x] Добавить отдельный `verify` job для Linux archive, Windows archive и SBOM archive.
- [x] Проверять наличие assets, SHA-256, artifact/checksum Sigstore bundles и GitHub attestations до publish.
- [x] Сделать `publish` зависимым от успешного `verify`.
- [ ] Подтвердить весь build→sbom→verify→publish pipeline на следующем version tag.

#### Реализация

- `8dee7f7796e8c88f32d2cb6fabc73b303205e92e` — AppSec workflow;
- `b837dbf72e1843445281354175cb9fe714eb5099` — immutable pins в CI;
- `014ce95bb45bbcd321a25b222cce5ee902157770` — immutable pins в production gate;
- `252ddab202ef14e35f022a1792eb05aeb98efe33` — immutable pins в nightly audit;
- `6f63149b41419001b1c9fb5b849f493e41fc8e0c` — immutable pins в release workflow;
- `0ebb96c8c53105e2aece26ce3867f4c04ad6f98c` — signed CycloneDX SBOM и release verification;
- `0409468654d35aae57a8a56ef5d0365b485e3d64` — current CodeQL v4 immutable action;
- `0af9a93fc610305cc63e53824eb7e939b1b0e788` — least-privilege release job permissions;
- `c3c48a7e195e7f590a3372b36435cee6670d65ed` — AppSec CI documentation;
- `7c71e9b216a280647e0bcddf25bbdbe41a26a323` — production/release verification documentation.

**Definition of Done:** новые уязвимые зависимости, секреты, критические SAST findings и небезопасные workflow changes блокируют merge; release не публикуется без проверенного SBOM, checksums, signatures и provenance.

### P0.4. Store conformance и transactional publication

**Статус:** `[ ]` для нового вертикального среза при частично реализованной основе.

- [ ] Shared conformance suite на реальном SurrealDB в CI.
- [ ] Concurrent writer и transaction conflict tests.
- [ ] Portable prepared transaction handle.
- [ ] Native SurrealDB begin/write/prepare/commit/abort.
- [ ] Единый immutable generation id.
- [ ] Один current pointer после подготовки всех artifacts.
- [ ] Fault injection для prepare/commit/recovery.

## 4. P1 — безопасность исполнения и архитектурная сопровождаемость

### P1.1. External process sandbox hardening

- [ ] `clean_environment` как production default.
- [ ] Environment allowlist.
- [ ] Windows Job Objects.
- [ ] Linux sandbox/launcher boundary.
- [ ] Network/filesystem/CPU/memory/process limits.
- [ ] Capability report и degradation diagnostics.
- [ ] Timeout/cancellation/orphan/limit contract tests.

### P1.2. Декомпозиция CLI

- [ ] Оставить в `main.rs` bootstrap, parse, dispatch и top-level errors.
- [ ] Разнести command handlers.
- [ ] Вынести rendering.
- [ ] Help/exit-code/JSON contract tests.
- [ ] Не менять пользовательскую семантику.

### P1.3. Декомпозиция runtime

- [ ] Focused process contracts/builders.
- [ ] Composition wiring вне legacy paths.
- [ ] Cancellation-aware `ProcessRunner` через composition.
- [ ] Удалить test-only дубли production paths.
- [ ] Deprecate/remove legacy registry.
- [ ] Dependency-boundary tests.

### P1.4. Installer verification

**Статус:** `[-]` — реализация готова; Linux проверен локально, hosted Windows/Linux evidence отсутствует.

- [x] Per-binary `SHA256SUMS` внутри Linux и Windows archives.
- [x] Проверка `ath`/`athd` до создания/изменения install directory.
- [x] Fail closed при missing manifest, missing binary, malformed/duplicate/unexpected Windows entry и checksum mismatch.
- [x] Linux CI positive/tamper smoke.
- [x] Windows CI positive/tamper smoke.
- [x] Ручная archive checksum, Sigstore и provenance verification задокументирована.
- [ ] Подтвердить hosted Linux и Windows smoke jobs.
- [ ] Подтвердить release packaging на следующем version tag.

Реализация:

- `8c3a81b91d015dbbf54b0a37c69ed1d8ab6802d5` — Linux installer verification;
- `5a3056d23cee127e668672ca5eb91c457f594d90` — Windows installer verification;
- `c46ea1cfd53b7c458b7fb93df41f5c220ec42da4` — per-binary manifests;
- `709731d8827e59c27a4e08468ce26db62235d20c` — installer smoke tests;
- `8ec61590ce8c31ee7e00035dc0f42c8bf8590ca1` — production verification documentation;
- `17264662e98e2b7fe3f13208c99360d40c4e628f` — CI contract documentation.

### P1.5. Operation context и protocol E2E matrix

- [ ] Deadline propagation CLI→app, daemon→app, MCP→app.
- [ ] Stable code/retryable mapping.
- [ ] daemon v2/v3 compatibility.
- [ ] Cancellation queued/running writes and reads.
- [ ] Cooperative or isolated synchronous projectors.
- [ ] Rollback/recovery outside request deadline.

### P1.6. Aggregate performance budgets

- [ ] Aggregate byte budget for all phases.
- [ ] Peak RSS and phase timings.
- [ ] 10k/50k/100k regression thresholds.
- [ ] Large-file/high-relation scenarios.
- [ ] Incremental/full ratio matrix.

## 5. P2 — governance и contributor experience

### P2.1. Branch protection

- [ ] Confirm default branch settings.
- [ ] Disable force push/deletion.
- [ ] После bootstrap gates запретить direct push.
- [ ] Required: quality, feature matrix, AppSec, docs; coverage только если принято отдельное решение о blocking gate.
- [ ] Включить stale review dismissal и up-to-date policy.
- [ ] Включить secret push protection и проверить repository security settings.

### P2.2. Traceability

- [ ] Issue на каждый открытый P0/P1 package.
- [ ] Acceptance criteria и verification commands в issue.
- [ ] Commit/PR links на package.
- [ ] Синхронизация issue status и этого документа.

### P2.3. Commit/release policy

- [ ] Conventional commit lint.
- [ ] Запрет `fix`, `changes`, `wip`.
- [ ] Automated changelog/release notes.
- [ ] Release readiness checklist.
- [ ] Version/tag/Cargo consistency.

### P2.4. Локальная воспроизводимость

- [ ] Toolchain prerequisites/platforms.
- [x] Feature matrix commands.
- [x] Coverage measurement commands.
- [x] Installer/release verification documentation.
- [x] AppSec и local Zizmor commands.
- [ ] Common CI failures.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. P0.1 feature matrix — ожидает hosted enforcement.
2. P1.4 installer verification — реализован, ожидает hosted/tag evidence.
3. P0.3 AppSec/SBOM — основной код реализован, ожидает hosted evidence и blocking enforcement.
4. P0.4 store conformance/publication — следующий активный инженерный пакет.
5. P1.1 sandbox hardening.
6. P1.5 protocol E2E.
7. Coverage gate decision перед крупными CLI/runtime рефакторингами; не назначать threshold автоматически.
8. P1.2/P1.3 decomposition.
9. P1.6 performance budgets.
10. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активное подтверждение/enforcement:** `P0.3 PR-level AppSec и software supply chain`.

Остаётся:

1. получить hosted AppSec run и проверить dependency-review, CodeQL, Gitleaks и Zizmor;
2. разобрать high/high Zizmor findings и удалить `--no-exit-codes`, если нет необоснованного шума;
3. подтвердить release build→sbom→verify→publish на следующем version tag;
4. после branch protection сделать стабильные AppSec jobs required checks;
5. не считать отсутствие push-run в ответе коннектора доказательством успеха.

**Следующий кодовый пакет:** `P0.4 Store conformance и transactional publication`.

Первый срез P0.4 должен добавить shared backend conformance harness для Memory/JSONL и подготовить opt-in real SurrealDB CI path без изменения production semantics.

## 8. Definition of Done проекта

- Public queries изолированы committed snapshot.
- Memory, JSONL и SurrealDB проходят общий conformance suite.
- Publication crash-safe и atomic на generation boundary.
- Concurrent writers безопасны.
- External adapters bounded, cancellable и не оставляют orphan processes.
- Optional feature graphs проверяются CI.
- AppSec/dependency/supply-chain gates блокируют реальные regressions.
- Coverage используется как измеренная метрика; blocking gate включается только при доказанной пользе.
- Installers fail-closed проверяют packaged binaries.
- Release assets включают проверенный подписанный SBOM и provenance.
- CLI/runtime не остаются god modules.
- Typed errors/deadlines/cancellation эквивалентны во всех transports.
- Roadmap, plan, issues и release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-13 — AppSec, immutable actions и signed SBOM

- Добавлен AppSec workflow для dependency review, Rust CodeQL, Gitleaks и Zizmor.
- Dependency review настроен на новые уязвимости уровня `moderate` и выше.
- CodeQL обновлён до current v4 и выполняет locked all-features Rust build с `security-extended` queries.
- Gitleaks сканирует полную историю; GitHub public secret scanning учтён, push protection требует platform confirmation.
- Zizmor добавлен в report-only режиме для первого hosted rollout; blocking включается после review findings.
- Все текущие GitHub Actions references переведены на immutable commit SHA.
- Release permissions сужены по jobs: OIDC/attestation доступны только signing jobs, publish/write — только publish.
- Release workflow генерирует CycloneDX 1.5 workspace SBOM через pinned `cargo-cyclonedx 0.5.9`.
- SBOM archive и checksum подписываются, SBOM получает GitHub provenance attestation.
- Новый verify job проверяет platform archives и SBOM до публикации release.
- P0.3 оставлен `[-]` до hosted evidence, blocking Zizmor и required-check enforcement.
- Следующим кодовым пакетом назначен P0.4 store conformance/publication.

### 2026-07-13 — installer integrity и пересмотр coverage gate

- Жёсткий coverage threshold без hosted baseline и branch protection признан преждевременным.
- Coverage сохранён как measurement artifact.
- Release archives получили per-binary `SHA256SUMS`.
- Linux и Windows installers проверяют binaries до копирования и fail closed.
- Добавлены positive/tamper smoke tests для обеих платформ.

### 2026-07-13 — feature matrix и начало coverage measurement

- Добавлена optional feature matrix.
- Добавлен measurement-only coverage artifact job.
- Percentage floor не назначен без реального hosted measurement.

### 2026-07-13 — первичный расширенный аудит

- План сверён с архитектурой, workflows, installers и крупными modules.
- Выделены P0/P1/P2 work packages и порядок исполнения.
