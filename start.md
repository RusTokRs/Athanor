# Athanor — финальный архитектурный план

Agent entrypoint: read `AGENTS.md` before implementation work.

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

Roadmap строится от доменного ядра к проверяемому knowledge loop:

```text
Domain contracts
  → Storage and snapshots
  → First extraction/linking/checking loop
  → Generated read-models
  → Living documentation and API consistency
  → Incremental daemon
  → Search, vectors and integrations
```

Каждая фаза должна давать проверяемый результат в CLI и не протаскивать детали adapters в core.

Конкретные решения по сторонним библиотекам, границы adapters и критерии миграции ведутся в
[`docs/development/library-adoption-plan.md`](docs/development/library-adoption-plan.md). Библиотека
может реализовывать внутренности adapter, но не определяет canonical contracts Athanor.

---

### Phase 0 — Product Kernel

Цель: зафиксировать минимальный доменный фундамент.

Состав:

```text
athanor-domain
Entity / Fact / Relation / Evidence / Diagnostic / Snapshot / ContextPack / Concept
Repo / Branch / Commit / Snapshot identity
KnowledgeStore trait
SourceProvider trait
Extractor / Linker / Checker / Projector traits
SearchIndex / VectorIndex / EmbeddingProvider traits
Transport / AgentInterface contracts
ModuleRegistry
basic config
stable error codes
```

Результат:

```bash
ath --version
ath init
```

---

### Phase 1 — Storage and Portable Data Model

Цель: сделать canonical knowledge сохраняемым, переносимым и тестируемым.

Состав:

```text
Memory store for tests
SurrealDB embedded store
typed JSON schema
JSONL export/import
Snapshot model
basic repository model
source locations
evidence model
confidence/status fields
```

Результат:

```bash
ath index .
ath export jsonl
ath explain <entity>
```

---

### Phase 2 — First Vertical Slice

Цель: получить первый полный цикл Facts → Relations → Diagnostics → Context.

Состав:

```text
Rust extractor
Markdown extractor
OpenAPI extractor
basic docs ↔ API ↔ code linker
basic API Registry entities
basic stale docs checker
basic missing docs checker
basic API consistency checker
```

Результат:

```bash
ath check docs
ath check api
ath explain api://POST:/login
ath context "изменить login"
```

---

### Phase 3 — Generated Read Models

Цель: построить пересоздаваемые read-models для агента и человека.

Состав:

```text
Markdown wiki
YAML frontmatter
JSONL data
manifest
HTML report
agent context packs
atomic generation
current pointer
```

Правило:

```text
Generated wiki не является источником истины.
Generated artifacts можно удалить и пересобрать из canonical store.
```

Результат:

```bash
ath wiki
ath report html
ath context "изменить login" --json
```

---

### Phase 4 — Living Documentation and API Contract

Цель: встроить нижние добавления про документацию и API в основной продуктовый цикл.

Состав:

```text
Editable Documentation model
Generated Documentation model
DocumentationPage / DocumentationSection entities
ApiEndpoint / ApiSchema / ApiExample / ApiSnapshot entities
docs frontmatter
docs patch model
API source_of_truth policy
API strict mode
API examples validation
API contract snapshot
documentation diagnostics gate
```

Результат:

```bash
ath docs check
ath docs drift
ath docs propose-fix
ath docs apply-patch <id>
ath check api --strict
ath api snapshot
ath api diff
```

---

### Phase 5 — Operations, Configs and Environment

Цель: связать эксплуатационные знания с тем же knowledge model.

Состав:

```text
Shell scripts
Makefile
Dockerfile
docker-compose
GitHub Actions
.env.example
Cargo.toml
package.json
pyproject.toml
deployment configs
database migrations
runtime configuration
OperationsDocsChecker
EnvDocsChecker
ScriptDocsChecker
RunbookConsistencyChecker
```

Результат:

```bash
ath docs operations check
ath check scripts
ath check env
ath check deployment
```

---

### Phase 6 — Incrementality

Цель: пересчитывать только affected knowledge.

Состав:

```text
file hashing
change detection
affected graph
partial extraction
affected linkers/checkers
affected docs diagnostics
affected API snapshots
affected wiki pages
repair and garbage collection
deterministic output
```

Результат:

```bash
ath update --changed
ath check affected
ath impact --diff
ath context --diff
```

---

### Phase 7 — Daemon and Agent-Native Access

Цель: дать агентам быстрый локальный доступ без привязки к MCP.

Состав:

```text
file watcher
local socket
locks
hot cache
job system
cancellation
backpressure
debounce
agent commands
inbox/outbox protocol
output size control
```

Результат:

```bash
athd start
ath ping --json
ath context "задача" --json
```

---

### Phase 8 — i18n and Concepts

Цель: сделать знания многоязычными без раздвоения истины.

Состав:

```text
language detection
glossary
aliases
concept mapping
localized wiki
editable docs languages
translation_of relations
translation drift diagnostics
cross-language context
```

