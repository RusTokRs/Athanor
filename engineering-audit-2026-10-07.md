# Инженерный аудит Athanor

- **Репозиторий:** `RusTokRs/Athanor` (Rust workspace, MIT)
- **Ветка/коммит аудита:** `arena/9b76dc5a-athanor`, родитель `16a9865` (`main`)
- **Дата аудита:** 2026-10-07 (UTC)
- **Аудитор:** Arena.ai Agent Mode
- **Версия продукта:** `v0.2.1` (2026-07-23) + `Unreleased` (докген-контракты); `v0.1.0` (2026-06-25)
- **Репозиторий на GitHub:** создан 2026-06-18, 0 stars / 1 fork, 22 открытых issue, последний push 2026-10-05

## 0. Методология и ограничения

Аудит **статический**: выполнялся обзор исходного кода, документации, CI-конфигураций,
GitHub metadata (issues/releases) и сравнение с внешними аналогами через веб-поиск.

**Ограничение:** в песочнице отсутствует Rust toolchain, а хосты `sh.rustup.rs` /
`static.rust-lang.org` недоступны из сети, поэтому `cargo test` / `cargo clippy` / `cargo fmt`
**не запускались**. Выводы о прохождении проверок опираются на задокументированные в
репозитории evidence (CI run-id в `docs/development/roadmap-status.md` и
`athanor_implementation_plan_ru.md`), бейдж CI в README и собственную верификационную
матрицу проекта. Этот аудит не заменяет прогон `cargo test --workspace --locked` и
`cargo clippy --workspace --all-targets --locked -- -D warnings` на машине с toolchain.

Масштаб кодовой базы (собственные замеры по рабочей копии):

| Метрика | Значение |
| --- | --- |
| Пакеты workspace | 31 (29 крейтов + `ath` CLI + `athd` daemon) |
| `.rs` файлы | 481 |
| Строки Rust-кода | ~127 900 |
| Тестовые функции (`#[test]` / `#[tokio::test]`) | 935 |
| `unsafe` блоки | **0** (запрещены политикой, `-Dunsafe_code` в security workflow) |
| `TODO` / `FIXME` / `unimplemented!` / `todo!` в коде | **0** |
| Property-based тесты (proptest/quickcheck) | **0** (нет в `Cargo.lock`) |
| Fuzz targets | нет |
| Edition / MSRV | Rust 2024 / 1.95, `resolver = "3"` |
| CI workflows | 13 (ci, appsec, security, store-conformance, production, release, verification-evidence + 6 docgen-воркфлоу) |

---

## 1. Что за продукт

Athanor — **local-first code knowledge engine** для AI-агентов и разработчиков. Идея
(README, `start.md` §1): агенту даётся «функциональная схема» репозитория до того, как он
откроет «детальную принципиальную схему» (исходники). Достигается это не пересказом кода
LLM-ом, а построением **канонической, evidence-backed модели знаний**: сущности, факты,
связи и диагностики со ссылками на исходные файлы, владельцами (ownership) и стабильными
ID; поверх неё — снапшоты, лексический поиск, impact analysis, проверки документации и
API-контрактов, детерминированная генерация документации, wiki и HTML-отчёты, MCP-транспорт
и локальный демон.

Ключевое отличие от «графового вьюера»: граф — лишь одна проекция канонической модели;
источник истины — неизменяемые снапшоты в JSONL-хранилище, а любые сгенерированные
артефакты (wiki/HTML/доки) **пересобираемы** из канонического store.

Целевая ниша по позиционированию: между **CodeGraph/GitNexus** (agent context graph, см. §7)
и **Joern/Kythe** (program analysis / verifiable claims): Athanor ближе к первой группе по
цели (дать агенту карту репозитория), но с инженерной строгостью второй (доказательства,
диагностики, версионированные контракты, conformance, AppSec-pipeline).

---

## 2. Архитектура (как устроено)

### 2.1 Слоение и порты/адаптеры

Чистая гексагональная схема, закреплённая документально (`docs/architecture/adapters.md`,
`start.md` §10):

- `athanor-domain` — каноническая модель: `Entity`, `Fact`, `Relation`, `Diagnostic`,
  `ContextPack`, `Concept`, снапшоты, `GenerationId`, `StableKey`. 24 вида сущностей
  (`EntityKind`: File…ApiEndpoint…EnvVar, CiJob, DockerService, DbMigration, Runbook…),
  11 видов фактов, 28 видов связей с обязательным `RelationStatus`
  (verified/inferred/suspected/broken/missing/conflicting/stale), 26 видов диагностик,
  4 уровня severity. Каждая сущность/факт/связь несёт `Evidence` (путь + строки +
  `EvidenceStatus`) и `Ownership`.
- `athanor-core` — порты (`ports.rs`): `KnowledgeStore`, `SourceProvider`, `Extractor`,
  `Linker`, `Checker`, `Projector`, `SearchIndex`, а также отменяемость
  (`cancellation.rs`), атомарная публикация (`atomic_publication.rs`,
  `prepared_publication.rs`), указатели `latest_pointer.rs`.
