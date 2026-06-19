# Athanor — финальный архитектурный план

## 1. Суть продукта

**Athanor** — независимый Rust-based Code Knowledge Engine для AI-агентов и разработчиков.

Он строит инкрементальную, проверяемую и объяснимую базу знаний проекта, связывая:

* код;
* документацию;
* API;
* скрипты;
* конфиги;
* тесты;
* зависимости;
* БД/миграции;
* историю Git;
* доменные понятия;
* несоответствия и ошибки.

Главная цель Athanor — дать агенту не поиск по файлам, а целостную карту смысла проекта:

```text
что существует
как связано
откуда мы это знаем
насколько это точно
что устарело
что сломано
что влияет на что
какой контекст нужен для задачи
```

Athanor не является просто графом кода, MCP-сервером, CLI-утилитой или поисковиком. Это движок знаний проекта.

---

## 2. Главный архитектурный принцип

Фундамент Athanor:

```text
Facts → Relations → Diagnostics → Context
```

То есть система сначала извлекает факты, потом строит связи, потом ищет проблемы, потом формирует контекст для агента.

Граф, wiki, поиск, HTML-отчёты, Postgres-таблицы, векторный индекс — это read-models. Они могут быть удалены и пересобраны.

Источник истины:

```text
Entity
Fact
Relation
Evidence
Diagnostic
Snapshot
ContextPack
Concept
ManualKnowledge
```

---

## 3. Что Athanor должен дать агенту

Плохой ответ агенту:

```text
Вот 20 файлов, где встречается слово auth.
```

Правильный ответ Athanor:

```text
Задача относится к authentication.

Главные сущности:
- api://POST:/api/login
- symbol://rust:crate::auth::login
- env://JWT_SECRET
- doc://docs/auth.md#login-flow

Связи:
- endpoint реализован в src/routes/auth.rs
- бизнес-логика находится в src/auth/login.rs
- документация частично устарела
- OpenAPI совпадает частично
- .env.example не содержит JWT_SECRET

Риски:
- изменение затронет refresh token
- может сломать middleware
- нет теста на expired token

Рекомендуемые проверки:
- cargo test auth
- ath check api
- ath check docs
```

---

## 4. Нерушимые принципы ядра

### 4.1. Core не знает про реализации

Ядро не должно знать про:

```text
SurrealDB
Postgres
SeaORM
Axum
Loco
MCP
Tantivy
Qdrant
HTML
Rustok
```

Ядро знает только доменную модель и контракты.

---

### 4.2. Всё является сущностью

Сущностями являются:

```text
file
symbol
function
class
module
api_endpoint
api_schema
doc_section
script
script_command
env_var
db_table
db_migration
test_case
ci_job
docker_service
feature
package
dependency
concept
```

---

### 4.3. Каждый факт имеет доказательство

Любой факт или связь должны иметь evidence:

```text
source_file
line_start
line_end
extractor
commit_hash
confidence
status
```

Агент должен всегда уметь спросить:

```bash
ath explain api://POST:/api/login
ath why doc://docs/auth.md#login api://POST:/api/login
```

---

### 4.4. Статусы связей обязательны

Связь может быть:

```text
verified      подтверждена точно
inferred      выведена логически
suspected     предположительная
broken        ссылка есть, цели нет
missing       связь должна быть, но её нет
conflicting   источники противоречат
stale         связь устарела
```

---

### 4.5. Версионирование обязательно

Athanor должен понимать:

```text
repo
branch
commit
snapshot
working tree
diff snapshot
PR snapshot
```

Команды:

```bash
ath context --diff
ath impact --diff
ath check affected --diff
```

---

## 5. Каноническая модель данных

### Entity

```rust
pub struct Entity {
    pub id: EntityId,
    pub stable_key: StableKey,
    pub kind: EntityKind,
    pub name: String,
    pub title: Option<String>,
    pub source: Option<SourceLocation>,
    pub language: Option<LanguageCode>,
    pub aliases: Vec<String>,
    pub payload: serde_json::Value,
}
```

Примеры stable key:

```text
file://src/auth/login.rs
symbol://rust:crate::auth::login
api://POST:/api/login
doc://docs/auth.md#login-flow
script://scripts/deploy.sh
env://JWT_SECRET
db://table/users
concept://authentication
```