Результат:

```bash
ath concept map "авторизация"
ath docs i18n check
ath docs propose-translation --from ru --to en
ath context "исправить авторизацию" --lang ru
```

---

### Phase 9 — Search and Vectors

Цель: добавить быстрый lexical/semantic retrieval как read-model, а не как источник истины.

Состав:

```text
Tantivy search
basic semantic search
local embeddings
hybrid search
vector index adapters
search result evidence
```

Результат:

```bash
ath search "refresh token"
ath context "добавить refresh token" --level deep
```

---

### Phase 10 — Rustok Adapter

Цель: встроить Athanor в Rustok без загрязнения Athanor core.

Состав:

```text
Postgres/SeaORM store
JSONB schema
Loco/Axum routes
Rustok-compatible errors
Rustok permissions
Rustok dashboards
API Registry read-model
docs/API drift dashboards
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
API Overview
API Consistency
Breaking Changes
Docs Drift
Undocumented Endpoints
Invalid Examples
Translation Drift
```

---

### Phase 11 — Community Modules Foundation

Цель: подготовить расширяемость без зависимости от Rustok marketplace.

Состав:

```text
module manifest
ModuleRegistry
CLI module management
permission model
compatibility matrix
extension SDK docs
contract tests for adapters
```

Результат:

```bash
ath module list
ath module enable framework-magento
```

Без Rustok marketplace.

---

### Phase 12 — Advanced Language and Framework Support

Цель: расширять покрытие языков через adapters и importers.

Состав:

```text
TypeScript/JavaScript deeper support
Python deeper support
Go
PHP
Java
C#
C/C++
framework adapters
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

---

## 33. Конфигурация и layering настроек

Athanor должен иметь предсказуемую систему конфигурации, чтобы standalone, daemon, CI, agent mode и Rustok module не конфликтовали.

Источники конфигурации по приоритету:

```text id="92l5xq"
1. CLI flags
2. environment variables
3. .athanor/config.toml
4. user-level config
5. built-in defaults
```

Пример:

```toml id="4zf1p6"
[project]
name = "my-project"
root = "."
default_language = "ru"

[storage]
mode = "surreal-embedded"
path = ".athanor/db"

[search]
engine = "tantivy"

[vectors]
enabled = false
provider = "none"

[wiki]
enabled = true
languages = ["ru", "en"]
atomic_publish = true

[privacy]
redact_secrets = true
allow_env_values = false