- `athanor-app` — приложение: пайплайн индексации, композиция рантайма, все read/write
  операции, daemon-протокол, docgen-подсистема (~69k строк, 477 тестов — ядро продукта).
- Адаптеры — отдельные крейты per port per format: экстракторы, линкеры, чекеры, проекторы,
  стораджи, поиск, транспорты, RusTok-адаптеры.

Композиция **явная** (`RuntimeComposition` в `composition.rs`): `ath`, `athd` и MCP-host
сами собирают рантайм; process-global installers удалены (это записано как архитектурное
решение `COMP-003` и в `docs/development/legacy-runtime-compatibility.md`). Домен и core
не знают про адаптеры — это соблюдено на практике (AGENTS.md фиксирует как правило).

### 2.2 Пайплайн индексации (`docs/architecture/pipeline.md`)

```
discovery → классификация changed/unchanged/removed → аллокация снапшота
  → экстракция (bounded concurrency, per-adapter лимиты, byte budget)
  → инкрементальный merge (carry-forward unchanged, prune по ownership, дедуп по ID)
  → линковка → чекеры → канонизация/валидация → транзакционная публикация
```

Инкрементальность реализована через `IndexStateStore` (хеши файлов, identity
предыдущего снапшота). Публикация транзакционная: journal → подготовка JSONL read-model
и state → atomic commit снапшота в Store → rollback при неудаче / durable success при
commit; отдельный atomic `IndexCurrent` pointer с собственным journal и recovery.
Предусмотрены operation deadline/cancellation на каждом pre-commit boundary; lock
публикации на проект. Есть `ath index --validate-only` (полный пайплайн без публикации).

### 2.3 Хранилище и снапшоты

- `athanor-store-jsonl` — дефолтный portable стор: неизменяемые снапшоты JSONL в
  `.athanor/store/canonical/jsonl`.
- `athanor-store-memory` — для тестов.
- `athanor-store-surrealdb` — опционально behind cargo feature `store-surreal`,
  требуется окружение.
- `athanor-store-conformance` — общий conformance-набор (контракт видимости снапшотов:
  `LatestCommitted` vs `Exact`, fail-closed на uncommitted), прогоняется отдельным
  workflow `store-conformance.yml`.
- Запросы всегда через `SnapshotSelector` (`LatestCommitted` / `Exact`) — «latest fallback
  отсутствует» для docgen-операций, fail-closed при missing/uncommitted/mismatch.

### 2.4 Read-модели и генерация

Все read-команды сериализуют **один и тот же типизированный application report** для
CLI/daemon/MCP (нет транспорт-специфичных схем). Генерация — через immutable
checksum-bound generations: `.athanor/generated/generations/<id>` + atomic
`current.json`-pointer, `UpToDate` reuse, `--force`, tamper recovery, cancellation-safe.

### 2.5 Транспорты

- **MCP stdio** (`athanor-transport-mcp`): 8 инструментов — `index`, `explain`, `search`,
  `context`, `impact`, `change_map`, `check`, `rustok_architecture_context` — все с
  hard limits, typed JSON-контрактами, cancellation и busy-ответами под насыщением
  (`MCP-004`, `MCP-007`).
- **Daemon `athd`**: per-user реестр проектов (`ath projects add`), TCP loopback или
  local-socket/named pipe, `--watch` (notify-debouncer-mini, debounce), job
  scheduler/registry/history, лимиты запросов/ответов, drain при shutdown, управление
  сервисом (`service install/uninstall/status`). Протокол v1 с authenticated handshake,
  флаг `insecure_allow_v1` явно помечен.
- Внешние процесс-адаптеры (plugins): schema-less typed request/response через
  stdin/stdout, allowlist, trust hashes, таймауты, cooperative cancellation
  (`docs/development/external-process-runner.md`, `env-athanor-adapter-trust.md`).

---

## 3. Что сделано (функциональная карта)

### 3.1 CLI `ath` — 26 команд (apps/ath/src/root_command.rs)

`init`, `index`, `bench` (синтетические бенчмарки индексации), `update`,
`validate-changed`, `context`, `explain`, `overview`, `impact`, `change-map`, `check`,
`docs` (check/drift/generate-*/inspect per profile/completeness), `config`
(validate/doctor), `api` (snapshot/diff/breaking-changes), `wiki`, `report` (html),
`generate` (координированная публикация JSONL+wiki+HTML), `graph` (запросы/экспорт графа,
incl. pagerank), `projects`, `plugins`, `repair`, `search`, `coverage`, `capabilities`,
`mcp`, `rustok`.

### 3.2 Экстракторы (crates/athanor-extractor-*)

