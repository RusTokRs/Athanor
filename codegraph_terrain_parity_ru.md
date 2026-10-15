# Athanor → CodeGraph + Terrain: разрыв, объём работ и порядок действий

- **Репозиторий:** `RusTokRs/Athanor`, ветка `arena/76b8e17f-athanor`, база `ed33c05` (`main`)
- **Дата:** 2026-10-11 (UTC)
- **Метод:** сверка фактического кода Athanor с клонированными на дату анализа репозиториями
  `colbymchenry/codegraph` (срез `main`, 2026-10-10) и `sopaco/terrain` (срез `main`, 2026-10-09),
  плюс GitHub API по обоим репозиториям. Оценки конкурентов — по их README/коду, не по
  сторонним статьям; часть маркетинговых цифр (бенчмарки CodeGraph) самоотчётная.
- **Что это за документ:** ответ на вопрос «сколько надо сделать, чтобы выйти на уровень
  CodeGraph и Terrain». Это не аудит качества (см. `engineering-audit-2026-10-07.md`), а оценка
  разрыва в возможностях и объёма работ.

---

## 0. Как считать «уровень» и «сколько»

Три разных смысла «догнать», которые нельзя смешивать:

1. **Возможности (capabilities)** — то, что чувствует агент и пользователь: какие вопросы можно
   задать и какие ответы получить. Считается слайсами и LOC.
2. **Дистрибуция и интеграция (adoption surface)** — то, как продукт попадает на машину и
   подключается к агентам. Считается слайсами, но даёт непропорционально много adoption на слайс.
3. **Популярность (stars, community)** — **кодом не покупается вообще**. CodeGraph прошёл
   0 → 73,7k за 9 месяцев при готовом продукте и постоянном контент-конвейере (X, бенчмарки,
   site, сравнения). Ни один сценарий ниже не даёт звёзд; он даёт только право на них.

**Единицы измерения (калибровка по самому Athanor):**

