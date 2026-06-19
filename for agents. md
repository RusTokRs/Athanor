1. Первый проход — архитектура и зависимости.
2. Второй проход — data model, инкрементальность, generated wiki.
3. Третий проход — security, Rustok compatibility, тесты и future extensibility.


Ты выступаешь как независимый архитектурный ревизор Rust-проекта Athanor.

Контекст продукта:

Athanor — независимый Rust-based Code Knowledge Engine для AI-агентов и разработчиков. Он строит инкрементальную, проверяемую и объяснимую базу знаний проекта, связывая код, документацию, API, скрипты, конфиги, тесты, зависимости, БД/миграции, Git-историю, доменные понятия и несоответствия.

Ключевые архитектурные принципы Athanor:

1. Ядро не должно зависеть от конкретных реализаций:

   * SurrealDB;
   * Postgres;
   * SeaORM;
   * Axum;
   * Loco;
   * MCP;
   * Tantivy;
   * vector store;
   * HTML/wiki;
   * Rustok.

2. Источник истины:

   * Entity;
   * Fact;
   * Relation;
   * Evidence;
   * Diagnostic;
   * Snapshot;
   * ContextPack;
   * Concept;
   * ManualKnowledge.

3. Всё внешнее должно быть адаптером:

   * KnowledgeStore;
   * SearchIndex;
   * VectorIndex;
   * EmbeddingProvider;
   * SourceProvider;
   * Extractor;
   * Linker;
   * Checker;
   * Projector;
   * Transport;
   * AgentInterface.

4. Standalone-режим:

   * SurrealDB embedded;
   * Tantivy;
   * local vectors;
   * Markdown/JSONL generated wiki;
   * CLI/daemon/socket;
   * single binary.

5. Rustok module-режим:

   * Postgres;
   * SeaORM;
   * Loco/Axum adapter;
   * Rustok permissions;
   * без SurrealDB в зависимостях модуля.

6. MCP не является ядром. MCP — только внешний адаптер.

7. Generated wiki не является источником истины. Она должна полностью пересобираться из canonical store.

8. Agent-native доступ должен работать через:

   * CLI;
   * local socket;
   * file inbox/outbox;
   * generated wiki;
   * JSONL;
   * MCP только как дополнительный адаптер.

9. Community extensions должны зависеть от Athanor, а не от Rustok. Rustok marketplace не входит в текущий roadmap.

Твоя задача — провести глубокий архитектурный аудит текущего MVP.

Не ограничивайся поверхностным code review. Ищи именно скрытые архитектурные ошибки, которые сейчас могут выглядеть незначительными, но через 3–6 месяцев заставят переписывать ядро.

Проверь следующие направления:

## 1. Чистота ядра

Проверь, не протекли ли в `athanor-domain`, `athanor-core`, `athanor-app` зависимости или типы из:

* SurrealDB;
* SeaORM;
* Postgres;
* Axum;
* Loco;
* MCP;
* Tantivy;
* конкретных языковых парсеров;
* конкретных projectors;
* конкретного формата wiki.

Найди все места, где ядро знает о реализации, хотя должно знать только trait/port.

Для каждого нарушения укажи:

* файл;
* строку или участок кода;
* почему это архитектурный риск;
* как исправить;
* нужно ли исправлять сейчас или можно отложить.

## 2. Проверка ports/adapters

Проверь, достаточно ли хорошо выделены контракты:

* KnowledgeStore;
* SearchIndex;
* VectorIndex;
* EmbeddingProvider;
* SourceProvider;
* Extractor;
* Linker;
* Checker;
* Projector;
* AgentInterface;
* Transport;
* ModuleRegistry.

Определи:

* какие traits слишком завязаны на конкретную реализацию;
* какие traits слишком широкие;
* какие traits слишком мелкие;
* где нарушена ответственность;
* где в будущем будет больно добавить новый adapter.

Особенно проверь, можно ли без переписывания ядра:

* заменить SurrealDB на Postgres;
* заменить Tantivy на другой search engine;
* заменить local vector store на pgvector/Qdrant;
* добавить новый язык программирования;
* добавить новый framework adapter;
* добавить новый generated output;
* добавить новый транспорт вместо MCP.

## 3. Проверка режимов сборки

Проверь Cargo features и dependency graph.

Критически важно:

* standalone-сборка может тянуть SurrealDB;
* rustok-module-сборка не должна тянуть SurrealDB;
* rustok-module-сборка должна использовать Postgres/SeaORM;
* ядро не должно тянуть Axum/Loco;
* CLI не должен быть обязательным для library mode;
* MCP не должен быть обязательным;
* HTML/wiki projectors не должны быть обязательными для ядра.

Проверь:

```bash
cargo tree
cargo tree --features standalone
cargo tree --features rustok-module
cargo tree -i surrealdb
cargo tree -i sea-orm
cargo tree -i axum
cargo tree -i loco-rs
cargo tree -i tantivy
```