| Экстрактор | Технология | Что извлекает |
| --- | --- | --- |
| `basic` | file inventory | канональная инвентаризация файлов (базис для completeness) |
| `rust` | `syn` (+ `syn::visit`) | символы; bounded Axum `.route()` проекция (`axum_route`) |
| `js-ts` | `tree-sitter` (javascript/typescript grammars) | символы; bounded Next.js (App/Pages Router conventions) и Express проекции (`nextjs_route`, `express_route`) |
| `markdown` | `pulldown-cmark` | страницы/секции, frontmatter, ссылки |
| `openapi` | `oas3` | endpoints/schemas/examples из OpenAPI 3 |
| `graphql` | парсер GraphQL | schema/request-response consistency |
| `operations` | YAML/TOML/PS1/shell | CI jobs/steps, env vars (incl. PowerShell `$env:`), скрипты и ScriptCommand, Docker services, миграции/таблицы БД, runbooks, `.github` composite actions, dependabot, `deny.toml` (cargo-deny политики), root `athanor.toml`, `install.sh`/`install.ps1`/`verify_release_version.py`, GitHub Issue Forms |

Базовые экстракторы намеренно framework-neutral; framework-проекции — отдельными
adapter-scoped сущностями (Slices 7A–7C), расширение (schemas/auth/middleware, route
composition, handler linking) **сознательно отложено** и запланировано evidence-driven.

### 3.3 Линкеры и чекеры

- Линкеры: `markdown` (containment, ссылки на документы, cross-source), `api`
  (code ↔ OpenAPI/GraphQL), `js-ts`, `rust` (связи вызовов/импортов в рамках bounded
  анализа).
- Чекеры: `markdown` (структура, unresolved references, duplicate IDs, drift),
  `api` (cross-protocol consistency: request/response schemas, status policy,
  auth families, permission scopes, examples; breaking-change detection между снапшотами;
  strict mode в CI-gate).

### 3.4 Поиск, проекторы, отчёты

- Поиск: `tantivy` (лексический; FTS по канонической модели).
- Проекторы: Markdown wiki, статический HTML-отчёт, общий support-слой; coordinated
  `ath generate` публикует JSONL + wiki + HTML как один immutable generation.
- Read-операции: `overview`, `context` (уровни summary/normal/deep/full + лимиты),
  `explain`, `impact` (blast radius), `change-map`, `search`, `coverage`, `capabilities`
  (completeness по адаптерам, low-confidence facts, unprocessed files), `graph`.

### 3.5 Docgen (DOCGEN-001, активная подсистема)

Evidence-backed генерация документации **без LLM-провайдера**: строгие версионированные
контракты (request/manifest/outline/context/citation/draft/validation), hard limits,
omission disclosure, SHA-256 checksum identity, cited Markdown + relation-backed Mermaid,
fail-closed валидация (pointer escape, identity drift, tampered manifests). Профили:
**architecture, module, api, operations, onboarding** (каждый: pure inventory → scoped
facts/relations/open diagnostics → immutable публикация → exact Store loading → CLI →
validated inspection) + **completeness** (read-only отчёт покрытия, versioned JSON
transport). deterministic, snapshot-exact (только `Exact` snapshot ID). Есть
self-evaluation самого Athanor на своём репозитории: **685/731 файла (9370 bps)** после
Slice 8H; Slices 8A–8H последовательно закрывали семантические пробелы (ps1, athanor.toml,
composite actions, dependabot, deny.toml, install-скрипты, issue forms), каждый подтверждён
exact CI run-id. Провайдер/LLM, по их же плану, вне скоупа.

### 3.6 RusTok-адаптеры и плагины

Три адаптера (`athanor-adapter-rustok-fba`, `-ffa`, `-page-builder`) — мост в экосистему
Rustok (внешние/зависимые адаптеры), plus registry внешних процесс-адаптеров с trust
model (`ath plugins`). Запланирован вынос Dart/Flutter анализа в отдельный репозиторий
DartScope, в Athanor — только adapter-wrapper (правило в AGENTS.md; план
`docs/development/dart-flutter-adapter-plan.md`).

### 3.7 Daemon `athd` и эксплуатация

Per-user project registry, фоновый `start`, foreground `serve`, `stop`/`status`/`doctor`,
`service install/uninstall/status` (user-level сервис), watch-режим с debounced
reindex, job history (до 1000), лимиты конкурентности/размеров, shutdown drain.
`docs/development/production.md` описывает security model, runtime paths, external-adapter
policy, signed release verification.

### 3.8 Инженерные практики и процессы (сильная сторона)

- **935 тестов**, включая интеграционные CLI-тесты (`apps/ath/tests/*` — 20 файлов),
  contract inventory тесты (версионированные JSON-схемы), store conformance, publication
  cancellation, tamper-recovery, fail-closed регрессии.