---

### Fact

```rust
pub struct Fact {
    pub id: FactId,
    pub kind: FactKind,
    pub subject: EntityId,
    pub object: Option<EntityId>,
    pub value: serde_json::Value,
    pub evidence: Vec<Evidence>,
    pub snapshot: SnapshotId,
    pub extractor: String,
    pub confidence: f32,
}
```

Примеры фактов:

```text
symbol_defined
route_declared
doc_section_found
doc_mentions_symbol
env_var_used
script_references_file
migration_creates_table
function_queries_table
test_covers_symbol
```

---

### Relation

```rust
pub struct Relation {
    pub id: RelationId,
    pub kind: RelationKind,
    pub from: EntityId,
    pub to: EntityId,
    pub status: RelationStatus,
    pub confidence: f32,
    pub evidence: Vec<Evidence>,
    pub snapshot: SnapshotId,
    pub payload: serde_json::Value,
}
```

Примеры связей:

```text
defines
contains
imports
calls
implements
documents
uses_env
queries_table
covered_by_test
changed_with
introduced_in
outdated_against
contradicts
broken_reference
```

---

### Diagnostic

```rust
pub struct Diagnostic {
    pub id: DiagnosticId,
    pub kind: DiagnosticKind,
    pub severity: Severity,
    pub status: DiagnosticStatus,
    pub title: String,
    pub message: String,
    pub entities: Vec<EntityId>,
    pub evidence: Vec<Evidence>,
    pub snapshot: SnapshotId,
    pub suggested_fix: Option<String>,
    pub payload: serde_json::Value,
}
```

Примеры диагностик:

```text
missing_documentation
stale_documentation
openapi_mismatch
broken_script_reference
missing_env_var
orphan_doc
dead_endpoint
uncovered_symbol
migration_code_mismatch
```

---

### ContextPack

```rust
pub struct ContextPack {
    pub id: ContextPackId,
    pub task: String,
    pub scope: Vec<String>,
    pub level: ContextLevel,
    pub language: Option<LanguageCode>,
    pub summary: String,
    pub entities: Vec<EntityId>,
    pub files: Vec<String>,
    pub diagnostics: Vec<DiagnosticId>,
    pub suggested_checks: Vec<String>,
    pub confidence: f32,
    pub payload: serde_json::Value,
}
```

Уровни:

```text
summary
normal
deep
full
```

---

### Concept

```rust
pub struct Concept {
    pub id: ConceptId,
    pub canonical_name: String,
    pub terms: HashMap<LanguageCode, Vec<String>>,
    pub related_entities: Vec<EntityId>,
    pub payload: serde_json::Value,
}
```

Пример:

```json
{
  "id": "concept://authentication",
  "canonical_name": "Authentication",
  "terms": {
    "en": ["authentication", "auth", "login flow"],
    "ru": ["авторизация", "аутентификация", "вход пользователя"]
  },
  "related_entities": [
    "api://POST:/api/login",
    "symbol://rust:crate::auth::login",
    "doc://docs/auth.md#login"
  ]
}
```

---

## 6. Typed JSON как главный переносимый формат

Основной формат Athanor — versioned typed JSON documents.

Пример Entity:

```json
{
  "schema": "athanor.entity.v1",
  "id": "ent_auth_login",
  "repo_id": "repo_rustok",
  "snapshot_id": "snap_abc123",
  "stable_key": "api://POST:/api/login",
  "kind": "api_endpoint",
  "name": "POST /api/login",
  "status": "active",
  "language": "en",
  "aliases": ["login", "auth login", "вход"],
  "source": {
    "path": "openapi.yaml",
    "line_start": 42,
    "line_end": 67
  },
  "payload": {
    "method": "POST",
    "path": "/api/login",
    "auth_required": false,
    "tags": ["authentication"]
  }
}
```

Для файлового экспорта:

```text
entities.jsonl
facts.jsonl
relations.jsonl
diagnostics.jsonl
context_packs.jsonl
concepts.jsonl
```

Одна строка = один объект.

---

## 7. Postgres / JSONB совместимость

Для Postgres используется гибрид:

```text
важные поля → обычные колонки
гибкие данные → payload jsonb
доказательства → evidence jsonb
```