[modules]
enabled = [
  "lang-rust",
  "docs-markdown",
  "api-openapi",
  "scripts-shell"
]
```

Обязательное правило:

```text id="o99aeh"
Любой adapter/provider/projector должен получать настройки через config layer, а не читать env/config напрямую внутри себя.
```

---

## 34. API versioning и стабильные контракты

Athanor должен версионировать не только данные, но и публичные контракты.

Версионировать нужно:

```text id="p0ver7"
domain schema
JSON/JSONL schema
CLI JSON output
agent protocol
plugin API
storage migrations
wiki format
MCP tools
HTTP API
```

Примеры версий:

```text id="9q4dmk"
athanor.entity.v1
athanor.relation.v1
agent-protocol-v1
plugin-api-v1
wiki-format-v1
postgres-schema-v1
```

Правило:

```text id="8zwxtz"
Human output CLI можно менять.
Machine-readable JSON output нельзя ломать без версии.
```

Для CLI:

```bash id="4nyjdy"
ath context "auth" --json --protocol agent-v1
```

---

## 35. Stable error codes

Ошибки должны быть машинно-читаемыми, особенно для агентов.

Нужен единый формат:

```json id="28vu6a"
{
  "ok": false,
  "error": {
    "code": "ATHANOR_STORE_UNAVAILABLE",
    "message": "Knowledge store is unavailable",
    "hint": "Run ath status or check .athanor/db",
    "retryable": true,
    "details": {}
  }
}
```

Классы ошибок:

```text id="pntpk3"
ATHANOR_CONFIG_INVALID
ATHANOR_STORE_UNAVAILABLE
ATHANOR_SCHEMA_MISMATCH
ATHANOR_SNAPSHOT_CONFLICT
ATHANOR_INDEX_LOCKED
ATHANOR_PLUGIN_INCOMPATIBLE
ATHANOR_SECRET_REDACTED
ATHANOR_SOURCE_UNREADABLE
ATHANOR_EXTRACTOR_FAILED
ATHANOR_PROJECTOR_FAILED
```

Правило:

```text id="8ce82d"
Агент не должен парсить текст ошибки. Он должен читать stable error code.
```

---

## 36. Job system и cancellation

Индексация, пересборка wiki, embeddings, projectors, Postgres sync и deep checks могут быть долгими. Нужна модель задач.

Сущность:

```rust id="6td2q3"
pub struct Job {
    pub id: JobId,
    pub kind: JobKind,
    pub status: JobStatus,
    pub progress: f32,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub input: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error: Option<AthanorError>,
}
```

Статусы:

```text id="npqp30"
queued
running
cancelling
cancelled
failed
completed
```

Команды:

```bash id="1gl6hq"
ath jobs list
ath jobs status <id>
ath jobs cancel <id>
ath update --async
ath wiki --async
```

Обязательное правило:

```text id="al4euj"
Любая долгая операция должна быть отменяемой и повторяемой.
```

---

## 37. Recovery, repair и garbage collection

Нужно заранее заложить восстановление после сбоев.

Команды:

```bash id="094ow7"
ath doctor
ath repair
ath gc
ath rebuild read-models
ath rebuild wiki
ath rebuild search
ath rebuild vectors
```

Сценарии:

```text id="qevycd"
сломалась generated wiki
сломался Tantivy index
частично записался snapshot
устарели relations после падения
битый manifest
несовместимая schema version
```

Правило:

```text id="mr08df"
Canonical store должен позволять пересобрать все read-models.
Read-model corruption не должен ломать проект.
```

Garbage collection:

```text id="e7yfgu"
старые snapshots
старые generations
устаревшие embeddings
старые job logs
старые temporary folders
```

---

## 38. Path normalization, encodings, symlinks

Это маленькая, но очень опасная зона. Без неё потом будут странные баги.

Athanor должен нормализовать:

```text id="w9k7nk"
Windows paths
Unix paths
case-sensitive/case-insensitive FS
symlinks
submodules
nested git repos
line endings CRLF/LF
UTF-8 / non-UTF-8 files
renames
relative paths
absolute paths
```

Правило stable key:

```text id="k7qwgl"
stable_key не должен зависеть от текущей ОС.
```

Пример:

```text id="3i42jl"
file://src/auth/login.rs
```

а не:

```text id="3yg7fm"
C:\Users\...\src\auth\login.rs
```

Нужен отдельный модуль:

```text id="ld5r97"
athanor-path
```

---

## 39. Ignore rules и large-file policy

Athanor не должен индексировать всё подряд.

Источники ignore rules:

```text id="4nqr7h"
.gitignore
.athanorignore
.athanor/config.toml
built-in ignore rules
```

Игнорировать по умолчанию:

```text id="zcr3wn"
node_modules
target
vendor
dist
build
.cache
.git
binary files
large generated files
lock artifacts where not needed
secrets
```

Пример `.athanorignore`:

```gitignore id="5xi43m"
node_modules/
target/
dist/
*.min.js
*.map
.env
.env.local
```

Нужна политика больших файлов:

```text id="czcbqe"
skip
hash-only
metadata-only
full-index
```

---

## 40. Determinism и reproducibility

Athanor должен давать воспроизводимый результат.

Если один и тот же snapshot индексируется дважды, результат должен быть одинаковым:

```text id="jw1pci"
одинаковые stable_key
одинаковые facts
одинаковые relations
одинаковые diagnostics
одинаковый JSONL output
```

Нужно избегать:

```text id="h2tfpa"
random ids без deterministic seed
зависимости от порядка файловой системы
зависимости от текущего времени внутри facts
нестабильного порядка JSONL
нестабильной сортировки diagnostics
```

Правило:

```text id="mq2lkh"
created_at может отличаться.
domain output должен быть deterministic.
```

Это критично для тестов, diff, CI и доверия агента.

---

## 41. Source priority и conflict resolution

Мы обсуждали trust model, но надо явно добавить conflict resolution.

Источники имеют приоритет:

```text id="mvpp96"
code
compiler/language server
tests
OpenAPI
migrations
scripts
README/docs
comments
AI inference
manual knowledge
```

Но manual knowledge не всегда должен побеждать код. Нужно правило:

```text id="l3u4r5"
manual knowledge can override interpretation,
but cannot silently override observed facts.
```

Если источники конфликтуют, создаётся diagnostic:

```text id="ihw5rn"
docs_contradict_code
openapi_contradicts_code
manual_link_conflicts_with_observed_fact
```

Формат:

```json id="x1arfe"
{
  "kind": "conflict",
  "sources": ["code", "docs"],
  "winner": "code",
  "status": "open",
  "confidence": 0.92
}
```

---

## 42. Audit trail для ручных действий агента

Все действия агента и человека, которые меняют knowledge, должны иметь audit trail.

Логировать:

```text id="huvhvw"
ath link
ath annotate
ath ignore
ath confirm
ath reject
ath apply-patch
ath module enable/disable
```

Сущность:

```rust id="i8noze"
pub struct KnowledgeAuditEvent {
    pub id: AuditEventId,
    pub actor: Actor,
    pub action: String,
    pub target: String,
    pub reason: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}