- **0 `unsafe`**, запрет enforced через `-Dunsafe_code` (UNSAFE.md + security workflow).
- **0 TODO/FIXME** в коде.
- CI matrix: Linux/Windows/macOS × Rust 1.95; `cargo-deny` (advisories/licenses/bans/
  sources), `fmt --check`, `test --locked`, `clippy -D warnings`, installer checksum smoke,
  indexing smoke, `docs check`, **feature matrix**, **source coverage** (cargo-llvm-cov).
- AppSec workflow: CodeQL, Zizmor (GitHub Actions hardening), Gitleaks.
- Release: pinned actions, least-privilege permissions, checksums + **Sigstore bundles +
  provenance attestations + CycloneDX SBOM**, tag gating (версия/чейнджлог/артефакты/
  подписи), immutable failed-publication handling (v0.2.0 сохранён как failed attempt).
- Evidence ledger: каждый пакет/слайс имеет exact source SHA + CI/AppSec/Store run-id
  (`roadmap-status.md`, `athanor_implementation_plan_ru.md`); правила «metadata ≠ execution
  evidence».
- Документация как часть поставки: coding standards (1626 строк), definition of done,
  ADR template, completeness gate для editable docs (`ath docs check`, frontmatter +
  политики в `athanor.toml`), 30+ документов по адаптерам.
- Keep a Changelog + семвер; репозиторий молодой (июнь 2026), но процессы взрослые.

---

## 4. Инженерные находки: слабые места и риски

1. **Транзитивные уязвимости зависимостей.** 7 открытых issue с RUSTSEC-адвизориями:
   `memmap2` (RUSTSEC-2026-0186), `LruCache` (RUSTSEC-2026-0253, ×2), `bincode`
   (RUSTSEC-2025-0141, unmaintained), `atomic-polyfill` (RUSTSEC-2023-0089), `event-listener`
   (RUSTSEC-2026-0221), `IterMut`/Stacked Borrows (RUSTSEC-2026-0002). Источники — прежде
   всего optional `surrealdb` (2.6.5, BUSL-1.1 exception в `deny.toml`) и `tantivy`
   (memmap2). Частично закрывается открытыми dependabot-bump'ами (surrealdb→3.2.1 #25,
   tree-sitter→0.26.11 #22, oxc→0.140 #24/#26), но стратегии «обновить vs заменить vs
   обоснованный ignore» в `deny.toml` пока нет — сейчас игнорируется только
   RUSTSEC-2026-0235 (rkyv). **Риск:** `cargo-deny check` в CI должен падать при новых
   advisory; отсутствует согласованная стратегия реагирования.
2. **Нет property-based и fuzz-тестирования.** Критичные инварианты (детерминизм merge,
   стабильность ID, канонизация, JSONL round-trip, парсеры operations-форматов) покрыты
   юнит/интеграционными тестами, но proptest/quickcheck отсутствуют в `Cargo.lock`,
   fuzz targets нет. Для парсеров YAML/PS1/issue-forms это естественное следующее усиление.
3. **Тонкое покрытие отдельных крейтов.** `athanor-linker-rust` — 1 тест,
   `projector-wiki` — 3, `projector-html` — 4, `search-tantivy` — 4,
   `athanor-domain` — 1, `apps/athd` — 0 юнит-тестов (1301 строка CLI-слоя: parsing,
   service install, клиент); daemon логика вынесена в `athanor-app` (daemon_* модулей
   много, с тестами incl. `daemon_read_dispatch_tests.rs`,
   `daemon_write_job_contract_tests.rs`), а e2e есть в `production.yml` (daemon-e2e,
   nightly soak) — но сам бинарник стоит обложить хотя бы CLI-контрактными тестами.
   `athanor-store-conformance` (545 строк, 0 `#[test]`) — это библиотека conformance-набора,
   исполняемая из тестов стораджей; acceptable, но стоит иметь прямой smoke в conformance
   workflow (он есть отдельным workflow — ок).
4. **Compatibility publication layout.** Индексация всё ещё пишет compatibility-пути
   `.athanor/generated/current/jsonl` + `.athanor/state/index-state.json`; переход на
   immutable generation-specific paths запланирован в `pipeline.md` (Target) — техдолг,
   который надо закрывать вместе с миграцией recovery journal/pointers.
5. **Semantic retrieval отсутствует.** Есть порты `EmbeddingProvider`/`VectorIndex`
   (в плане), реализован только лексический Tantivy. В 2026 году конкуренты (Semble —
   CPU-эмбеддинги Model2Vec; Claude Context — векторный MCP) уже дают semantic search;
   для Athanor это естественный следующий read-model.
6. **Нет LSP-интеграции.** Точность символов ограничена статическими экстракторами
   (syn/tree-sitter); Serena доказывает ценность LSP-backed symbol tools. Возможный
   адаптер, а не core-зависимость.
7. **Нет интерактивной визуализации.** `graph` экспорт и статический HTML есть; у
   CodeGraph — browser viewer, у GitNexus — графовый UI. Для «human review» ценности
   продукта это заметный пробел.