Пример таблицы:

```sql
create table athanor_entities (
    id text primary key,
    repo_id text not null,
    snapshot_id text not null,
    stable_key text not null,
    kind text not null,
    name text,
    title text,
    status text not null default 'active',
    language text,
    source_path text,
    source_line_start integer,
    source_line_end integer,
    aliases jsonb not null default '[]',
    payload jsonb not null default '{}',
    created_at timestamptz not null,
    updated_at timestamptz not null
);

create unique index athanor_entities_stable_idx
on athanor_entities (repo_id, snapshot_id, stable_key);

create index athanor_entities_kind_idx
on athanor_entities (repo_id, snapshot_id, kind);

create index athanor_entities_payload_gin_idx
on athanor_entities using gin (payload);
```

---

## 8. Два режима хранения

### 8.1. Standalone Athanor

```text
SurrealDB embedded
Tantivy
local vector index
Markdown/JSONL wiki
single binary
```

Цель — молниеносная локальная работа без внешней БД.

---

### 8.2. Rustok module

```text
Postgres
SeaORM
Rustok/Loco adapter
Rustok permissions
Rustok dashboards
```

В режиме Rustok-модуля SurrealDB не тянется.

Фиксация:

```text
standalone = SurrealDB
rustok-module = Postgres через SeaORM
```

---

## 9. Cargo features

```toml
[features]
default = ["standalone"]

standalone = [
  "store-surreal",
  "search-tantivy",
  "wiki-md",
  "jsonl",
  "cli",
  "daemon"
]

rustok-module = [
  "store-postgres-seaorm",
  "rustok-adapter",
  "api-axum"
]

store-surreal = ["dep:surrealdb"]
store-postgres-seaorm = ["dep:sea-orm"]
search-tantivy = ["dep:tantivy"]

vector-local = []
vector-pgvector = []

api-axum = ["dep:axum"]
transport-mcp = []
transport-socket = []
export-html = []
export-jsonl = []

lang-rust = []
lang-typescript = []
lang-python = []
lang-go = []
lang-php = []

framework-axum = []
framework-loco = []
framework-fastapi = []
framework-express = []
```

Сборки:

```bash
cargo build -p ath --features standalone

cargo build -p athanor-rustok-adapter \
  --features rustok-module
```

---

## 10. Ports and adapters

Athanor строится как hexagonal architecture:

```text
Domain Model
  ↓
Core Ports
  ↓
Adapters
```

Ядро знает только контракты.

---

### KnowledgeStore

```rust
#[async_trait::async_trait]
pub trait KnowledgeStore: Send + Sync {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> Result<SnapshotId>;

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> Result<()>;
    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> Result<()>;
    async fn put_relations(&self, snapshot: SnapshotId, relations: Vec<Relation>) -> Result<()>;
    async fn put_diagnostics(&self, snapshot: SnapshotId, diagnostics: Vec<Diagnostic>) -> Result<()>;

    async fn query_entities(&self, query: EntityQuery) -> Result<Vec<Entity>>;
    async fn query_relations(&self, query: RelationQuery) -> Result<Vec<Relation>>;
    async fn query_diagnostics(&self, query: DiagnosticQuery) -> Result<Vec<Diagnostic>>;

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> Result<()>;
}
```

Реализации:

```text
SurrealKnowledgeStore
PostgresSeaOrmKnowledgeStore
MemoryKnowledgeStore
JsonlKnowledgeStore
```

---

### SearchIndex

```rust
#[async_trait::async_trait]
pub trait SearchIndex: Send + Sync {
    async fn index_document(&self, doc: SearchDocument) -> Result<()>;
    async fn remove_document(&self, id: SearchDocumentId) -> Result<()>;
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>>;
}
```

Реализации:

```text
TantivySearchIndex
PostgresSearchIndex
MeilisearchAdapter
OpenSearchAdapter
NoopSearchIndex
```

---

### EmbeddingProvider и VectorIndex

```rust
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, input: EmbeddingInput) -> Result<Vec<f32>>;
}

#[async_trait::async_trait]
pub trait VectorIndex: Send + Sync {
    async fn upsert(&self, item: VectorItem) -> Result<()>;
    async fn search(&self, query: VectorQuery) -> Result<Vec<VectorSearchResult>>;
}
```