```

Правило:

```text id="s2yurv"
Нельзя молча скрывать diagnostic через ignore-rule без reason.
```

---

## 43. Patch model

Athanor должен различать:

```text id="98aib5"
knowledge corrections
project file changes
generated output
```

Для исправлений проекта нужен patch model.

Команды:

```bash id="jx2a82"
ath propose-fix diag_stale_auth_docs
ath patch list
ath patch show <id>
ath patch apply <id>
ath patch reject <id>
```

Patch должен содержать:

```text id="f5uf3j"
affected files
diff
reason
source diagnostic
expected resolved diagnostics
suggested checks
```

Generated patches не должны применяться автоматически без явной команды.

---

## 44. Contract tests для adapters

Для каждого adapter нужно иметь общий набор тестов.

Например, любой `KnowledgeStore` должен пройти один и тот же test suite:

```text id="zl5vfm"
put/get entity
put/get facts
relations query
diagnostics query
snapshot commit
rollback failed snapshot
idempotent writes
delete/obsolete old facts
JSON payload preservation
```

Один и тот же тест должен запускаться для:

```text id="w1a8xq"
MemoryKnowledgeStore
SurrealKnowledgeStore
PostgresSeaOrmKnowledgeStore
```

Это защитит от ситуации, когда standalone и Rustok module ведут себя по-разному.

---

## 45. CI quality gates

Нужны специальные проверки архитектуры в CI.

Минимум:

```bash id="tafbu8"
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all
cargo test --no-default-features
cargo tree --features standalone -i sea-orm
cargo tree --features rustok-module -i surrealdb
cargo deny check
```

Дополнительные проверки:

```text id="n9w5zw"
domain crate не зависит от adapter crates
core crate не зависит от axum/loco/surrealdb/seaorm/tantivy
rustok-module не тянет surrealdb
standalone не тянет loco
generated wiki can be rebuilt
JSONL schema snapshots stable
secret redaction tests pass
```

---

## 46. License, dependency и supply-chain policy

Так как Athanor будет независимым продуктом и с community-модулями, нужно заранее определить политику зависимостей.

Проверять:

```text id="hq9ii6"
license compatibility
unmaintained crates
security advisories
duplicate heavy dependencies
native dependencies
OpenSSL/system dependencies
binary size impact
```

Инструменты:

```text id="3tq7yr"
cargo-deny
cargo-audit
cargo-udeps
cargo-bloat
```

Правило:

```text id="x6z2ho"
Любой новый adapter обязан указать dependency impact.
```

---

## 47. Binary size и feature budget

Так как standalone должен быть единым быстрым бинарником, нужно следить за размером.

Для каждой feature:

```text id="d6hzld"
store-surreal
search-tantivy
vector-local
lang-typescript
lang-python
html-export
mcp
http
```

нужно понимать:

```text id="csye7o"
размер бинарника
время компиляции
runtime memory
native dependencies
```

Команда:

```bash id="6krfrt"
ath doctor build-info
```

Должна показывать включённые features и адаптеры.

---

## 48. Resource limits

Daemon и agent mode должны иметь лимиты.

Настройки:

```toml id="p8y90b"
[limits]
max_file_size_mb = 5
max_total_indexed_mb = 2048
max_parallel_extractors = 8
max_embedding_jobs = 2
max_wiki_generations_kept = 5
max_snapshot_history = 50
```

Нужно избежать ситуации, когда агент запустил глубокую переиндексацию и положил машину.

---

## 49. Backpressure и debounce для daemon

File watcher не должен запускать 100 обновлений подряд.

Нужно:

```text id="2rdx4m"
debounce file changes
batch updates
coalesce events
queue priority
cancel obsolete jobs
```

Пример:

```text id="wlv25u"
агент изменил 20 файлов
→ daemon ждёт 300-1000 ms
→ собирает batch
→ делает один affected update
```

---

## 50. Observability

Athanor должен быть наблюдаемым.

Минимум:

```text id="lddbxi"
structured logs
trace id
job id
snapshot id
generation id
duration metrics
extractor metrics
checker metrics
projector metrics
```

Команды:

```bash id="ev33r0"
ath metrics
ath logs
ath doctor
```

Для Rustok:

```text id="uoudw7"
интеграция с Rustok telemetry
```

Для standalone:

```text id="0d60vh"
tracing pretty/json logs
optional OpenTelemetry
```

---

## 51. Performance benchmark suite

Нужны benchmark-репозитории.

Категории:

```text id="t5co5l"
small: 1k files
medium: 50k files
large: 300k files
dirty docs project
broken scripts project
mixed language project
large markdown project
large OpenAPI project
```

Измерять:

```text id="ulvnpx"
initial index time
incremental update time
memory usage
wiki generation time
search latency
context generation latency
diagnostic time
```

Целевые метрики фиксировать отдельно.

---

## 52. Output size control для агента

ContextPack не должен быть бесконечным.

Нужны лимиты:

```text id="72b4b7"
max_tokens
max_files
max_entities
max_diagnostics
max_depth
```

Команды:

```bash id="uzwxto"
ath context "auth" --budget 8000
ath context "auth" --level summary
ath context "auth" --level deep --max-files 20
```

Athanor должен уметь объяснить, что было исключено:

```text id="z7n210"
omitted_entities
omitted_files
reason: budget_limit
```

---

## 53. Confidence calibration

Если у связей есть confidence, нужно сделать его осмысленным.

Нельзя, чтобы разные extractors ставили confidence произвольно.

Нужна шкала:

```text id="45ztrt"
1.00 observed exact
0.90 strongly verified by multiple sources
0.70 inferred by structure/name
0.50 weak semantic match
0.30 suspected
```

Каждый Extractor/Linker должен документировать, как он выставляет confidence.

---

## 54. Human review workflow

Некоторые diagnostics и inferred relations должны проходить ручное подтверждение.

Статусы:

```text id="z02eey"
open
confirmed
ignored
false_positive
fixed
stale
```

Команды:

```bash id="ocossn"
ath confirm relation <id>
ath reject relation <id>
ath confirm diagnostic <id>
ath ignore diagnostic <id> --reason "..."
```

Это важно, чтобы база знаний становилась лучше со временем.

---

## 55. Compatibility matrix

Нужно хранить matrix возможностей не только для языков, но и для всех adapters.

```json id="6chvgk"
{
  "adapter": "SurrealKnowledgeStore",
  "kind": "store",
  "capabilities": {
    "snapshots": true,
    "transactions": "partial",
    "vectors": true,
    "full_text": "partial",
    "graph_queries": true
  }
}
```

Для каждого adapter:

```text id="4x24e4"
supported
partial
planned
unsupported
```

Агент должен понимать, какие операции доступны в текущей сборке.

---

## 56. Public extension SDK documentation

Если будет community extension system, нужен не marketplace, а понятный SDK.

Документация:

```text id="58iax0"
How to write Extractor
How to write Checker
How to write FrameworkAdapter
How to test plugin
How to define manifest
How permissions work
How compatibility works
```

Команда-шаблон:

```bash id="v7bv71"
ath module new checker my-company-checker
ath module new framework my-framework
```

---

## 57. No side effects rule для extractors/checkers

Extractor, Linker и Checker не должны изменять проект.

Правило:

```text id="s47eus"
Extractor reads.
Linker links.
Checker diagnoses.
Projector writes read-models.
Only explicit command can modify manual knowledge or project files.
```

Запрещено:

```text id="cni8jv"
extractor запускает npm install
checker меняет файл
linker пишет в generated wiki
projector пишет в canonical facts без команды
```

---

## 58. External command policy

Некоторые адаптеры могут захотеть запускать внешние команды:

```text id="jg6zqg"
cargo metadata
cargo check
npm
python
go list
composer
docker
```

Нужно явно разделить режимы:

```text id="kos6q4"
safe parse mode
trusted command mode
CI mode
```

По умолчанию:

```text id="xgghjk"
не запускать опасные команды
не выполнять shell scripts
не запускать package install
```

Конфиг:

```toml id="rx7r8p"
[commands]
allow_external = false
allow_network = false
allowed = ["cargo metadata"]
```

---

## 59. Network policy

По умолчанию standalone Athanor должен работать локально и offline.

Любой network access должен быть явным:

```text id="uu0rti"
GitHub provider
cloud embeddings
plugin registry
remote vector DB
license check
telemetry export
```

Конфиг:

```toml id="zy1zd7"
[network]
enabled = false
allow = []
```

---

## 60. Final additional rule

Главное дополнительное правило:

```text id="urzg9a"
Любая новая возможность должна входить в систему через один из контрактов:
Extractor, Linker, Checker, Projector, Store, SearchIndex, VectorIndex,
EmbeddingProvider, SourceProvider, Transport, AgentInterface, Module.