8. **i18n/Concepts — только модель.** `Concept`/`LocalizedTerms`/`TranslationOf` есть в
   domain, реализация (Phase 8) впереди; RU-доки (start.md, план) сосуществуют с EN-доками
   — осознанно, но для внешней аудитории стоит либо двуязычная карта, либо перевод ключевых
   доков.
9. **Daemon multi-repo/job orchestration — рудиментарный.** Registry есть, но
   «production multi-process storage, daemon job orchestration, daemon-served
   multi-repository workflows» явно названы roadmap (README Current Status).
10. **Молодость и bus-factor.** 0 stars, 1 fork, вся история — squash-коммиты от
    github-actions[bot] в этом клоне; по GitHub — молодой проект (с 2026-06-18), 22 открытых
    issue, из них ~10 dependabot-bump'ов без automerge. Зависимость от одного основного
    автора (по CHANGELOG/PR-истории) — риск для устойчивости.
11. **SurrealDB — тяжёлый optional dependency.** BUSL-1.1 exception в `deny.toml`,
    feature-gated, но тянет транзитивные advisories (см. п.1); для OSS-проекта
    лицензионная чистота важна — рассмотреть вытеснение или уход от default-ски.
12. **Docgen completeness остаток.** 685/731 (9370 bps): оставшиеся YAML-fixtures
    (issue forms/OpenAPI fixture) сознательно вне scope — «не гнаться за coverage ради
    coverage» — правильная позиция, но зафиксировать остаточный долг в issue стоит.

Позитивные практики, которые стоит **сохранить**: evidence-обязательность, fail-closed
публикации, версионированные JSON-контракты с inventory-тестами, conformance-suite для
сторов, AppSec trio (CodeQL/Zizmor/Gitleaks), signed releases + SBOM, evidence ledger с
run-id, «metadata ≠ execution evidence».

---

## 5. Что необходимо сделать (по дорожной карте проекта и аудита)

### 5.1 Ближайшие (P0 — hygiene и закрытие долгов)