Реализации:

```text
LocalCandleEmbeddingProvider
OrtEmbeddingProvider
OpenAiEmbeddingProvider

SurrealVectorIndex
PgVectorIndex
QdrantVectorIndex
NoopVectorIndex
```

---

### Extractor

```rust
#[async_trait::async_trait]
pub trait Extractor: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports(&self, source: &SourceFile) -> bool;
    async fn extract(&self, input: ExtractInput) -> Result<ExtractOutput>;
}
```

---

### Linker

```rust
#[async_trait::async_trait]
pub trait Linker: Send + Sync {
    fn name(&self) -> &'static str;
    async fn link(&self, input: LinkInput) -> Result<Vec<Relation>>;
}
```

---

### Checker

```rust
#[async_trait::async_trait]
pub trait Checker: Send + Sync {
    fn name(&self) -> &'static str;
    async fn check(&self, input: CheckInput) -> Result<Vec<Diagnostic>>;
}
```

---

### Projector

```rust
#[async_trait::async_trait]
pub trait Projector: Send + Sync {
    fn name(&self) -> &'static str;
    async fn project(&self, input: ProjectInput) -> Result<()>;
}
```

Реализации:

```text
MarkdownWikiProjector
JsonlProjector
HtmlReportProjector
PostgresReadModelProjector
TantivyProjector
VectorProjector
GraphExportProjector
```

---

## 11. Runtime builder

Athanor должен собираться через runtime builder.

```rust
pub struct AthanorRuntime {
    pub store: Arc<dyn KnowledgeStore>,
    pub search: Arc<dyn SearchIndex>,
    pub vectors: Arc<dyn VectorIndex>,
    pub embeddings: Arc<dyn EmbeddingProvider>,
    pub sources: Vec<Arc<dyn SourceProvider>>,
    pub extractors: Vec<Arc<dyn Extractor>>,
    pub linkers: Vec<Arc<dyn Linker>>,
    pub checkers: Vec<Arc<dyn Checker>>,
    pub projectors: Vec<Arc<dyn Projector>>,
}
```

Standalone:

```rust
let runtime = AthanorRuntime::builder()
    .store(SurrealKnowledgeStore::embedded(".athanor/db")?)
    .search(TantivySearchIndex::new(".athanor/index")?)
    .vectors(SurrealVectorIndex::new())
    .source(LocalFileSystemSource::new("."))
    .projector(MarkdownWikiProjector::new(".athanor/generated"))
    .projector(JsonlProjector::new(".athanor/generated"))
    .extractor(RustLanguageAdapter::default())
    .extractor(MarkdownExtractor::default())
    .build()?;
```

Rustok module:

```rust
let runtime = AthanorRuntime::builder()
    .store(PostgresSeaOrmKnowledgeStore::new(ctx.db.clone()))
    .search(PostgresSearchIndex::new(ctx.db.clone()))
    .vectors(PgVectorIndex::new(ctx.db.clone()))
    .source(RustokProjectSource::new(project_id))
    .projector(PostgresReadModelProjector::new(ctx.db.clone()))
    .build()?;
```

---

## 12. Workspace structure

```text
athanor/
  crates/
    athanor-domain/
    athanor-core/
    athanor-app/

    athanor-store-surreal/
    athanor-store-postgres/
    athanor-store-memory/

    athanor-search-tantivy/
    athanor-search-postgres/

    athanor-vector-local/
    athanor-vector-pgvector/
    athanor-vector-qdrant/

    athanor-source-fs/
    athanor-source-git/
    athanor-source-github/

    athanor-language/
    athanor-lang-rust/
    athanor-lang-ts/
    athanor-lang-python/
    athanor-lang-go/
    athanor-lang-php/

    athanor-framework-axum/
    athanor-framework-loco/
    athanor-framework-fastapi/
    athanor-framework-express/

    athanor-i18n/
    athanor-link/
    athanor-check/
    athanor-context/
    athanor-impact/

    athanor-projector-wiki/
    athanor-projector-jsonl/
    athanor-projector-html/
    athanor-projector-postgres/

    athanor-agent/
    athanor-transport-socket/
    athanor-transport-mcp/
    athanor-api-axum/

    athanor-cli/
    athanor-daemon/
    athanor-rustok-adapter/

  apps/
    ath/
    athd/

  examples/
  test-repos/
  docs/
```