- **1 слайс** = 1 PR репозиторного класса: код + тесты + документация + verification evidence
  (как Slice 8A–8I и PR #141). Наблюдаемая медиана содержательного слайса — **400–1000 LOC**
  (выборка по PR #141/#139/#100/#90/#80: 1069, 1467, 315, 264, 98).
- **Темп:** 141 PR за 115 дней (≈1,2 PR/день), из них содержательных ≈0,45–0,6/день →
  **≈3–4 содержательных слайса в неделю**. Это высокий темп (агент-assisted), но он
  размазан по лёгким слайсам; на research-heavy работах (разрешение символов JS/TS,
  tree-sitter-паки) ставлю коэффициент **1,5–2,5×**.
- **LOC** — консервативно: слайс ≈ 600 LOC, но не метрика успеха, а только санити-чек.

---

## 1. Три продукта в цифрах (2026-10-11)

| Метрика | **Athanor** | **CodeGraph** | **Terrain** |
| --- | --- | --- | --- |
| GitHub | `RusTokRs/Athanor` | `colbymchenry/codegraph` | `sopaco/terrain` |
| Создан / возраст | 2026-06-18 · 3,8 мес | 2026-01-18 · 9 мес | 2026-06-14 · 4 мес |
| Stars / forks | **0 / 1** | **73 710 / 4 747** | **257 / 26** |
| Лицензия | MIT | MIT | MIT |
| Язык реализации | Rust (129,5k LOC, 483 файла) | TS 150k + Rust-ядро 27,9k + UI 25,3k | Rust 28,3k + Svelte/TS 16,1k |
| CI / release | 13 workflows, 3 платформы, Sigstore + SBOM + provenance, `publish = false` | npm provenance, attested bundles, `upgrade` | npm CLI + unsigned DMG/EXE, `v0.9.7` |
| PR в истории | 141 (≈1,2/день, squash) | — | — |
| Тесты | 935 `#[test]`, 483 файла | ~700 тестовых файлов | — |
| Языки кода | **2** (Rust, JS/TS) + Markdown/OpenAPI/GraphQL/ops | **34** (33 full, 1 partial) | 0 (не строит граф — **встраивает CodeGraph**) |
| Framework-проекции | **3** (Next.js, Axum, Express) | **~22** + 7 роутеров с `navigates` | — |
| Хранилище | JSONL canonical + Tantivy + semantic sidecar | SQLite + FTS5 | Markdown-ассеты в `.terrain/` + registry.json |
| MCP | **10 инструментов** (`index, explain, overview, search, context, impact, change_map, rustok_architecture_context, check, capabilities`) | **1** по умолчанию (`explore`) + 7 опциональных | MCP нет: CLI-JSON API (`terrain tools …`) + skills |
| UI | статический HTML-отчёт | browser viewer (25k LOC) | **desktop app** (Tauri/Svelte) |
| LLM | нет (детерминизм — принцип) | нет | **да**: C4-доки, DeepWiki Q&A, SDD (BYO OpenAI/Ollama/LM Studio + ACP) |
| Интеграция с агентами | MCP-сервер (ручная установка) | `codegraph install` → **9 агентов** авто | `terrain env apply` → skills + AGENTS.md + CodeGraph/RTK |

**Ключевое наблюдение, которое меняет постановку задачи.** Terrain не конкурирует с CodeGraph —
он его **встраивает**: `terrain env apply` ставит CodeGraph и RTK в `~/.terrain/bin` и раздаёт
skills поверх. То есть «уровень Terrain» — это не «Terrain», а **композитный стек CodeGraph + Terrain**,
который пользователь получает сегодня одной командой. Догнать «обоих» означает заменить этот
стек одним бинарником. Отсюда и объём: ниже сценарий S3 посчитан именно как замена стека, а не
как «функции Terrain без CodeGraph».

---

## 2. Что у Athanor уже лучше — это не переделывать, это moat

Проверено по коду, не по README:

| Возможность | Athanor | CodeGraph | Terrain |
| --- | --- | --- | --- |
| Доказательство на каждом факте (path+line+status) | **да** (`Evidence`, `Ownership` в `athanor-domain`) | нет | нет (ссылки в LLM-ответах) |
| Канонические снапшоты, immutable generations, recovery journal | **да** | нет (мутабельный SQLite) | нет (markdown-файлы + freshness) |
| Диагностики/чекеры как first-class (~35 видов) | **да** | impact/blast radius | нет |
| API-контракты: registry, snapshots, diff, breaking changes | **да** (`athanor-checker-api`, 5,7k LOC) | нет | нет |
| Детерминированный docgen без LLM с gates и валидацией (5 профилей) | **да** | нет | нет (доки — LLM) |
| Store conformance harness | **да** | нет | нет |
| Операционный интеллект (CI, env, скрипты, installers, issue forms, dependabot, deny.toml) | **да** | нет | частично (env apply) |
| Signed releases: Sigstore + SBOM + provenance | **да** | npm provenance + attestation | нет (unsigned) |
| Semantic/hybrid search | **да** (PR #141, локальные hashing-эмбеддинги) | нет (FTS5) | нет (full-text по знаниям) |
| `unsafe` / `todo!` / `unwrap`-политика | 0 / 0 / запрещено | — | — |

**Вывод:** по «инженерной строгости» Athanor уже выше обоих. Разрыв — не в модели, а в
**эргономике для агента, охвате языков, продуктовой оболочке и дистрибуции**. Ниже — только это.
Ни одну строку из таблицы выше трогать не нужно, и именно она оправдывает существование Athanor
после паритета.

---

## 3. Разрыв с CodeGraph (code-intelligence слой)

| # | Ось | CodeGraph | Athanor сегодня | Что делать | Слайсы |
| --- | --- | --- | --- | --- | --- |
| C1 | **Языки** | 34 (tree-sitter-грамматики, Rust-ядро + TS-модули + per-language resolvers) | **2** программируемых (Rust через `syn`, JS/TS через `oxc`) | Generic tree-sitter engine + query-паки; целевые 8–12 языков, не 34 | 17–27 (путь B) <br> 45–60 (путь A) |
| C2 | **Разрешение символов** | calls→defs, imports→files, extends/implements, кросс-файловые цепочки, tier-синтезаторы | Rust: `Calls` (`Inferred`, 0.8, path-резолв в `linker-rust`), `Imports` (Rust+JS/TS). **JS/TS: call-графа нет вообще.** `Implements` — есть в модели, **нет ни одного продюсера** | JS/TS call graph (scope-aware на oxc), trait/interface→impl, callback/HOF-мосты, path aliases + workspace-пакеты, type hierarchy | 11–14 |
| C3 | **Ответ одним вызовом** | `explore`: verbatim source по файлам + call path + blast radius; 1 инструмент вместо меню | 10 MCP-инструментов, отдают канонические факты/JSON **без исходников** — агент всё равно идёт читать файлы | `explore`-класс: evidence-anchored source slices из снапшота + call path + impact; `node` (файл/символ с номерами строк); политика «1 инструмент по умолчанию» | 8–9 |
| C4 | **Framework routing** | ~22 фреймворка + навигация между роутерами | 3 (Next.js, Axum, Express) | +8–12 популярных (React Router, Vue/Nuxt, SvelteKit, Django, Flask/FastAPI, NestJS, Spring, Rails, Laravel, Gin, ASP.NET, Expo, Angular, TanStack) | 6–10 |
| C5 | **Свежесть / auto-sync** | OS-события, debounce 2 с, MCP-сервер сам синкает индекс «never stale» | Демон с `--watch` + file-level incremental (changed/unchanged/removed) **есть**, но MCP не демон-серверный: multi-repo/serving — roadmap | Daemon-served MCP + auto-sync + freshness-сигнал в ответах; symbol-level incremental | 6–8 |
| C6 | **Масштаб и производительность** | «fastest complete code graph» + публичные замеры | нет публичных бенчмарков, нет harness | Benchmark harness (10k+ файлов, время индекса/запроса) + оптимизации | 4–7 |
| C7 | **Установка и дистрибуция** | curl + npm + self-contained bundle + `upgrade` + 9 агентов авто + viewer | curl/ps1 + signed releases; `publish = false`; авто-конфигурации агентов нет | `ath install` (авто-wiring агентов), npm/npx, brew/scoop/winget, `ath upgrade`, crates.io | 8–11 (общий с Terrain) |
| C8 | **Визуализация** | browser viewer | статический HTML | daemon-served web viewer (граф + evidence-панель + поиск) | 5–8 |
| C9 | **Доказательство качества** | 700 тестовых файлов, per-language «fair coverage» замер на 22 репо, 7-repo agent-бенчмарк с методологией | 935 тестов, покрытие крейтов неровное, внешних замеров нет | То же, но своими словами: coverage-метрика по языкам + agent-бенчмарк с методологией (иначе цифры не поверят) | 4–7 (общий с C6) |

**Итого CodeGraph-разрыв: 52–75 слайсов** (C1–C6 + C9, где C6/C9 — один bucket; C7–C8 вынесены
в общий дистрибуционный workstream с Terrain).

> **Обновление статуса на 2026-10-15.** Таблица выше фиксирует baseline исследования на 2026-10-11,
> а не текущее состояние после PARITY-001. С тех пор исходный код получил JS/TS call graph (P1),
> `Implements`-мосты Rust/JS/TS (P2), а текущий P2A-слайс разводит `Extends` и `Implements`:
> Rust supertraits и JS/TS class/interface inheritance используют `Extends`, Rust `impl Trait for Type`
> и JS/TS class `implements` остаются `Implements`. P2A также повышает схему index-state с v46 до
> v47 для безопасной переиндексации. P2B реализовал явные named re-export chains JS/TS с
> сохранением imported/exported aliases, module edges и миграцией index-state v47→v48; полный CI
> прошёл в run `38742833180` на `5b5f8a1`. P2C добавил named resolution через source-backed
> `export *` при единственном wildcard-источнике на модуль и поднял схему до v49; модули с несколькими
> wildcard-источниками остаются unresolved. Точный GitHub Actions run `38748788007` на `49444d5`
> прошёл. P2D добавил default imports/re-exports для именованных функций и поднял схему до v50;
> anonymous expressions и class-valued defaults не становятся call targets. Run `38757025492` на
> `c105cad` прошёл на Ubuntu, macOS и Windows. P2E добавил прямые `export * as name` mappings для
> single-member calls через aliased named imports, сохранив исключение `default` для bare `export *`,
> и поднял схему до v51. Exact-commit run `38772624382` на `6aefc28` прошёл полный CI на Ubuntu, macOS
> и Windows; feature matrix, coverage, AppSec, Store Conformance, Production Gate и DOCGEN checks тоже
> прошли. Открытыми остаются receiver/type-aware dispatch, callbacks/HOF, path aliases/workspace
> packages, multi-hop namespace aliases, deep member paths и symbol-level incremental invalidation;
> обязательный scope — Rust и TypeScript/JS.
> См. актуальный source-state план в `docs/development/code-intelligence-parity-plan.md` и ledger в
> `docs/development/roadmap-status.md`; эта записка не заменяет verification matrix.

---

## 4. Разрыв с Terrain (продуктовый слой)

| # | Ось | Terrain | Athanor сегодня | Что делать | Слайсы |
| --- | --- | --- | --- | --- | --- |
| T1 | **GUI** | Desktop (Tauri + Svelte): список проектов с freshness, reader C4-доков, DeepWiki Q&A, env-экран, SDD | CLI + статический HTML | Вариант «дёшево»: daemon-served web UI (граф/доки/поиск/evidence). Вариант «паритет»: desktop-приложение | 5–8 (web) <br> 15–25 (desktop) |
| T2 | **Knowledge assets** | dual-track: `human/` (narrative C4 + Mermaid) + `agent/context.md` ≤14 KiB + `repomix.md` pack + `knowledge/` glossary | 5 docgen-профилей с публикацией и checksums — **инженерно сильнее**, но нет: freshness-скора, pack/grep-pack-микро-слоя, глоссария (Concepts — только модель, Phase 8) | Freshness-модель по ассетам + gate (<50 → down-weight), pack + `grep-pack`/`read-pack-file`, реализация Concepts/глоссария, C4-narrative профиль, i18n ассетов | 10–15 |
| T3 | **Env standardization** | `env status/plan/apply`: 4 skills, 4 AGENTS.md-фрагмента, bundled tools (CodeGraph, RTK), dependency-order, `.terrain/.gitignore`-политика | **нет ничего** (но есть read-only проекция `.codex.json`/`antigravity.json` — задел) | `ath env plan/status/apply` + каталог skills + managed AGENTS.md-сниппеты + встраиваемые инструменты + git-policy | 7–11 |
| T4 | **LLM-слой** | C4-доки, DeepWiki Q&A с цитатами, SDD 4 фазы (requirements→design→codegen→review), NDJSON-стрим, token usage, BYO endpoint | нет (детерминизм — принцип) | **Опциональный** adapter: provider port (OpenAI-compatible/Ollama), grounded Q&A с обязательными evidence-цитатами, narrative-доки отдельной проекцией, SDD-артефакты, учёт токенов | 12–20 |
| T5 | **Multi-repo** | registry + `terrain tools list-projects/read-context/…` | user-level registry есть; multi-repo serving — roadmap | Daemon multi-repo + per-project routing + NDJSON-стрим для долгих задач | 4–6 |
| T6 | **i18n** | en/zh ассеты + language setting | Concepts/LocalizedTerms в domain, реализации нет | Language-plumbing ассетов + localized terms (перевод контента — отдельный, неоцениваемый трек) | 2–4 |
| T7 | **Упаковка** | npm CLI + DMG/EXE + документация по Gatekeeper/SmartScreen | curl/ps1, npm нет, code signing нет | npm-обёртка с bundled binary, brew/scoop/winget, signing-сертификаты | 4–6 |
| T8 | **Позиционирование** | README-картинки, скриншоты, site, en/zh | README сухой, скриншотов нет, сравнения нет | Site + comparison-страница + demo/gif + примеры в блоге | 3–4 |

**Итого Terrain-разрыв: 38–60 слайсов (web-вариант) или 48–77 (desktop-вариант).**

---

## 5. Сводная смета по workstreams

| # | Workstream | Слайсы | LOC (≈) | Риск | ROI |
| --- | --- | --- | --- | --- | --- |
| 1 | Generic language engine + 8–12 языков | 17–27 | +8–14k | **высокий** (архитектурное решение) | высокий |
| 2 | Точность графа (JS/TS calls, impl, dispatch, aliases) | 11–14 | +7–11k | **высокий** (research) | высокий |
| 3 | Agent UX: `explore`/`node`/`affected`, минимум инструментов | 8–9 | +4–6k | низкий | **максимальный** |
| 4 | Свежесть и auto-sync (daemon-served MCP, freshness) | 6–8 | +3–5k | средний | высокий |
| 5 | Framework-проекции +8–12 | 6–10 | +4–7k | низкий | средний |
| 6 | Viewer (web → desktop) | 5–8 → 15–25 | +6–10k → +15–25k | средний | средний |
| 7 | Knowledge assets (freshness, pack, glossary, C4, i18n) | 10–15 | +6–10k | низкий | средний |
| 8 | Env standardization (`ath install`, skills, AGENTS.md) | 7–11 | +4–7k | низкий | **максимальный** |
| 9 | Дистрибуция (crates.io, npm, brew/scoop/winget, upgrade, site) | 8–11 | +4–6k | низкий | высокий |
| 10 | LLM-слой (опциональный) | 12–20 | +8–14k | средний (продуктовый выбор) | средний |
| 11 | Multi-repo daemon + JSON API | 4–6 | +3–5k | средний | средний |
| 12 | Бенчмарки / perf / позиционирование | 4–7 | +2–4k | низкий | высокий |

---

## 6. Три сценария

### S1 — «Агент не отличает нас от CodeGraph» (capabilities)

WS 1+2+3+4+5+12 = **52–75 слайсов**, ≈ **+30–45k LOC** (+25–35% к кодовой базе).

Календарь при наблюдаемом темпе с поправкой на research-heavy части: **6–9 месяцев**
одним потоком. С двумя параллельными потоками (языки ∥ эргономика/UX) — **4–5 месяцев**.

Definition of done: 8+ языков с измеренным «fair coverage» по каждому; `explore` возвращает
исходники + call path + blast radius за один вызов; индекс обновляется сам; MCP по умолчанию
показывает 1–2 инструмента.

### S2 — «Terrain-уровень продукта»

- **S2a (web-viewer, без desktop):** WS 6(web)+7+8+10+11 = **38–60 слайсов**, ≈ **+25–40k LOC**,
  **5–8 месяцев** (LLM-слой — независимый поток, можно ∥).
- **S2b (с desktop-приложением):** **48–77 слайсов**, ≈ **+40–60k LOC**, **8–12 месяцев**
  (нужен frontend-профиль и дизайн; это другая компетенция, чем весь текущий Athanor).

### S3 — «Заменить композитный стек CodeGraph + Terrain одним бинарником»

Объединение S1 и S2b с общими WS(8, 9, 11): **95–140 слайсов**, ≈ **+60–95k LOC** (кодовая база
примерно ×1,7–2,2 от текущей).

Календарь: одним потоком **15–24 месяца**; в 3–4 параллельных потока **6–9 месяцев** — но это
уже требует команды (сейчас bus-factor = 1) и снимает главное преимущество Athanor: единый
источник истины, который один человек держит в голове целиком.

### Чего ни один сценарий не даёт

Звёзд и community. CodeGraph = 73,7k при 9 месяцах непрерывного контента; Terrain = 257 при
готовом GUI и 4 месяцах. **Правдоподобная цель для Athanor после S1+: 1–5k звёзд за 6–12 месяцев
непрерывного маркетинга + интеграций** — и это отдельный трек, не «слайсы в репозитории».

---

## 7. Ловушки (что НЕ делать и почему)

1. **34 языка как цель.** CodeGraph держит per-language resolvers (36 файлов в `resolution/frameworks`
   + 30 языковых модулей + 27,9k LOC Rust-ядра) — это конвейер, который надо кормить вечно:
   обновления грамматик, регрессии, фикстуры, замеры покрытия. Целевой набор — **8–12 языков
   (Python, Go, Java, C#, C/C++, PHP, Ruby, Kotlin, Swift, Dart) + generic query-паки**, остальное —
   community (Phase 11 уже в roadmap Athanor). Это разница между 17–27 и 45–60 слайсами.
2. **SFC/DSL-языки** (Vue, Svelte, Astro, Liquid, Pascal, COBOL, CFML, VB.NET, Erlang) — цена высокая,
   отдача только в узких нишах. Делать по одному, под конкретный запрос, а не «для паритета».
3. **Desktop-приложение раньше adopter'ов.** У Terrain GUI есть — и 257 звёзд. GUI не покупает
   adoption; сначала WS 3+8+9 (эргономика агента, авто-wiring, дистрибуция), потом оболочка.
4. **LLM в ядре.** Детерминированный docgen без LLM — один из пяти пунктов, где Athanor объективно
   сильнее обоих конкурентов. LLM должен быть **опциональным адаптером** с обязательными
   evidence-цитатами и data-policy; иначе теряется вся разница с Terrain.
5. **Бенчмарки без методологии.** CodeGraph публикует методологию (блокировка CLI в обоих плечах,
   медиана из 4 прогонов, 7 репо, контроль контаминации). Если Athanor публикует «−60% токенов»
   без такого протокола — это репутационная мина, а не маркетинг.
6. **Налог дисциплины.** 3-платформенная exact-commit матрица на каждый слайс — moat, но при
   100+ слайсах это бутылочное горлышко. Нужен **второй, лёгкий трек** для UI/дистрибуции/
   документации (быстрые PR без полной evidence-церемонии), иначе скорость adoption никогда не
   догонит скорость продукта.
7. **«Паритет по всему» = распыление.** Athanor выигрывает там, где стек CodeGraph+Terrain слаб:
   верифицируемость, приватные репозитории, CI-гейты, API-контракты, compliance, операционный
   интеллект. Паритет нужен только по «входному билету» (агентская эргономика + дистрибуция),
   а не по каждому пункту чужого README.

---

## 8. Первые 60 дней: top-10 по ROI/слайс

| Порядок | Слайс | Почему сейчас | Из WS |
| --- | --- | --- | --- |
| 1 | `explore`-класс: evidence-anchored исходники + call path + blast radius, один вызов | Это то, из-за чего агент выбирает CodeGraph. Максимальная видимая разница при минимальной цене — канонический снапшот уже есть | WS3 |
| 2 | `ath install`: авто-wiring MCP в Claude Code / Codex / Cursor / opencode / Gemini / Copilot (+ `status`/`uninstall`) | Сейчас MCP надо подключать руками; у обоих конкурентов это одна команда. Дешёвый adoption | WS8 |
| 3 | Политика MCP-поверхности: 1–2 инструмента по умолчанию, остальные по `ATHANOR_MCP_TOOLS` | CodeGraph измерил, что одно сильное орудие снижает mis-picks и экономит контекст | WS3 |
| 4 | Daemon-served MCP + freshness-сигнал в ответах | «Индекс никогда не устарел» — половина ценности watcher'а CodeGraph; демон и watch уже написаны | WS4 |
| 5 | Generic tree-sitter engine + первые 2 языка (Python, Go) | Архитектурное решение, которое фиксирует цену всех последующих языков | WS1 |
| 6 | JS/TS call graph (scope-aware, oxc уже в зависимостях) | Без него Athanor слеп на самом массовом стеке; сейчас там только Imports | WS2 |
| 7 | Freshness-модель + gate для `context`-паков | Входной билет в логику Terrain («context < 50 не грузим»), поверх уже существующих budget-контрактов | WS4/WS7 |
| 8 | Freshness + pack/grep-pack: микро-слой для агента | Заменяет «читать файлы целиком» на «grep по паку с адресами» | WS7 |
| 9 | npm-обёртка + `npx` + crates.io (`publish = false` → политика публикации) | Снимает барьер «надо собрать Rust, чтобы попробовать» | WS9 |
| 10 | Benchmark harness + comparison-страница с методологией | Без цифр и сравнения продукт не находят и не цитируют | WS12 |

**Порядок неслучаен:** 1–4 дают «ощущение CodeGraph» почти без новых подсистем, 5–6 закрывают
фундаментальный разрыв, 7–10 конвертируют всё это в adoption.

---

## 9. Метрики «мы вышли на уровень»

| Метрика | Порог S1 | Порог S3 |
| --- | --- | --- |
| Языков с измеренным coverage | ≥8, у каждого опубликован замер | ≥12 |
| Time-to-first-answer на репо 10k файлов | <60 с (индекс) / <3 с (запрос) | то же |
| Ответ агента одним вызовом с исходниками | да (`explore`) | да |
| Инструментов MCP по умолчанию | 1–2 | 1–2 |
| Агентов, подключаемых одной командой | ≥6 | ≥9 |
| Установка без тулчейна | 1 команда (npm/brew/curl) | + desktop-инсталлятор |
| Интерактивный viewer | нет (допустимо в S1) | да |
| Публичный agent-бенчмарк с методологией | да | да |
| Freshness-модель с gate | да | да |
| Stars | 0 → 300+ | 1–5k |

---

## 10. Ответ в одну таблицу

| Сценарий | Слайсы | LOC | Календарь (1 поток) | Календарь (3–4 потока) | Что получаем |
| --- | --- | --- | --- | --- | --- |
| **S1. CodeGraph-capability** | 52–75 | +30–45k | 6–9 мес | 4–5 мес | Агент перестаёт видеть разницу; языковой паритет на 8–12 языках, не 34 |
| **S2a. Terrain-lite (web UI)** | 38–60 | +25–40k | 5–8 мес | 3–4 мес | Доки+свежесть+Q&A+env+multi-repo, без desktop |
| **S2b. Terrain-parity (desktop)** | 48–77 | +40–60k | 8–12 мес | 5–7 мес | Полный продуктовый слой Terrain |
| **S3. Заменить стек целиком** | **95–140** | **+60–95k** | **15–24 мес** | **6–9 мес** | Один бинарник вместо CodeGraph + Terrain |
| Adoption-трек (не код) | — | — | 6–12 мес непрерывно | — | 1–5k звёзд, community, цитируемость |

**Короткий вывод.** «Уровень CodeGraph» — это **52–75 слайсов** (6–9 месяцев одним потоком,
4–5 двумя), причём примерно треть из них — не новые подсистемы, а эргономика поверх уже
построенной канонической модели: самый дорогой пункт (34 языка) в полном объёме **не нужен**,
нужны 8–12 + generic-движок. «Уровень Terrain» — ещё **38–77 слайсов** и отдельная компетенция
(frontend/desktop). «Уровень обоих сразу» — **95–140 слайсов** и 6–9 месяцев уже командой,
а не одним агентом; но именно этот сценарий единственный осмысленный стратегически, потому что
Terrain уже раздаёт CodeGraph в комплекте — конкурировать нужно со стеком, а не с половиной.

Разумная последовательность: **S1 → adoption-трек (WS8/WS9/WS12 вперёд) → S2a → решать про
desktop и LLM по факту спроса.** Desktop и LLM-слой — единственные два пункта, где цену нельзя
снизить архитектурой: там покупаются чужая компетенция и чужой UX, а не строки Rust.

---

## Приложение A. Санити-чек сметы через LOC конкурентов

| Компонент | Ориентир (чужой код) | Наша оценка | Комментарий |
| --- | --- | --- | --- |
| Языковая поддержка 34 языков | 27,9k Rust-ядра + 30 TS-модулей + 36 resolvers ≈ 60–70k LOC | 8–12 языков = 8–14k LOC | Generic query-паки сжимают цену языка в 3–5× |
| Call/impact граф | ~15k LOC (`src/graph`, `src/resolution`) | 7–11k LOC | Своя цена выше: у нас есть каноническая модель + evidence на каждое ребро |
| `explore` UX | `src/mcp/explore*` + `src/context` | 4–6k LOC | У нас проще: снапшот и бюджеты уже есть |
| Viewer | 25,3k LOC UI | 6–10k (web) | React-free, read-only |
| Desktop app | 16,1k frontend + Tauri-слой | 15–25k | Другая компетенция |
| LLM-слой | `terrain-agent` (12 модулей) | 8–14k | У нас строже: цитаты и data-policy обязательны |
| Env/installers | `env-catalog` + 9 таргетов установки | 4–7k | Дешёвый и самый конверсионный код |
| Дистрибуция/упаковка | npm + curl + installers обоих | 4–6k | |

## Приложение B. Ограничения этой оценки

- Оценки конкурентов — по клонированным срезам `main` на 2026-10-10/09; их README-цифры
  (бенчмарки, coverage) самоотчётные и не проверялись нами.
- Слайсы — внутренняя единица Athanor; конвертация в календарь опирается на наблюдаемый темп
  репозитория (141 PR / 115 дней) и на предположение, что research-heavy работы дороже в
  1,5–2,5×. Это оценка порядка величины, не обещание.
- Не оценены: code signing (macOS/Windows — деньги и время на сертификаты), переводы контента,
  community-модерация, поддержка пользователей — всё это в S2b/S3 существенно.
- LOC конкурентов считались по файлам их репозиториев (`.ts`, `.rs`, `.svelte`) без учёта
  сгенерированного кода и грамматик tree-sitter.