1. **Закрыть RUSTSEC-адвизории** (issues #83–#88): смержить/doganать bump'ы
   surrealdb→3.x, tantivy, tree-sitter, oxc (#22–#26); для неисправимых upstream —
   либо замена зависимости, либо явный `ignore` в `deny.toml` с комментарием и датой
   пересмотра. Включить automerge для dependabot patch/minor (конфиг есть,
   `automerge` не настроен — 10 открытых bump-issue).
2. **Довести focused verification для Slices 8F–8H** на одном exact commit и выбрать
   следующий bounded semantic gap из post-gate artifact (это их собственный «next step»;
   issue #137 — Slice 8I: проекция MCP-конфигурации как agent-tool; #136 — Windows
   process fixtures hardening).
3. **Юнит-тесты для `athd`** (CLI parsing, service install paths, doctor) и смоук
   conformance-набора.
4. **Поднять покрытие** linker-rust, projector-wiki/html, search-tantivy, domain;
   добавить **proptest** для merge/canonicalization/stable-ID и round-trip JSONL;
   рассмотреть `cargo-fuzz` для operations-парсеров (YAML/PS1/issue forms/TOML).
5. **Зафиксировать остаточный docgen debt** (YAML-fixtures, issue forms) как issue с
   явным «out of scope» rationale — чтобы completeness gate не блокировал PR без причины.

### 5.2 Среднесрочные (P1 — из roadmap проекта, фазы 6–9)

6. **Убрать compatibility publication layout** (переход на immutable generation paths +
   миграция journal/pointer recovery) — уже в Target Architecture.
7. **Semantic search**: реализовать порты `EmbeddingProvider`/`VectorIndex` локальными
   CPU-эмбеддингами (по опыту Semble), как read-model поверх канонических снапшотов;
   hybrid lexical+semantic в `ath search` / MCP `search`.
8. **Daemon**: multi-repository orchestration, job scheduling/backpressure по плану
   (Phase 7), горячий кэш для read-команд.
9. **Framework-проекции углубить** evidence-driven: Next.js/Axum/Express schemas/auth/
   middleware, route composition, handler linking (Slices 7D+).
10. **Dart/Flutter**: вынести парсер в DartScope (отдельный репозиторий), в Athanor —
    wrapper-адаптер (уже спланировано, `dart-flutter-adapter-plan.md`).
11. **Интерактивный viewer** (HTML/graph UI) поверх `graph` export / HTML report.
12. **LSP-адаптер** (symbol precision à la Serena) как опциональный порт, не в core.

### 5.3 Долгосрочные (P2 — фазы 8–10 и продукт)

13. **i18n/Concepts** (Phase 8): glossary, aliases, translation drift diagnostics,
    localized wiki — модель в domain уже готова.
14. **Postgres/SeaORM store + Rustok module** (Phase 10): `ath sync postgres`, JSONB-схема,
    дашборды в Rustok (Diagnostics, API Consistency, Breaking Changes, Docs Drift…).
15. **Agent-native режим** (`ath agent *` из `start.md` §28) поверх daemon + MCP.
16. **Community/рост**: `publish = false` сейчас у всего workspace — для adoption
    рассмотреть публикацию library crates (domain/core/ports) на crates.io; демо-материалы,
    comparison-страница с CodeGraph/GitNexus/Serena; переводы ключевых доков.

---

## 6. Сравнение с похожими решениями

Классификация конкурентов (актуально на 2026-10): **local agent-context graphs**
(CodeGraph, GitNexus, codebase-memory-mcp, code-review-graph, Graphify, repo-map экосистема),
**LSP-based symbol tools** (Serena), **repo map внутри чат-агентов** (Aider repomap,
OpenCode/Cline встроенные индексаторы), **hosted wiki/Q&A** (DeepWiki), **enterprise code
intelligence** (Sourcegraph/Cody, Greptile, Augment), **program-analysis knowledge graphs**
(Joern, Kythe), **context packers** (Repomix, gitingest).

| Критерий | **Athanor** | CodeGraph (colbymchenry) | GitNexus | Serena | Joern | Sourcegraph/Cody | DeepWiki / Aider repomap |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Stars (2026-10) | 0 | ~70k [1][2][3] | ~43–47k [3][4] | ~26k [4] | ~10k+ (OSS, JVM) [9] | enterprise | DeepWiki hosted (Cognition) [2]; Aider ~44k [4] |
| Лицензия | MIT | MIT [1][2] | PolyForm NC [3][4] | open source (спорно MIT/GPL по источникам [2][4]) | MIT [9] | proprietary | DeepWiki free для public repos [2]; Aider Apache-2.0 [4] |
| Модель данных | канонические снапшоты (Entity/Fact/Relation/Diagnostic + evidence + ownership), JSONL store | SQLite knowledge graph (tree-sitter) [1][3] | LadybugDB graph, WASM [3][4] | без persistent graph — поверх LSP [4] | Code Property Graph [9] | hosted index, multi-repo | wiki (LLM) / tree-sitter repo map |
| Локальность | fully local-first | fully local [1][2] | local + optional remote [4] | local [4] | local | hosted/enterprise | DeepWiki — cloud; Aider — local map |
| Evidence/provenance | **да, встроено** (путь+строки+status на каждый факт) | нет (graph + FTS5) [3] | impact queries [3][4] | символы из LSP | CPG + dataflow | code intel | нет / эвристика ранжирования |
| Диагностики/чекеры | **встроены**: docs drift, duplicate IDs, unresolved refs, API consistency, breaking changes | impact/blast radius [3] | impact, process flows [4] | нет (навигация/рефакторинг) | security queries (CPG DSL) | code intel + Cody review | нет |
| API-контракты | **registry, snapshots, diff, breaking-change detection** | нет | нет | нет | нет | code intel | нет |
| Docgen | **детерминированный, без LLM, с gates и валидацией** | нет | LLM wiki (опц.) [4] | нет | нет | нет | DeepWiki — LLM wiki [2] |
| Семантический поиск | пока нет (лексический Tantivy) | FTS5 [3] | graph queries [4] | нет | query DSL | semantic+graph | BM25/vector у части аналогов (Semble [3], Claude Context [3]) |
| MCP | 8 bounded tools | MCP [1][2] | MCP, deep integration [4] | MCP — ядро продукта [4] | нет (CLI/REPL) | Cody в IDE/вебе | — |
| Watch/incremental | да (daemon watch + инкрементальный merge) | file watcher, 2s debounce [3] | incremental + watch [2] | live LSP | batch | hosted sync | — |
| UI | статический HTML report | browser viewer [4] | graph UI [4] | IDE-like tools | desktop/IDE | web UI | web wiki |
| Зрелость | молодой (06-2026), но CI/AppSec/release mature | очень высокая adoption | высокая | высокая | зрелый (с 2019) | enterprise | hosted/зрелый |

Источники: [1](https://www.opensourcealternatives.to/item/codegraph), [2](https://ai-tldr.dev/tools/codegraph/),
[3](https://www.knolli.ai/post/graphify-alternatives), [4](https://rywalker.com/research/code-intelligence-tools),
[9](https://github.com/joernio/joern); см. также [5](https://ssojet.com/blog/ai-codebase-understanding-tools),
[6](https://repowise.dev/blog/mcp/repository-intelligence-coding-agents-buyers-guide),
[7](https://github.com/topics/repo-map), [8](https://www.opensourcealternatives.to/blog/best-open-source-ai-coding-assistants).

### 6.1 Разбор по классам

- **vs CodeGraph / GitNexus / codebase-memory-mcp / code-review-graph (прямые конкуренты).**
  У Athanor более строгая модель (канонические снапшоты + evidence + ownership +
  диагностики + версионированные контракты + conformance + signed releases), шире
  «не-кодовая» поверхность (операционные файлы, CI, env, install-скрипты, issue forms,
  API-контракты, docgen без LLM). У CodeGraph — массовая adoption, browser viewer,
  FTS5, 30+ языков через tree-sitter; у GitNexus — WASM, LadybugDB, LLM wiki. **Чего не
  хватает Athanor:** semantic search, LSP-точность, UI-визуализация, breadth языков
  (сейчас Rust/JS-TS/Markdown/OpenAPI/GraphQL + ops-форматы), community.
- **vs Serena.** Serena — символ-уровня точность через LSP и symbol-editing; Athanor —
  репозиторий-уровня модель. Не конкуренты, а кандидаты на интеграцию (LSP-адаптер в
  Athanor дал бы Serena-подобную точность внутри канонической модели).
- **vs Joern / Kythe.** Те — program analysis / verifiable claims (глубокий dataflow,
  security research). Athanor не пытается в dataflow- точность; его сила — operating the
  knowledge loop (индексация → проверки → docgen → публикация) и инженерная дисциплина.
  Kythe (Google) — ближайший по духу «verifiable claims with provenance», но проект
  фактически dormant; Athanor его догоняет по evidence-подходу в local-first нише.
- **vs Sourcegraph/Cody, Greptile, Augment.** Enterprise multi-repo code intel/search с
  hosted/cloud доставкой. Athanor — локальный single-repo baseline с понятной нишей
  «приватность + агентный доступ»; multi-repo/hosted — их территория и дорожная карта
  Athanor (daemon multi-repo workflows — roadmap).
- **vs DeepWiki / Aider repomap.** DeepWiki — hosted LLM wiki «из коробки» для public
  repos; Athanor — локальный, детерминированный, без LLM и без утечки кода, с валидацией
  и gates — дополняет, а не копирует. Aider repomap — tree-sitter карта внутри чат-агента;
  Athanor даёт тот же «карта перед кодом», но как инженерный артефакт со снапшотами и
  диагностиками, а не как prompt-эвристику.

### 6.2 Уникальные сильные стороны Athanor (дифференциаторы)

1. Evidence-обязательность и ownership в канонической модели (у «быстрых» конкурентов —
   нет).
2. Диагностический слой как продукт (docs drift/completeness gates, API consistency,
   breaking changes, strict CI mode) — ближе к CodeScene/Greptile по ценности, но локально
   и бесплатно.
3. Детерминированный docgen без LLM-провайдера с контрактами, валидацией и immutable
   публикацией (уникально в классе; DeepWiki — ровно противоположная стратегия).
4. Operations/config-интеллект (CI, env, scripts, Docker, migrations, installers,
   dependabot, deny.toml, issue forms) — немногие конкуренты вообще смотрят за пределы
   исходников.
5. Инженерная дисциплина: conformance-suite для стораджей, версионированные JSON
   контракты с inventory-тестами, fail-closed публикация, signed releases + SBOM,
   evidence ledger с CI run-id, 0 unsafe.

### 6.3 Где Athanor уступает (честно)

Semantic search (нет), LSP-точность (нет), визуальный UI (статический HTML только),
количество языков (5 семейств + ops), multi-repo/hosted режим (нет), community/ecosystem
(0 stars), размер команды/скорость экосистемы (молодой проект).

---

## 7. Выводы

**Уровень инженерной зрелости — высокий для возраста проекта (3.5 месяца), и выдающийся по
дисциплине:** adapter-first архитектура выдержана, каноническая модель с evidence внятна,
публикация транзакционна и fail-closed, контракты версионированы, CI/AppSec/release
конвейеры взрослые (cross-platform matrix, cargo-deny, CodeQL/Zizmor/Gitleaks, coverage,
feature matrix, signed releases + SBOM + provenance), тестовая база большая (935 тестов),
`unsafe` запрещён enforced, а статус работ ведётся с exact CI evidence.

**Функционально сделано:** полный локальный knowledge loop — индексация (инкрементальная)
→ экстракция (Rust/JS-TS/MD/OpenAPI/GraphQL/ops) → линковка → чекеры → канонические
снапшоты (JSONL/memory/SurrealDB) → read-команды (overview/context/explain/impact/
change-map/search/coverage/capabilities/graph) → проекции (wiki/HTML/JSONL generations) →
docgen-профили (5 + completeness) → API-contract registry с diff/breaking-changes →
MCP (8 tools) → daemon с watch и service lifecycle → плагины внешних адаптеров →
repair/config/projects/bench.

**Главные долги:** (а) транзитивные RUSTSEC в зависимостях (surrealdb/tantivy) без
зафиксированной стратегии; (б) нет property/fuzz тестирования и тонкое покрытие у
нескольких крейтов (linker-rust, projectors, athd CLI); (в) compatibility publication
layout — запланированный техдолг; (г) отсутствуют semantic search, LSP-адаптер, UI,
multi-repo daemon orchestration, i18n — всё это уже в их roadmap, часть — P1.

**Позиция на рынке:** Athanor не «ещё один repo-map», а **engineering-grade local knowledge
engine**: ближе к CodeGraph/GitNexus по цели (дать агенту карту репозитория, сократить
токены), но с discipline уровня Joern/Kythe-подхода к доказательствам, плюс уникальные
фичи (операционный интеллект, API-contract registry, детерминированный docgen без LLM,
gates в CI). Ниша — приватные репозитории и команды, где code intelligence должен быть
локальным, проверяемым и воспроизводимым. Главный риск — adoption (0 stars) и конкуренция
с быстрорастущими MIT-игроками; mitigations — publish library crates, демо, UI, semantic
search, LSP-адаптер.

## 8. Рекомендуемый top-10 порядок работ

1. Закрыть RUSTSEC (bump surrealdb/tantivy/tree-sitter/oxc; automerge dependabot;
   задокументировать ignore-стратегию).
2. Focused verification Slices 8F–8H + выбор следующего gap из artifact (issue #137/#136).
3. Proptest для merge/canonicalization/stable-ID/JSONL round-trip; fuzz для ops-парсеров.
4. Юнит-тесты `athd`; поднять покрытие linker-rust/projectors/search/domain.
5. Миграция off compatibility publication layout (immutable generations).
6. Semantic search (локальные CPU-эмбеддинги) как read-model; hybrid search.
7. Framework-проекции 7D+ (schemas/auth/middleware, route composition) evidence-driven.
8. LSP-адаптер (symbol precision) + интерактивный graph/HTML viewer.
9. DartScope вынос + wrapper-адаптер; i18n/Concepts (Phase 8) после quality gates.
10. Adoption: publish crates (domain/core), демо-запись, comparison-док, переводы ключевых
    доков, multi-repo daemon orchestration (Phase 7).

---

## Приложение A. Метрики по крейтам (строки / тесты)

| Крейт/приложение | Строки | Тесты |
| --- | --- | --- |
| athanor-app | 69 133 | 477 |
| apps/ath | 9 957 | 81 |
| athanor-checker-api | 5 743 | 35 |
| athanor-extractor-operations | 5 569 | 35 |
| athanor-extractor-graphql | 3 585 | 27 |
| athanor-adapter-rustok-fba | 3 297 | 18 |
| athanor-store-surrealdb | 2 879 | 26 |
| athanor-transport-mcp | 2 527 | 40 |
| athanor-extractor-js-ts | 2 517 | 23 |
| athanor-store-jsonl | 2 380 | 18 |
| athanor-adapter-rustok-page-builder | 2 114 | 5 |
| athanor-adapter-rustok-ffa | 1 852 | 19 |
| athanor-extractor-openapi | 1 655 | 14 |
| athanor-core | 1 623 | 15 |
| athanor-extractor-rust | 1 458 | 12 |
| athanor-linker-api | 1 440 | 14 |
| apps/athd | 1 301 | 0 |
| athanor-projector-html | 1 247 | 4 |
| athanor-store-memory | 1 002 | 10 |
| athanor-extractor-markdown | 959 | 17 |
| athanor-checker-markdown | 639 | 7 |
| athanor-projector-support | 618 | 10 |
| athanor-runtime-defaults | 570 | 6 |
| athanor-store-conformance | 545 | 0 |
| athanor-linker-markdown | 545 | 5 |
| athanor-projector-wiki | 533 | 3 |
| athanor-linker-rust | 516 | 1 |
| athanor-search-tantivy | 483 | 4 |
| athanor-domain | 378 | 1 |
| athanor-source-fs | 349 | 5 |
| athanor-linker-js-ts | 323 | 2 |
| athanor-extractor-basic | 151 | 1 |

## Приложение B. CI/CD поверхность (13 workflows)

`ci.yml` (cargo-deny, fmt, test `--locked`, clippy `-D warnings`, installer checksum smoke,
indexing smoke, docs check, feature matrix, llvm-cov coverage; matrix
Linux/Windows/macOS × Rust 1.95), `appsec.yml` (CodeQL, Zizmor, Gitleaks), `security.yml`
(`-Dunsafe_code` и проверки), `store-conformance.yml`, `production.yml` (daemon e2e,
nightly soak), `release.yml` (pinned actions, least-privilege, Sigstore + provenance +
CycloneDX SBOM, tag gating), `verification-evidence.yml`, + 6 docgen-воркфлоу
(self-evaluation, completeness 6A–6B, framework 7A–7C, onboarding 5A–5C, rustok
evaluation/probe).

## Приложение C. Ограничения аудита

Статический аудит без прогона `cargo test/clippy/fmt` (нет Rust toolchain в песочнице, нет
доступа к rustup-хостам). Состояние «verified» по пакетам принято по задокументированным
в репозитории CI run-id; рыночные данные (stars, описания аналогов) — по публичным
источникам на 2026-10-07, с оговоркой, что часть из них — агрегаторы и могут лагать.