---

## 13. Pipeline индексации

```text
Source Discovery
  ↓
Change Detection
  ↓
Extraction Plan
  ↓
Fact Extraction
  ↓
Identity Resolution
  ↓
Store Transaction
  ↓
Relation Linking
  ↓
Diagnostics Checking
  ↓
Read Model Projection
  ↓
Wiki Generation
  ↓
Atomic Publish
```

---

### Подробно

```text
1. Найти файлы.
2. Определить типы файлов.
3. Посчитать content hash.
4. Найти изменённые, новые и удалённые файлы.
5. Запустить нужные extractors.
6. Получить raw facts/entities.
7. Сопоставить stable_key и entity_id.
8. Записать facts/entities.
9. Пересчитать affected relations.
10. Пересчитать affected diagnostics.
11. Обновить search/vector/impact indexes.
12. Сгенерировать Markdown/JSONL/HTML wiki.
13. Атомарно переключить current generation.
```

---

## 14. Инкрементальность

Инкрементальность работает на уровне фактов и зависимостей.

```text
file hash changed
  ↓
affected source file
  ↓
affected facts
  ↓
affected entities
  ↓
affected relations
  ↓
affected diagnostics
  ↓
affected context packs
  ↓
affected wiki pages
```

Нужно хранить dependency map:

```text
source_file -> facts
fact -> entities
entity -> relations
relation -> diagnostics
entity -> context_packs
entity -> wiki_pages
```

---

## 15. Agent-native режим

MCP — только адаптер. Основной агентный режим должен быть нативным.

Интерфейсы:

```text
CLI
local socket
file inbox/outbox
generated wiki
JSONL data
MCP adapter
HTTP adapter
```

Команды:

```bash
ath ping --json
ath status --json
ath update --changed --json
ath context "исправить авторизацию" --json
ath impact src/auth/session.rs --json
ath check affected --json
ath explain api://POST:/api/login --json
ath link ...
ath annotate ...
ath ignore ...
ath propose-fix ...
```

---

## 16. Папка `.athanor/`

```text
.athanor/
  config.toml

  knowledge/
    manual-links.toml
    annotations.md
    ignore-rules.toml
    domain-map.toml
    i18n/
      glossary.toml
      aliases.ru.toml
      aliases.en.toml

  generated/
    current -> generations/000124
    generations/
      000124/
        manifest.json
        wiki/
          ru/
          en/
          neutral/
        data/
          entities.jsonl
          facts.jsonl
          relations.jsonl
          diagnostics.jsonl
          context_packs.jsonl
          concepts.jsonl
        html/

  db/
  index/
  cache/
  logs/
  locks/
  inbox/
  outbox/
  patches/
  modules/
    enabled.toml
    registry.lock
    configs/
```

Коммитить:

```text
.athanor/config.toml
.athanor/knowledge/
.athanor/modules/enabled.toml
```

Не коммитить:

```text
.athanor/db/
.athanor/index/
.athanor/cache/
.athanor/generated/
.athanor/logs/
.athanor/outbox/
```

---

## 17. Generated wiki

Основной формат для агента:

```text
Markdown + YAML frontmatter + JSONL indexes
```

HTML — только для человека.

```text
.athanor/generated/current/wiki/
  ru/
    index.md
    features/authentication.md
  en/
    index.md
    features/authentication.md
  neutral/
    entities/
    diagnostics/
```

Пример wiki-страницы:

```md
---
id: concept://authentication
kind: concept
language: ru
snapshot: snap_abc123
generated_at: 2026-06-18T12:00:00Z
---

# Авторизация

## Summary

Авторизация включает login, logout, refresh token и проверку сессии.

## Main entities

- api://POST:/api/login
- api://POST:/api/refresh
- symbol://rust:crate::auth::session::refresh_token
- env://JWT_SECRET

## Relevant files

- src/auth/session.rs
- src/routes/auth.rs
- docs/auth.md
- openapi.yaml

## Known diagnostics

- docs/auth.md использует старый endpoint /signin
- .env.example не содержит JWT_SECRET

## Suggested checks

- cargo test auth
- ath check api
- ath check docs
```