Сделай вывод:

* какие зависимости протекли не туда;
* какие features надо разделить;
* какие crates надо вынести;
* где нарушена модульность.

## 4. Проверка модели данных

Проверь доменные типы:

* Entity;
* Fact;
* Relation;
* Evidence;
* Diagnostic;
* Snapshot;
* ContextPack;
* Concept;
* ManualKnowledge.

Определи:

* хватает ли стабильных ID;
* не используется ли path/line number как identity;
* есть ли stable_key;
* есть ли evidence у фактов и связей;
* есть ли confidence/status;
* есть ли snapshot/version;
* можно ли хранить мультиязычность;
* можно ли переносить модель в JSON/JSONL/Postgres JSONB;
* нет ли полей, которые привязаны к конкретной БД или UI.

Особенно проверь:

* можно ли пересобрать read-models из canonical store;
* можно ли объяснить любую связь через evidence;
* можно ли хранить broken/missing/stale/conflicting relations;
* можно ли поддержать diff snapshot / working tree snapshot / PR snapshot.

## 5. Проверка инкрементальности

Проверь pipeline обновления:

* source discovery;
* change detection;
* extraction plan;
* fact extraction;
* identity resolution;
* store transaction;
* relation linking;
* diagnostics checking;
* read-model projection;
* wiki generation;
* atomic publish.

Найди:

* где система пересчитывает слишком много;
* где может оставить старые факты;
* где может появиться рассинхрон между facts/relations/diagnostics/wiki/search;
* где нет dependency map;
* где нет invalidation logic;
* где возможны race conditions.

Проверь, можно ли корректно обработать:

* изменённый файл;
* удалённый файл;
* переименованный файл;
* изменённый markdown-заголовок;
* удалённый endpoint;
* переименованный symbol;
* изменение OpenAPI;
* изменение `.env.example`;
* изменение Dockerfile/script.

## 6. Проверка generated wiki

Проверь `.athanor/generated`.

Важно:

* generated wiki не должна быть источником истины;
* агент может её читать;
* если агент её сломает, Athanor должен пересоздать её;
* генерация должна быть атомарной;
* current pointer не должен указывать на неполную генерацию;
* manifest должен содержать schema_version, snapshot, generation, status;
* Markdown должен быть удобен агенту;
* JSONL должен быть пригоден для машинного чтения.

Проверь:

* нет ли ручных knowledge-файлов внутри generated;
* не смешаны ли generated и editable данные;
* можно ли безопасно удалить generated и пересобрать;
* не читает ли core generated wiki как источник истины.

## 7. Проверка ManualKnowledge

Проверь:

* manual-links;
* annotations;
* ignore-rules;
* domain-map;
* glossary;
* i18n aliases.

Найди:

* где агент может испортить knowledge без валидации;
* где нужны команды вместо прямого редактирования;
* где manual knowledge не учитывается при linking/checking;
* где нет audit/evidence для ручной правки;
* где ignore-rules могут скрыть реальные ошибки.

## 8. Проверка agent-native интерфейса

Проверь команды:

* ath ping;
* ath status;
* ath update --changed;
* ath context;
* ath impact;
* ath check affected;
* ath explain;
* ath link;
* ath annotate;
* ath ignore;
* ath propose-fix.

Найди:

* где агенту придётся читать слишком много;
* где ответы не структурированы;
* где нет JSON output;
* где нет machine-readable ошибок;
* где команда смешивает human output и agent output;
* где нет возможности объяснить результат;
* где действия агента обходят validation.

## 9. Проверка безопасности

Проверь:

* чтение `.env`;
* CI secrets;
* docker-compose;
* shell scripts;
* generated context packs;
* logs;
* diagnostics;
* vector embeddings.

Найди:

* где секреты могут попасть в wiki;
* где секреты могут попасть в JSONL;
* где секреты могут попасть в embeddings;
* где агент может получить значения secret;
* где надо добавить redaction;
* где надо добавить do-not-index rules;
* где надо добавить safe context mode.

## 10. Проверка многоязычности

Проверь, можно ли сейчас или без ломки добавить:

* language на Entity/Fact/DocSection;
* language detection;
* aliases;
* glossary;
* Concept;
* localized wiki;
* localized context packs;
* cross-language search.

Найди места, где английский язык зашит неявно:

* enum names можно оставить английскими;
* user-facing text должен быть локализуемым;
* diagnostics message должен быть отделён от diagnostic kind;
* docs/wiki generation должна поддерживать locale.

## 11. Проверка language/framework adapters

Проверь текущую поддержку языков и фреймворков.

Важно:

* language adapter не должен смешиваться с framework adapter;
* Rust extractor не должен быть жёстко встроен в pipeline;
* OpenAPI/docs/scripts не должны быть второстепенными;
* framework-specific logic должна быть расширяемой.