Если новая возможность требует менять domain core,
значит надо сначала доказать, что это действительно новая доменная сущность,
а не adapter-specific деталь.
```

---

## 61. Adapter-first feature rule

Перед добавлением новой возможности нужно задать простой вопрос:

```text
Это часть смысла Athanor или это способ прочитать/записать/показать данные?
```

Если это способ работы с конкретным форматом, языком, фреймворком, базой данных,
поиском, UI, транспортом или внешним сервисом — это должен быть adapter.

Примеры:

```text
Markdown parser             -> extractor adapter
OpenAPI parser              -> extractor adapter
Rust AST parser             -> language adapter
Postgres storage            -> store adapter
SurrealDB storage           -> store adapter
HTML report                 -> projector adapter
MCP                         -> transport adapter
Rustok dashboard            -> Rustok adapter
```

В core можно добавлять только то, что остаётся верным при замене adapter:

```text
Entity
Fact
Relation
Evidence
Diagnostic
Snapshot
ContextPack
Concept
ports/contracts
stable statuses and ids
```

Практическое правило:

```text
Сначала пробуем вынести фичу в adapter.
Если не получается, объясняем почему это новая часть доменной модели.
Только после этого меняем core/domain.
```

---

## 62. Living Documentation и API Consistency

Athanor должен поддерживать не только generated wiki для агента, но и полноценную живую документацию проекта.

Документация делится на три слоя:

```text
1. Canonical Knowledge
   Entity / Fact / Relation / Evidence / Diagnostic / Snapshot / Concept