---

## 18. Атомарная генерация wiki

Нельзя обновлять generated wiki прямо на месте.

Правильно:

```text
1. Сгенерировать новую папку generations/000125.tmp
2. Проверить manifest
3. Переименовать в generations/000125
4. Переключить current
```

Manifest:

```json
{
  "generation": 125,
  "snapshot": "snap_abc123",
  "status": "complete",
  "athanor_version": "0.1.0",
  "schema_version": "athanor-schema-v1",
  "wiki_format_version": "agent-wiki-v1",
  "created_at": "2026-06-18T12:00:00Z"
}
```

---

## 19. Многоязычность

Многоязычность — часть knowledge model, а не только UI.

Athanor должен поддерживать:

```text
language detection
section language metadata
multilingual aliases
project glossary
concept mapping
localized wiki
localized context packs
cross-language search
```

Команды:

```bash
ath context "исправить авторизацию" --lang ru
ath context "fix authentication" --lang en

ath i18n detect
ath i18n terms
ath i18n align docs
ath i18n missing-translations
ath concept map "авторизация"
ath wiki --lang all
```

Пример glossary:

```toml
[[concepts]]
id = "concept://authentication"

[concepts.terms]
en = ["authentication", "auth", "login", "login flow"]
ru = ["авторизация", "аутентификация", "вход", "логин"]
```

---

## 20. Языковая стратегия

Не писать один универсальный парсер кода.

Писать:

```text
universal knowledge model
+ extractor API
+ language adapters
+ framework adapters
```

Уровни поддержки языка:

```text
Tier 1 — structural
AST, files, symbols, imports.

Tier 2 — semantic
go to definition, references, types, call graph.

Tier 3 — framework
routes, controllers, ORM, migrations, tests.

Tier 4 — agent-grade
impact, diagnostics, docs drift, context packs.
```

---

### v1 языки

```text
Rust
TypeScript / JavaScript
Python
Markdown
OpenAPI
Shell
Dockerfile
docker-compose
YAML
TOML
JSON
.env.example
```

---

### v2 языки

```text
Go
PHP
Java
C#
C/C++
Kotlin
Ruby
```

---

## 21. LanguageAdapter

```rust
pub trait LanguageAdapter: Send + Sync {
    fn language_id(&self) -> LanguageId;
    fn capabilities(&self) -> LanguageCapabilities;

    fn structural_extractors(&self) -> Vec<Arc<dyn Extractor>>;
    fn semantic_providers(&self) -> Vec<Arc<dyn SemanticProvider>>;
    fn framework_extractors(&self) -> Vec<Arc<dyn Extractor>>;
    fn checkers(&self) -> Vec<Arc<dyn Checker>>;
}
```

Capability matrix:

```json
{
  "language": "rust",
  "tier": 4,
  "capabilities": {
    "files": true,
    "symbols": true,
    "imports": true,
    "calls": true,
    "types": "partial",
    "routes": true,
    "dependencies": true,
    "tests": true,
    "diagnostics": true,
    "lsp": "planned"
  }
}
```

---

## 22. Framework adapters

Фреймворки отдельно от языков.

Примеры:

```text
Rust:
  axum
  loco
  actix
  rocket

TypeScript:
  express
  nest
  next

Python:
  fastapi
  django
  flask

PHP:
  laravel
  symfony
  magento

Go:
  gin
  echo
  chi
```

Почему отдельно:

```text
API routes, middleware, ORM и tests часто определяются фреймворком, а не языком.
```

---

## 23. HTTP/API стек

Основной HTTP-стек:

```text
axum
tokio
tower
tracing
```

Причина:

```text
совместимость с Rustok
аккуратность стека
единый подход к handlers/routes/state/errors
```

Rustok mode:

```text
Loco-compatible adapter
AppContext
ctx.db
Rustok permissions
Rustok telemetry
Rustok error model
```

---

## 24. SeaORM и SurrealDB

Решение:

```text
Standalone Athanor:
  SurrealDB embedded

Rustok module:
  Postgres через SeaORM
```

SurrealDB и SeaORM не смешиваются в одном режиме.

Два адаптера, но одна логика:

```text
Athanor Core
  ↓
KnowledgeStore trait
  ↓
SurrealStore / SeaOrmStore
```

В Rustok-модуле SurrealDB не тянется.

---

## 25. Поиск и векторы

Текстовый поиск:

```text
standalone → Tantivy
rustok → Postgres tsvector или отдельный read-model
future → Meilisearch/OpenSearch
```

Векторы:

```text
standalone → local vector index / Surreal vectors
rustok → pgvector
future → Qdrant
```

Embeddings отдельно от VectorIndex:

```text
EmbeddingProvider генерирует вектор
VectorIndex хранит и ищет
```

---

## 26. Security / secrets

Athanor читает `.env`, CI, configs, docker-compose, scripts. Поэтому нужна защита.

Обязательные возможности:

```text
secret detection
secret redaction
do-not-index rules
safe context mode
permission model for plugins
```

Пример:

```toml
[privacy]
redact_secrets = true
ignore_files = [".env", ".env.local"]
allow_env_names = true
allow_env_values = false
```

Агенту можно показывать:

```text
env://DATABASE_URL используется в src/db.rs
```

Но нельзя показывать значение секрета.

---

## 27. Модули и расширения сообщества

Маркетплейс Rustok на старте не нужен.

Фиксация:

```text
Community extensions depend on Athanor, not Rustok.
Rustok marketplace is out of roadmap for now.
```

Управление сначала через CLI и конфиг.

---

### Типы модулей

```text
Extractor
Linker
Checker
Projector
LanguageAdapter
FrameworkAdapter
EmbeddingProvider
VectorIndex
SearchIndex
StorageAdapter
Transport
```

---

### Module manifest

```toml
[plugin]
id = "athanor-framework-magento"
name = "Magento Framework Adapter"
version = "0.1.0"
kind = "framework-adapter"
api_version = "athanor-plugin-v1"
author = "community"
license = "MIT OR Apache-2.0"

[compatibility]
athanor = ">=0.1,<0.2"
rust = ">=1.80"
targets = ["standalone", "rustok-module"]

[capabilities]
extractors = ["magento_xml", "magento_routes", "magento_modules"]
linkers = ["magento_module_linker"]
checkers = ["magento_config_checker"]

[permissions]
filesystem = "project-read"
network = "none"
commands = "none"
env_read = false
secret_read = false

[config_schema]
path = "schema.json"
```

---

### ModuleRegistry

```rust
pub trait AthanorModule: Send + Sync {
    fn manifest(&self) -> ModuleManifest;
    fn register(&self, registry: &mut ModuleRegistry);
}
```

```rust
pub struct ModuleRegistry {
    pub extractors: Vec<Arc<dyn Extractor>>,
    pub linkers: Vec<Arc<dyn Linker>>,
    pub checkers: Vec<Arc<dyn Checker>>,
    pub projectors: Vec<Arc<dyn Projector>>,
}
```

---

### CLI для модулей

```bash
ath module list
ath module info lang-rust
ath module enable lang-rust
ath module disable framework-magento
ath module config framework-magento
ath module doctor
```

На старте это может управлять только встроенными модулями и config-файлами.

---

## 28. Commands

Основные команды продукта:

```bash
ath init
ath index .
ath update
ath status
ath ping

ath context "добавить refresh token"
ath impact src/auth/session.rs
ath explain api://POST:/api/login

ath check docs
ath check api
ath check scripts
ath check env
ath check affected
ath check all

ath broken
ath drift
ath missing
ath map feature authentication

ath wiki
ath export jsonl
ath sync postgres

ath module list
ath module enable lang-rust
ath module disable framework-magento
```

Daemon:

```bash
athd start
athd stop
athd status
```

Agent mode:

```bash
ath agent init
ath agent watch
ath agent context "задача"
ath agent update
ath agent check
```

---

## 29. Roadmap

### Phase 0 — Product Kernel

Цель: зафиксировать фундамент.

```text
athanor-domain
Entity/Fact/Relation/Evidence/Diagnostic/Snapshot/ContextPack/Concept
KnowledgeStore trait
Extractor/Linker/Checker/Projector traits
ModuleRegistry
basic config
```

Результат:

```bash
ath --version
ath init
```

---

### Phase 1 — Storage and JSON model