Проверь, можно ли добавить:

* TypeScript;
* Python;
* PHP;
* Go;
* Magento;
* FastAPI;
* Express;
* Loco;
* Axum;
* Docker;
* GitHub Actions.

Без изменения core.

## 12. Проверка projectors/read-models

Проверь:

* MarkdownWikiProjector;
* JsonlProjector;
* HtmlReportProjector;
* PostgresReadModelProjector;
* TantivyProjector;
* VectorProjector.

Определи:

* можно ли удалить read-model и пересобрать;
* нет ли read-model как источника истины;
* не пишет ли projector обратно в canonical store без явной команды;
* есть ли versioning output format;
* есть ли atomic generation.

## 13. Проверка Rustok compatibility

Проверь, готов ли MVP к будущему Rustok module mode:

* нет ли SurrealDB в rustok-module dependency graph;
* есть ли Postgres/SeaORM adapter;
* есть ли JSONB-compatible model;
* можно ли использовать AppContext/ctx.db;
* не завязан ли Athanor на свой daemon внутри Rustok;
* можно ли подключить routes через Loco/Axum adapter;
* можно ли использовать Rustok permissions;
* можно ли не тащить CLI в библиотечный режим.

## 14. Проверка тестовой стратегии

Проверь наличие тестов:

* unit tests для domain model;
* adapter contract tests;
* store conformance tests;
* extractor golden tests;
* linker tests;
* checker tests;
* incremental update tests;
* generated wiki tests;
* JSONL compatibility tests;
* cargo feature tests;
* dependency leak tests;
* security/redaction tests.

Составь список недостающих тестов.

Особенно нужны тесты:

```text
одно и то же поведение на SurrealStore и PostgresStore
удаление файла удаляет/устаревает факты
generated wiki пересоздаётся атомарно
rustok-module не тянет surrealdb
agent JSON output стабилен
secret не попадает в context pack
```

## 15. Проверка производительности

Оцени архитектуру с точки зрения:

* 1 000 файлов;
* 50 000 файлов;
* 300 000 файлов;
* большие markdown-доки;
* много JSONL;
* много relations;
* частые file watcher updates.

Найди:

* где будут O(n²) места;
* где нужны batch writes;
* где нужны индексы;
* где нужны lazy projectors;
* где нужен throttling для daemon;
* где embeddings надо делать lazy;
* где слишком много копирования JSON.

## 16. Проверка будущих возможностей

Определи, можно ли без ломки добавить:

* LSP integration;
* SCIP/LSIF import;
* Qdrant;
* OpenSearch;
* cloud embeddings;
* GitHub source provider;
* PR snapshot;
* CI integration;
* community plugins;
* WASM plugins later;
* advanced Rustok dashboard;
* multi-repo mode.

Для каждого пункта укажи:

* можно добавить без изменения core;
* нужно слегка расширить port;
* текущая архитектура мешает;
* надо исправить сейчас.

## 17. Формат итогового отчёта

Сделай итоговый отчёт в таком формате:

# Athanor MVP Architecture Audit

## Executive summary

Коротко:

* что хорошо;
* что опасно;
* что срочно исправить;
* что можно отложить.

## Critical issues

Для каждого:

```text
ID:
Severity: critical / high / medium / low
Area:
Files:
Problem:
Why it matters:
Evidence:
Recommended fix:
Estimated effort:
Should fix before next milestone: yes/no
```

## Hidden coupling map

Покажи, где ядро связано с реализациями.

## Dependency leaks

Покажи, какие зависимости попали не в тот crate/feature.

## Missing abstractions

Покажи, каких ports/traits не хватает.

## Over-abstractions

Покажи, где абстракция преждевременная и усложняет код без пользы.

## Data model risks

Покажи риски в Entity/Fact/Relation/Evidence/Diagnostic/Snapshot/ContextPack/Concept.

## Incrementality risks

Покажи риски в update pipeline.

## Agent-native risks

Покажи риски в CLI/socket/wiki/MCP.

## Security risks

Покажи риски по секретам, env, logs, embeddings.

## Rustok compatibility risks

Покажи риски для будущего module mode.

## Test gaps

Покажи недостающие тесты.

## Recommended refactoring plan

Разбей на:

```text
Fix now
Fix before beta
Fix later
Do not fix yet
```

## Final verdict

Дай честную оценку:

```text
Can this MVP evolve into the intended Athanor architecture without rewrite?
Yes / Mostly yes / Risky / No
```

Если ответ не “Yes”, объясни, что конкретно надо исправить, чтобы избежать будущего переписывания ядра.

Работай строго по репозиторию. Не выдумывай проблем без evidence. Любой вывод должен ссылаться на конкретные файлы, типы, зависимости, команды или архитектурные решения.