2. Editable Documentation
   Документация проекта, которую можно править человеком или агентом.

3. Generated Documentation
   Автоматически сгенерированные wiki/API/runbook страницы, которые можно пересоздать.
```

Правило:

```text
Generated documentation не является источником истины.
Editable documentation может быть источником пользовательского знания.
Canonical Knowledge остаётся главным источником для проверок и генерации.
```

---

## 63. Типы документации

Athanor должен различать типы документации:

```text
project_overview
architecture
developer_guide
installation
operations
deployment
configuration
environment
scripts
api_reference
api_examples
troubleshooting
runbook
migration_guide
security
permissions
testing
changelog
module_documentation
```

Особенно важные типы:

```text
API documentation
Operations documentation
Configuration documentation
Script documentation
Environment documentation
```

---

## 64. Editable docs layout

Редактируемая документация может жить в проекте:

```text
docs/
  ru/
    index.md
    api/
    operations/
    configuration/
    troubleshooting/
  en/
    index.md
    api/
    operations/
    configuration/
    troubleshooting/
```

Или в knowledge-папке:

```text
.athanor/knowledge/docs/
  ru/
  en/
```

Лучше поддержать оба варианта.

Конфиг:

```toml
[docs]
editable_path = "docs"
generated_path = ".athanor/generated/current/wiki"
languages = ["ru", "en"]
source_language = "ru"
mode = "patch-based"

[docs.api]
enabled = true
source_of_truth = "hybrid"
strict = true

[docs.operations]
enabled = true
include_scripts = true
include_env = true
include_docker = true
include_ci = true
```

---

## 65. Generated docs не должны перетирать ручные правки

Athanor не должен молча переписывать editable documentation.

Правильный workflow:

```text
1. Athanor находит устаревшую документацию.
2. Athanor генерирует suggested patch.
3. Агент/человек просматривает patch.
4. Patch применяется явно.
5. Athanor пересчитывает diagnostics.
```

Команды:

```bash
ath docs generate
ath docs check
ath docs drift
ath docs propose-fix
ath docs apply-patch <id>
```

Запрещено:

```text
автоматически переписывать docs/ без явной команды
смешивать generated wiki и editable docs
делать generated docs источником истины
```

---

## 66. Структура markdown-документа

Каждый редактируемый документ должен иметь frontmatter.

```md
---
id: doc://docs/ru/api/auth.md
kind: api_documentation
language: ru
source_language: ru
concepts:
  - concept://authentication
entities:
  - api://POST:/api/login
  - api://POST:/api/refresh
last_verified_snapshot: snap_abc123
status: verified
---

# API авторизации