```text
SurrealDB embedded store
Memory store for tests
JSON/JSONL export
Snapshot model
basic repository model
```

Результат:

```bash
ath index .
ath export jsonl
```

---

### Phase 2 — First vertical slice

Минимальный полный цикл:

```text
Rust extractor
Markdown extractor
OpenAPI extractor
basic docs ↔ API ↔ code linker
basic stale docs checker
basic missing docs checker
```

Результат:

```bash
ath check docs
ath explain api://POST:/login
```

---

### Phase 3 — Agent wiki

```text
Markdown wiki
YAML frontmatter
JSONL data
manifest
atomic generation
current pointer
```

Результат:

```bash
ath wiki
ath context "изменить login"
```

---

### Phase 4 — Инкрементальность

```text
file hashing
change detection
affected graph
partial extraction
affected linkers/checkers
affected wiki pages
```

Результат:

```bash
ath update --changed
ath check affected
```

---

### Phase 5 — Daemon and agent-native access

```text
file watcher
local socket
locks
hot cache
agent commands
inbox/outbox protocol
```

Результат:

```bash
athd start
ath ping --json
ath context "задача" --json
```

---

### Phase 6 — Scripts/configs/environment

```text
Shell
Makefile
Dockerfile
docker-compose
GitHub Actions
.env.example
Cargo.toml
package.json
pyproject.toml
```

Результат:

```bash
ath check scripts
ath check env
```

---

### Phase 7 — i18n and concepts

```text
language detection
glossary
aliases
concept mapping
localized wiki
cross-language context
```

Результат:

```bash
ath concept map "авторизация"
ath context "исправить авторизацию" --lang ru
```

---

### Phase 8 — Search and vectors

```text
Tantivy search
basic semantic search
local embeddings
hybrid search
```

Результат:

```bash
ath search "refresh token"
ath context "добавить refresh token" --level deep
```

---

### Phase 9 — Rustok adapter

```text
Postgres/SeaORM store
JSONB schema
Loco/Axum routes
Rustok-compatible errors
Rustok permissions
Rustok dashboards
```

Результат:

```bash
ath sync postgres
```

В Rustok:

```text
Athanor project overview
Diagnostics dashboard
Context packs
Docs/API drift
```

---

### Phase 10 — Community modules foundation

```text
module manifest
ModuleRegistry
CLI module management
permission model
compatibility checks
```

Результат:

```bash
ath module list
ath module enable framework-magento
```

Без Rustok marketplace.

---

### Phase 11 — Advanced language support

```text
TypeScript/JavaScript deeper support
Python deeper support
Go
PHP
Java
C#
C/C++
LSP integration
SCIP/LSIF import/export
```

---

## 30. Что не делать в v1

```text
не делать полноценный маркетплейс Rustok
не делать сложный SaaS
не делать глубокую поддержку всех языков сразу
не делать dynamic .so plugins
не делать AI обязательным для базовой логики
не делать wiki источником истины
не делать Postgres обязательным для standalone
не делать MCP главным API
не давать агенту прямой доступ к БД
```

---

## 31. Главные продуктовые отличия

Athanor должен быть сильнее обычных code graph tools за счёт:

```text
code + docs + API + scripts + configs + tests + history
evidence everywhere
diagnostics as core feature
agent-native context packs
generated wiki
multilingual concepts
incremental affected updates
manual knowledge
portable JSON/JSONL model
Surreal standalone
Postgres/Rustok module
community extensions through contracts
```

---

## 32. Финальная формула

```text
Athanor = stable knowledge kernel + pluggable adapters.

Standalone:
  SurrealDB embedded
  Tantivy
  local vectors
  Markdown/JSONL wiki
  Axum/socket/CLI
  single binary

Rustok:
  Postgres
  SeaORM
  pgvector/Postgres search
  Loco/Axum adapter
  Rustok permissions/dashboards

Future:
  new languages
  new frameworks
  new checkers
  new vector stores
  new search engines
  new transports
  community modules
```

Фундамент:

```text
Domain model is stable.
Storage is replaceable.
Read-models are rebuildable.
Interfaces are adapters.
Agent corrections are commands.
Wiki is generated.
Manual knowledge is versioned.
Extensions are contract-based.
Rustok integration is an adapter, not the core.
```
