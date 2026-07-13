# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Назначение: рабочий implementation plan для последовательного улучшения Athanor  
> Репозиторий: `RusTokRs/Athanor`  
> Базовая ветка: `main`  
> Точка сверки: `17264662e98e2b7fe3f13208c99360d40c4e628f`  
> Дата актуализации: 2026-07-13  
> Статус: active implementation plan

## 0. Правила исполнения

Статусы:

- `[x]` — реализовано и подтверждено воспроизводимой проверкой;
- `[-]` — реализация частичная либо ожидает hosted evidence/enforcement;
- `[ ]` — не начато;
- `[!]` — блокирует релиз или следующий обязательный этап.

Правила:

1. Один коммит — одна логически завершённая часть.
2. Не назначать числовые thresholds без фактического измерения.
3. Не переводить hosted/platform пункт в `[x]`, пока нет доступного run/status evidence.
4. Изменения release, CI и security одновременно отражать в соответствующей документации и этом плане.
5. Пока разрешены прямые коммиты в `main`, сохранять Conventional Commits и воспроизводимые локальные проверки. После branch protection перейти на PR.
6. Отсутствие push-triggered workflow run в ответе GitHub-коннектора не является ни успехом, ни ошибкой.

## 1. Текущий baseline

Подтверждено в коде и workflow:

- Rust workspace с `ath`, `athd`, adapters, stores, search и projectors;
- JSONL default backend и opt-in SurrealDB;
- snapshot-aware query/publication contracts;
- Linux/Windows/macOS default quality matrix;
- Linux feature matrix: default, `store-surreal`, `js-ts-precision`, `--all-features`;
- measurement-only source coverage job с pinned `cargo-llvm-cov 0.8.7`;
- signed archives, archive SHA-256 и provenance attestations;
- per-binary `SHA256SUMS` внутри release archives;
- fail-closed Linux/Windows installers, проверяющие `ath` и `athd` до копирования.

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
| 5.3 AppSec automation | `[-]` | cargo-deny, nightly audit, unsafe policy, signing/provenance | PR dependency review, SAST, secrets, workflow lint, SBOM |
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

- coverage artifact оставляется как наблюдаемая инженерная метрика;
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

**Статус:** `[ ]` — следующий активный пакет.

- [ ] Dependency review для pull request.
- [ ] CodeQL или поддерживаемый Rust SAST с SARIF.
- [ ] Secret scanning workflow.
- [ ] `zizmor`/`actionlint` для workflow.
- [ ] Third-party actions по immutable SHA.
- [ ] CycloneDX или SPDX SBOM для release artifacts.
- [ ] Attestation/signing SBOM.
- [ ] Release verification job для checksum, signature/provenance и SBOM.

**Definition of Done:** новые уязвимые зависимости, секреты, критические SAST findings и небезопасные workflow changes блокируют merge/release.

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
- `c46ea1cfd53b7c458b7fb93df41f5c220ec42da4` — per-binary manifests in release archives;
- `709731d8827e59c27a4e08468ce26db62235d20c` — Linux/Windows installer smoke tests;
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
- [ ] Stale review dismissal и up-to-date policy.

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
- [ ] Common CI failures.
- [ ] `justfile`, `xtask` или единый verification entrypoint.

## 6. Порядок реализации

1. P0.1 feature matrix — ожидает hosted enforcement.
2. P1.4 installer verification — реализован, ожидает hosted/tag evidence.
3. P0.3 PR-level AppSec — текущий активный пакет.
4. P0.4 store conformance/publication.
5. P1.1 sandbox hardening.
6. P1.5 protocol E2E.
7. Coverage gate decision перед крупными CLI/runtime рефакторингами; не назначать threshold автоматически.
8. P1.2/P1.3 decomposition.
9. P1.6 performance budgets.
10. P2 governance/DX.

## 7. Текущий рабочий пакет

**Активный пункт:** `P0.3 PR-level AppSec и software supply chain`.

Первый безопасный срез:

1. добавить PR dependency review;
2. добавить workflow lint (`zizmor` или `actionlint`);
3. не включать SAST/secret/SBOM actions без проверки permissions, supported Rust behavior и immutable pinning;
4. обновить security documentation и этот план;
5. оставить direct-main bootstrap только до появления стабильных required checks.

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
- CLI/runtime не остаются god modules.
- Typed errors/deadlines/cancellation эквивалентны во всех transports.
- Roadmap, plan, issues и release notes соответствуют коду.

## 9. Журнал актуализаций

### 2026-07-13 — installer integrity и пересмотр coverage gate

- Подтверждено, что жёсткий coverage threshold без hosted baseline и branch protection сейчас не нужен.
- Coverage сохранён как measurement artifact; решение о blocking gate отложено до фактических данных.
- Release archives теперь включают per-binary `SHA256SUMS`.
- Linux и Windows installers проверяют binaries до копирования и fail closed.
- Добавлены positive/tamper smoke tests для обеих платформ.
- Linux сценарий воспроизведён локально; hosted Windows/Linux evidence остаётся открытым.
- Документированы archive checksum, Sigstore bundle, provenance и installer verification.
- Активным пакетом назначен P0.3 AppSec.

### 2026-07-13 — feature matrix и начало coverage measurement

- Добавлена optional feature matrix.
- Добавлен measurement-only coverage artifact job.
- Percentage floor не назначен без реального hosted measurement.

### 2026-07-13 — первичный расширенный аудит

- План сверён с архитектурой, workflows, installers и крупными modules.
- Выделены P0/P1/P2 work packages и порядок исполнения.