Описание API авторизации...
```

Это позволит Athanor понимать:

```text
какие сущности описывает документ
на каком языке он написан
когда он был проверен
какому snapshot он соответствует
устарел ли он после изменений кода/API
```

---

## 67. Многоязычная документация

Athanor должен поддерживать несколько языковых версий одного документа.

Например:

```text
doc://docs/ru/api/auth.md
doc://docs/en/api/auth.md
```

Между ними создаётся связь:

```text
translation_of
translation_outdated_against
translation_missing
```

Если русская версия обновилась, а английская нет, Athanor создаёт diagnostic:

```text
translation_outdated
```

Пример:

```json
{
  "kind": "translation_outdated",
  "severity": "warning",
  "source_doc": "doc://docs/ru/api/auth.md",
  "target_doc": "doc://docs/en/api/auth.md",
  "message": "English API documentation is older than Russian source documentation.",
  "payload": {
    "source_language": "ru",
    "target_language": "en",
    "changed_sections": ["POST /api/login", "JWT lifetime"]
  }
}
```

Команды:

```bash
ath docs i18n check
ath docs i18n missing
ath docs i18n drift
ath docs translate --from ru --to en docs/ru/api/auth.md
ath docs propose-translation --from ru --to en
```

Важно:

```text
Автоматический перевод должен быть patch/draft, а не silent overwrite.
```

---

## 68. Operations documentation

Athanor должен уметь генерировать и проверять эксплуатационную документацию.

Источники:

```text
Dockerfile
docker-compose.yml
.env.example
Makefile
shell scripts
GitHub Actions
Cargo.toml
package.json
pyproject.toml
deployment configs
database migrations
runtime configuration
```

Документация по эксплуатации должна покрывать:

```text
installation
local development
configuration
environment variables
build
test
run
deployment
backup/restore
logs
metrics
health checks
troubleshooting
CI/CD
scripts
```

Пример diagnostics:

```text
env_var_used_but_not_documented
script_exists_but_not_documented
documented_command_missing
docker_service_not_documented
ci_job_not_documented
deployment_doc_stale
```

Команды:

```bash
ath docs operations generate
ath docs operations check
ath check env
ath check scripts
ath check deployment
```

---

## 69. API Registry

Athanor должен иметь отдельный API Registry внутри knowledge model.

API endpoint — это Entity:

```text
api://GET:/api/users
api://POST:/api/login
api://DELETE:/api/products/{id}
```

API Registry должен хранить:

```text
method
path
operation_id
handler symbol
request body schema
response schemas
status codes
path params
query params
headers
auth requirements
permissions
rate limits
error model
examples
OpenAPI source
code source
documentation source
tests
client usage
version
deprecation status
```

Пример Entity payload:

```json
{
  "stable_key": "api://POST:/api/login",
  "kind": "api_endpoint",
  "payload": {
    "method": "POST",
    "path": "/api/login",
    "operation_id": "login",
    "auth_required": false,
    "request_schema": "schema://LoginRequest",
    "responses": {
      "200": "schema://LoginResponse",
      "400": "schema://ErrorResponse",
      "401": "schema://ErrorResponse"
    },
    "handler": "symbol://rust:crate::routes::auth::login",
    "docs": [
      "doc://docs/ru/api/auth.md#login",
      "doc://docs/en/api/auth.md#login"
    ]
  }
}
```

---

## 70. API source of truth policy

Проект должен явно задавать источник истины для API.

Варианты:

```text
code-first
openapi-first
hybrid
manual-contract
```

Конфиг:

```toml
[api]
source_of_truth = "hybrid"
strict = true
fail_on_missing_docs = true
fail_on_openapi_mismatch = true
fail_on_undocumented_status_code = true
```

Рекомендованный режим для Athanor:

```text
hybrid
```

То есть:

```text
код показывает фактическую реализацию
OpenAPI показывает публичный контракт
docs показывают пользовательское описание
tests/examples подтверждают работоспособность
```

Если они расходятся — это не “мелкая проблема”, а API diagnostic.

---

## 71. API consistency checks

Athanor должен проверять консистентность API между всеми источниками.

Сравнивать:

```text
code routes ↔ OpenAPI
OpenAPI ↔ Markdown docs
code handlers ↔ documented endpoints
request schemas ↔ documented request examples
response schemas ↔ documented responses
status codes ↔ error documentation
auth middleware ↔ documented auth requirements
permissions ↔ docs
path params ↔ docs
query params ↔ docs
examples ↔ actual schema
tests ↔ endpoint registry
client SDK usage ↔ current API
```

Примеры diagnostics:

```text
api_endpoint_missing_in_openapi
api_endpoint_documented_but_not_implemented
api_endpoint_implemented_but_not_documented
api_method_mismatch
api_path_mismatch
api_request_schema_mismatch
api_response_schema_mismatch
api_status_code_undocumented
api_auth_requirement_mismatch
api_permission_mismatch
api_example_invalid
api_error_model_mismatch
api_deprecated_but_still_documented_as_active
api_removed_but_client_still_uses_it
```

---

## 72. API strict mode

API должны быть очень точными. Поэтому нужен strict mode.

Команды:

```bash
ath check api
ath check api --strict
ath check api --fail-on-warning
ath api diff
ath api registry
ath api explain api://POST:/api/login
```

В CI:

```bash
ath check api --strict
```

Если API documentation не совпадает с кодом/OpenAPI, команда должна завершаться ошибкой.

---

## 73. API Contract Snapshot

Athanor должен хранить snapshot публичного API.

```text
api_snapshot
```

Он содержит:

```text
endpoint list
schemas
status codes
auth requirements
permission model
error model
examples
```

Команды:

```bash
ath api snapshot
ath api diff --from snap_old --to snap_new
ath api breaking-changes
```

Breaking changes:

```text
removed endpoint
changed method
changed path
removed response field
changed response type
new required request field
removed status code
auth requirement changed
permission changed
error contract changed
```

Diagnostic:

```text
api_breaking_change_detected
```

---

## 74. API examples validation

Athanor должен проверять примеры API.

Источники примеров:

```text
Markdown docs
OpenAPI examples
curl examples
HTTP files
Postman collections
tests
generated snippets
```

Проверять:

```text
endpoint exists
method/path correct
request body matches schema
response example matches schema
headers documented
auth documented
status code valid
```

Команды:

```bash
ath api examples check
ath api examples generate
ath docs api examples update
```

---

## 75. API docs generation

Athanor должен уметь генерировать API docs из API Registry.

Форматы:

```text
Markdown
OpenAPI
JSONL
HTML report
Rustok dashboard
agent context
```

Команды:

```bash
ath docs api generate --lang ru
ath docs api generate --lang en
ath docs api propose-update
ath docs api check
```

Но генерация должна быть patch-based:

```text
Generated draft → review → apply patch
```

Пример generated API page:

`````md
---
id: doc://docs/ru/api/auth.md
kind: api_documentation
language: ru
entities:
  - api://POST:/api/login
last_verified_snapshot: snap_abc123
---

# API авторизации

## POST /api/login

Назначение: вход пользователя в систему.

### Request

```json
{
  "email": "user@example.com",
  "password": "secret"
}
```

### Responses

* 200 — успешный вход
* 400 — ошибка валидации
* 401 — неверные учётные данные

### Auth

Не требуется.

### Реализация

* `src/routes/auth.rs`
* `symbol://rust:crate::routes::auth::login`

`````

---

## 76. API docs as diagnostics gate

Документация API должна быть не просто текстом, а проверяемым контрактом.

Правило:

```text
Если API изменился, но docs/OpenAPI/examples не обновлены,
Athanor должен создать diagnostic.
```

Severity:

```text
critical — documented endpoint does not exist
critical — implemented endpoint missing from OpenAPI in strict mode
high     — schema mismatch
high     — auth requirement mismatch
medium   — example outdated
medium   — translation outdated
low      — wording/summary stale
```

---

## 77. API consistency with Rustok

В Rustok mode API Registry должен синхронизироваться в Postgres/JSONB.

Таблицы/read-model:

```text
athanor_api_endpoints
athanor_api_schemas
athanor_api_diagnostics
athanor_api_snapshots
athanor_api_examples
athanor_api_docs
```

Это позволит Rustok показывать dashboard:

```text
API Overview
API Consistency
Breaking Changes
Docs Drift
Undocumented Endpoints
Invalid Examples
Translation Drift
```

Но Rustok UI остаётся адаптером. API Registry принадлежит Athanor.

---

## 78. Documentation writer workflow

Athanor должен поддерживать workflow для агента-документатора.

Команды:

```bash
ath docs plan
ath docs missing
ath docs stale
ath docs propose-fix
ath docs apply-patch
ath docs verify
```

Пример workflow:

```text
1. Агент меняет API.
2. Athanor видит изменение.
3. Athanor создаёт diagnostic: api_docs_stale.
4. Агент вызывает ath docs propose-fix.
5. Athanor генерирует patch для ru/en docs и OpenAPI.
6. Агент применяет patch.
7. Athanor выполняет ath check api --strict.
8. Diagnostic закрывается.
```

---

## 79. API precision rule

Для API действует отдельное строгое правило:

```text
API documentation is not descriptive only.
API documentation is a checked contract surface.
```

Любое API-описание должно быть связано с evidence:

```text
код маршрута
OpenAPI operation
schema/type
handler symbol
test/example
documentation section
```

Если evidence нет, API-документация считается неподтверждённой.

Статусы API документации:

```text
verified
partially_verified
stale
missing
conflicting
untrusted
```

---

## 80. Добавить новые сущности в модель

Добавить к доменной модели:

```text
DocumentationPage
DocumentationSection
ApiEndpoint
ApiSchema
ApiExample
ApiSnapshot
ApiContract
ApiDiagnostic
TranslationUnit
Runbook
OperationStep
```

Но физически они могут быть вариантами Entity/Fact/Relation, а не отдельными Rust struct на старте.

Минимально:

```text
EntityKind::ApiEndpoint
EntityKind::ApiSchema
EntityKind::ApiExample
EntityKind::DocumentationPage
EntityKind::DocumentationSection
EntityKind::Runbook
EntityKind::OperationStep
```

---

## 81. Новые relation kinds

```text
documents
documents_api
documents_operation
translation_of
translation_outdated_against
implemented_by
declared_in_openapi
tested_by
example_for
schema_for_request
schema_for_response
requires_auth
requires_permission
contradicts
stale_against
missing_against
```

---

## 82. Новые checkers

```text
ApiImplementationChecker
OpenApiConsistencyChecker
ApiDocsConsistencyChecker
ApiExamplesChecker
ApiBreakingChangesChecker
ApiTranslationDriftChecker
OperationsDocsChecker
EnvDocsChecker
ScriptDocsChecker
RunbookConsistencyChecker
```

---

## 83. Новые projectors

```text
ApiMarkdownProjector
ApiOpenApiProjector
ApiHtmlProjector
OperationsDocsProjector
LocalizedDocsProjector
DocsPatchProjector
RustokApiDashboardProjector
```

---

## 84. Итоговое правило по документации

Athanor должен уметь:

```text
генерировать документацию
проверять документацию
переводить/синхронизировать документацию
предлагать patches
отслеживать устаревание
проверять API как строгий контракт
```

Но не должен:

```text
молча перетирать editable docs
считать generated docs источником истины
скрывать API mismatch как warning в strict mode
генерировать API docs без evidence
```


