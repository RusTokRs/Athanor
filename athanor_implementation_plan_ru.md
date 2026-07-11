# Athanor: план устранения архитектурных и реализационных проблем

> Язык документа: русский  
> Назначение: рабочий implementation plan для последовательного улучшения Athanor  
> Основание: повторный статический аудит текущей ветки `main` репозитория `RusTokRs/Athanor`  
> Статус: draft for implementation

## Статус сверки (2026-07-11)

План повторно сверяется с рабочим деревом, а не закрывается по исходному аудиту.

| Пункт | Фактический статус | Основание сверки / следующий шаг |
| --- | --- | --- |
| 3.1 | частично выполнен | ID-based storage queries и resolver есть; SurrealDB conformance execution ещё отсутствует. |
| 3.2 | частично выполнен | selector committed snapshot есть; требуется завершить conformance всех backend. |
| 3.3 | частично выполнен | JSONL staging/pointer есть; нет coordinated publication canonical/read model/state. |
| 3.4 | частично выполнен | JSONL writer lock, persistent sequence и cleanup известных staging artifacts есть; нет полного lifecycle lock и Surreal transaction. |
| 3.5 | частично выполнен | Tokio process/I/O/timeout есть; нет request cancellation и process-tree termination. |
| 4.1 | частично выполнен | manifest trust, allowlist и executable SHA-256 binding есть; остаются trust v2 policy и sandbox profile. |
| 4.2 | частично выполнен | pipeline проверяет cancellation между фазами; adapter subprocess не получает cancellation token. |
| 4.3 | частично выполнен | shared `Arc<Vec<_>>` и benchmark metrics есть; нет memory budget/per-adapter limits. |
| 4.4 | не выполнен | add/remove всё ещё принудительно запускают full extraction. |
| 4.5 | не выполнен | порты не имеют abort/prepare/batch transaction. |
| 4.6 | не выполнен | `OnceLock` composition factories остаются в `athanor-app`. |
| 4.7 | не выполнен | публичный `pub use ports::*` остаётся wildcard export. |
| 4.8 | не выполнен | `runtime.rs`, `pipeline.rs` и daemon ещё требуют декомпозиции. |
| 4.9 | частично выполнен | добавлены `Conflict`/`SnapshotNotCommitted`; стабильный typed public error/protocol contract не завершён. |
| 4.10 | выполнен | default builds не включают optional SurrealDB adapter; `store-surreal` — явный opt-in feature. |
| 5.1 | не подтверждён | branch protection и hosted checks требуют проверки/настройки GitHub. |
| 5.2 | частично выполнен | есть coverage reports, но нет CI coverage no-regression gate. |
| 5.3 | частично выполнен | security/release workflows есть; полный SAST/secret/SBOM gate не подтверждён. |
| 5.4 | выполнен | strict parsing и `ath config validate`/`doctor` реализованы. |
| 5.5 | частично выполнен | локальные governance/templates/dependabot добавлены; CI policy и release/changelog policy остаются. |

- [-] 3.1: введён низкоуровневый запрос по `EntityId` и отдельный `EntityResolver`; конфликт
  stable key возвращается типизированной ошибкой `Conflict`.
- [-] 3.2: все методы `KnowledgeStore::query_*` требуют `SnapshotSelector`; `Exact` отклоняет
  незакоммиченный snapshot, `LatestCommitted` видит только committed snapshot.
- [-] 3.1–3.2: выделен общий `athanor-store-conformance` suite и он выполняется для JSONL и
  Memory; для полного критерия всё ещё нужен интеграционный запуск того же набора с реальным
  SurrealDB backend.
- [-] 3.5: внешний adapter runner переведён со `std::process` и блокирующих потоков на
  `tokio::process` и async I/O; остаются отдельный `ProcessRunner` порт, отмена по request token
  и гарантированное завершение process tree.
- [-] 3.3: JSONL canonical snapshot полностью записывается в уникальный staging-каталог и
  публикуется rename; pointer публикуется отдельным staged replacement после snapshot. Общий
  coordinator для canonical snapshot, read model и index state ещё не реализован.
- [-] 3.4: JSONL использует OS-level writer lock при выделении и commit snapshot, а persistently
  stored sequence устраняет коллизию ID между отдельными процессами. Нужны lock на полный
  application publication lifecycle и транзакционная модель SurrealDB. При следующем begin JSONL
  удаляет только известные stale staging artifacts под тем же lock.
- [x] 5.4: `athanor.toml` отклоняет неизвестные поля на каждом поддерживаемом уровне конфигурации;
  добавлены проверки top-level и nested полей, а также `ath config validate` и `ath config doctor`
  для effective config и локальной проверки backend/adapter compatibility.
- [-] 5.5: добавлены CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, PR/issue templates, ADR template
  и Dependabot. Conventional-commit enforcement и release/changelog policy в CI остаются открыты.

Проверка baseline: `cargo test --workspace --quiet`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo run -p ath --quiet -- index .` и `cargo run -p ath --quiet -- docs check` проходят. Metadata
GraphQL и coding standards приведены к `verified`, поскольку status policy описывает проверенность
документа, а не завершённость соответствующей продуктовой feature.
- [ ] 3.3–3.5 и разделы P1/P2: остаются в работе и не отмечаются как выполненные без отдельной
  реализации и проверок.

---

## 1. Цель документа

Этот документ фиксирует актуальные архитектурные и реализационные проблемы Athanor и предлагает поэтапный план их устранения.

Главная цель — довести Athanor до состояния, в котором:

- канонические snapshot публикуются атомарно;
- запросы всегда изолированы по конкретному committed snapshot;
- все storage backend имеют одинаковую семантику;
- конкурентные writer-процессы не могут повредить хранилище;
- внешние адаптеры не блокируют async runtime и корректно отменяются;
- pipeline масштабируется на крупные репозитории;
- runtime composition не зависит от глобального состояния;
- публичные API и ошибки типизированы и стабильны;
- CI, security и release gates подтверждают заявленные гарантии.

---

## 2. Что уже исправлено и не требует повторной переработки

По сравнению с предыдущим аудитом часть старых замечаний уже устранена.

### 2.1. Workspace синхронизирован

Текущий корневой `Cargo.toml` включает:

- `apps/ath`;
- `apps/athd`;
- `athanor-runtime-defaults`;
- Tantivy search adapter;
- SurrealDB store;
- GraphQL, JS/TS, Rust, OpenAPI и operations extractors;
- проекторы, linkers, checkers и transport crates.

Проблему старого manifest drift повторно исправлять не требуется.

### 2.2. README обновлён

README теперь описывает:

- canonical knowledge model;
- evidence и ownership;
- incremental snapshots;
- daemon;
- MCP;
- search;
- graph и impact analysis;
- production operation;
- security model.

### 2.3. Production workflow существует

В репозитории уже есть:

- daemon lifecycle tests на Linux и Windows;
- Windows service lifecycle;
- nightly watcher/query soak;
- release workflow с checksum, Sigstore и provenance.

### 2.4. Внешние адаптеры уже частично защищены

Реализованы:

- отключение external process adapters по умолчанию;
- allowlist executable paths;
- user-level manifest trust;
- stdin/stdout/stderr limits;
- wall-clock timeout;
- строгий manifest parsing.

Дальнейшая работа должна усиливать эту модель, а не заменять её полностью.

---

# 3. Критические проблемы P0

## 3.1. Некорректный контракт запросов к хранилищам

### Проблема

`RelationQuery` и `DiagnosticQuery` используют `StableKey`, тогда как канонические связи и диагностики хранят ссылки через `EntityId`.

В результате:

- storage layer смешивает два разных типа идентификаторов;
- JSONL и SurrealDB реализуют фильтрацию по-разному;
- часть фильтров в JSONL фактически игнорируется;
- SurrealDB может сравнивать stable key с полем, содержащим entity id;
- один и тот же application query имеет разную семантику на разных backend.

### Последствия

- неверные результаты relation query;
- неверные результаты diagnostic query;
- невозможность гарантировать взаимозаменяемость backend;
- смешивание исторических и актуальных данных;
- нестабильное поведение daemon/API.

### Целевое решение

Низкоуровневый storage API должен использовать только `EntityId`.

```rust
pub struct RelationQuery {
    pub snapshot: SnapshotSelector,
    pub from_entity: Option<EntityId>,
    pub to_entity: Option<EntityId>,
    pub kind: Option<RelationKind>,
    pub limit: QueryLimit,
}
```

Stable key должен разрешаться application service:

```text
StableKey
  -> EntityResolver
  -> EntityId
  -> KnowledgeQueryStore
```

Нужно добавить отдельный порт:

```rust
#[async_trait]
pub trait EntityResolver {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>>;
}
```

### Acceptance criteria

- JSONL, Memory и SurrealDB возвращают одинаковые результаты;
- relation filtering по `from` и `to` покрыта contract tests;
- diagnostic filtering по entity покрыта contract tests;
- `StableKey` не используется как storage foreign key;
- конфликт нескольких сущностей с одним stable key возвращает typed error.

---

## 3.2. Запросы не изолированы по snapshot

### Проблема

Текущие query-методы не требуют snapshot selector.

Это позволяет:

- JSONL store объединять объекты нескольких snapshot;
- SurrealDB обращаться ко всей таблице без фильтра snapshot;
- получать устаревшие diagnostics;
- получать несколько версий одного entity.

### Целевое решение

Ввести:

```rust
pub enum SnapshotSelector {
    Exact(SnapshotId),
    LatestCommitted,
}
```

Все query операции должны требовать selector.

```rust
#[async_trait]
pub trait KnowledgeQuery {
    async fn query_entities(
        &self,
        snapshot: SnapshotSelector,
        query: EntityQuery,
    ) -> CoreResult<Vec<Entity>>;

    async fn query_relations(
        &self,
        snapshot: SnapshotSelector,
        query: RelationQuery,
    ) -> CoreResult<Vec<Relation>>;

    async fn query_diagnostics(
        &self,
        snapshot: SnapshotSelector,
        query: DiagnosticQuery,
    ) -> CoreResult<Vec<Diagnostic>>;
}
```

### Дополнительные требования

- `load_snapshot` для публичного чтения возвращает только committed snapshot;
- чтение uncommitted snapshot доступно только внутреннему publication flow;
- query без snapshot selector удаляется;
- daemon cache key всегда включает snapshot id.

### Acceptance criteria

- ни один query не смешивает данные разных snapshot;
- uncommitted snapshot невидим пользователю;
- latest означает latest committed;
- одинаковый query на всех backend даёт эквивалентный результат.

---

## 3.3. Публикация индекса не является атомарной

### Проблема

Сейчас pipeline сначала commit-ит canonical snapshot, после чего application layer отдельно пишет:

- JSONL read model;
- index state;
- manifests.

Если запись read model или state завершается ошибкой, canonical latest уже указывает на новый snapshot.

Кроме того, JSONL store и read model writer пишут непосредственно в финальные пути.

### Возможные повреждённые состояния

- `latest.json` указывает на частично записанный snapshot;
- canonical snapshot новый, `index-state.json` старый;
- часть read model новая, часть старая;
- manifest не соответствует содержимому;
- daemon видит snapshot до завершения полной публикации.

### Целевая архитектура

```text
begin publication
    |
    +-- stage canonical snapshot
    +-- validate canonical data
    +-- stage read model
    +-- stage index state
    +-- stage manifests/checksums
    |
prepare publication
    |
atomic commit
    1. rename canonical staging -> immutable snapshot
    2. rename read model staging -> immutable generation
    3. replace index state
    4. replace latest/current pointers last
```

Нужен publication coordinator:

```rust
#[async_trait]
pub trait IndexPublication {
    async fn prepare(
        &self,
        input: PublicationInput,
    ) -> Result<PreparedPublication>;

    async fn commit(
        &self,
        prepared: PreparedPublication,
    ) -> Result<PublishedIndex>;

    async fn abort(
        &self,
        prepared: PreparedPublication,
    ) -> Result<()>;
}
```

### Ключевое правило

`KnowledgeStore::commit_snapshot` не должен самостоятельно переключать глобальный latest pointer.

Latest/current pointer переключает только `PublicationCoordinator`, когда готовы все артефакты.

### Acceptance criteria

После ошибки в любой точке публикации видна либо:

- предыдущая полностью валидная generation;
- новая полностью валидная generation.

Смешанное состояние запрещено.

---

## 3.4. Конкурентные writers могут повредить storage

### JSONL

Номер snapshot вычисляется сканированием директорий и защищён только внутрипроцессным `Mutex`.

Два процесса могут одновременно выбрать один snapshot id и писать в одну директорию.

### SurrealDB

Counter обновляется через:

```text
select
-> increment in memory
-> update
```

Это не атомарная операция.

### Целевое решение для JSONL

- один OS-level writer lock на проект;
- lock держится на протяжении prepare/commit;
- snapshot ID выделяется под lock;
- staging directory уникальна;
- immutable snapshot directory создаётся через atomic rename;
- latest pointer заменяется атомарно;
- startup recovery удаляет только известные staging artifacts.

### Целевое решение для SurrealDB

- atomic sequence или transactional increment;
- unique snapshot id;
- begin/write/prepare/commit в транзакционной модели;
- committed visibility;
- optimistic concurrency или writer lease.

### Application-level coordination

Создать `IndexCoordinator`:

- один write job на проект;
- параллельные read-only query разрешены;
- повторный index получает `Busy` или ставится в bounded queue;
- watcher не создаёт бесконечную очередь одинаковых job.

### Acceptance criteria

- два одновременных index процесса не повреждают данные;
- duplicate snapshot id невозможен;
- read queries работают во время indexing;
- только committed snapshot виден query layer.

---

## 3.5. External process adapters блокируют Tokio runtime

### Проблема

Асинхронные adapter methods вызывают синхронный process runner:

- `std::process::Command`;
- `std::thread`;
- `try_wait`;
- `thread::sleep`;
- blocking `join`.

Медленный plugin занимает Tokio worker thread.

При нескольких process adapters это может заблокировать:

- daemon ping;
- cancellation;
- watcher;
- остальные async operations.

### Целевое решение

Использовать:

```rust
tokio::process::Command
tokio::io::AsyncReadExt
tokio::time::timeout
tokio::select!
CancellationToken
```

Создать порт:

```rust
#[async_trait]
pub trait ProcessRunner: Send + Sync {
    async fn run(
        &self,
        request: ProcessRequest,
        context: OperationContext,
    ) -> Result<ProcessOutput, ProcessRunError>;
}
```

### Дополнительные требования

- завершать process tree;
- Unix: отдельная process group;
- Windows: Job Object;
- cancellation завершает child и descendants;
- stdin закрывается после записи;
- environment очищается;
- разрешены только явно переданные environment variables;
- `current_dir` задаётся явно;
- timeout и byte limits применяются асинхронно;
- stderr не смешивается с JSON protocol.

### Временное решение

Допустимо временно обернуть старый runner в `spawn_blocking`, но это не финальная архитектура.

### Acceptance criteria

- медленный plugin не блокирует daemon ping;
- cancellation останавливает весь process tree;
- timeout не оставляет orphan process;
- oversized output не вызывает unbounded allocation;
- protocol output остаётся валидным и отделённым от stderr.

---

# 4. Высокоприоритетные проблемы P1

## 4.1. Trust проверяет manifest, но не executable

### Проблема

Trust record привязан к:

- canonical manifest path;
- manifest SHA-256.

Но содержимое executable по разрешённому пути не проверяется.

Доверенный бинарник можно заменить без изменения manifest.

### Целевое решение

Trust record v2:

```json
{
  "schema": "athanor.adapter_trust.v2",
  "manifest_path": "...",
  "manifest_sha256": "...",
  "executables": [
    {
      "canonical_path": "...",
      "sha256": "...",
      "size": 12345
    }
  ]
}
```

Перед запуском:

- canonicalize executable path;
- проверить digest;
- проверить размер;
- проверить, что путь не ушёл из разрешённой директории;
- проверить symlink substitution;
- при изменении требовать повторный trust.

### Дополнительное усиление

Опциональный sandbox profile:

- no network;
- read-only repository;
- isolated temp directory;
- CPU limit;
- memory limit;
- process count limit;
- seccomp/namespaces на Linux;
- Job Object/restricted token на Windows.

---

## 4.2. Cancellation работает только между крупными фазами

### Проблема

Pipeline проверяет cancellation между:

- discovery;
- extraction;
- linking;
- checking;
- storage.

Но adapter contracts cancellation token не получают.

### Целевое решение

Ввести общий execution context:

```rust
pub struct OperationContext {
    pub cancellation: CancellationToken,
    pub deadline: Option<Instant>,
    pub trace_id: TraceId,
    pub limits: ResourceLimits,
}
```

Добавить context в:

- source discovery;
- extractor input;
- linker input;
- checker input;
- store writes;
- projector execution;
- external process runner.

### Contract tests

Каждый adapter должен проверяться на:

- cancellation до запуска;
- cancellation во время выполнения;
- deadline exceeded;
- отсутствие publication после cancellation.

---

## 4.3. Pipeline расходует слишком много памяти

### Проблема

Pipeline:

- заранее создаёт полный список extraction tasks;
- клонирует `SourceFile` для каждого extractor;
- хранит весь source content в `String`;
- клонирует extracted objects;
- клонирует affected subset;
- одновременно держит старые, новые и canonicalized коллекции.

### Целевая модель source

```rust
pub struct SourceFile {
    pub path: Arc<str>,
    pub content_hash: ContentHash,
    pub language_hint: Option<LanguageId>,
    pub content: SourceContent,
}

pub enum SourceContent {
    Inline(Arc<[u8]>),
    Lazy(SourceHandle),
}
```

### Изменения pipeline

- генерировать extraction stream лениво;
- не собирать все tasks в `Vec`;
- concurrency задавать конфигурацией;
- ввести memory budget;
- ввести per-adapter concurrency;
- affected subset хранить через ids/indices/Arc;
- использовать batch output;
- по возможности писать output потоково.

### Acceptance criteria

- большой source не копируется для каждого extractor;
- peak RSS измеряется benchmark;
- есть configurable CPU и memory limits;
- small incremental update не создаёт полный in-memory duplicate snapshot.

---

## 4.4. Добавление или удаление файла вызывает полный rebuild

### Проблема

Любой add/remove принудительно переводит все файлы в changed.

Это безопасно, но плохо масштабируется.

### Целевое решение

Каждый adapter декларирует invalidation policy:

```rust
pub enum InvalidationScope {
    FileLocal,
    DependencyClosure,
    GlobalOnAdd,
    GlobalOnRemove,
    AlwaysGlobal,
}
```

Примеры:

- локальный extractor — `FileLocal`;
- duplicate-id checker — `GlobalOnAdd | GlobalOnRemove`;
- GraphQL fragment linker — `DependencyClosure`;
- absence checker — global только для конкретного entity kind.

Создать `InvalidationPlanner`:

```text
changed files
  -> changed canonical keys
  -> impacted adapters
  -> impacted entity kinds
  -> dependency closure
  -> relink/recheck subset
```

Adapters без declaration временно используют `AlwaysGlobal`.

---

## 4.5. Storage port не имеет rollback и batch transaction

### Проблема

Текущий port имеет только:

- `begin_snapshot`;
- `put_entities`;
- `put_facts`;
- `put_relations`;
- `put_diagnostics`;
- `commit_snapshot`.

Нет:

- abort;
- prepare;
- transaction handle;
- batch limits;
- partial failure policy.

### Целевой контракт

```rust
#[async_trait]
pub trait SnapshotWriter {
    async fn begin(
        &self,
        request: BeginSnapshot,
    ) -> Result<Box<dyn SnapshotTransaction>>;
}

#[async_trait]
pub trait SnapshotTransaction {
    async fn write_batch(
        &mut self,
        batch: CanonicalBatch,
    ) -> Result<()>;

    async fn prepare(
        self: Box<Self>,
    ) -> Result<Box<dyn PreparedSnapshot>>;

    async fn abort(
        self: Box<Self>,
    ) -> Result<()>;
}

#[async_trait]
pub trait PreparedSnapshot {
    async fn commit(
        self: Box<Self>,
    ) -> Result<CommittedSnapshot>;
}
```

Batch должен быть ограничен:

- числом объектов;
- количеством байтов;
- deadline;
- memory budget.

---

## 4.6. Глобальные `OnceLock` создают скрытое состояние

### Проблема

Composition устанавливается через глобальные factories и registries.

Повторная установка silently ignored.

### Последствия

- порядок инициализации скрыт;
- тесты зависят от process-global state;
- нельзя создать два runtime с разными adapter sets;
- embedded usage затруднён;
- ошибка install не видна.

### Целевое решение

```rust
pub struct RuntimeComposition {
    pub adapter_registry: AdapterRegistry,
    pub store_factory: Arc<dyn StoreFactory>,
    pub search_factory: Arc<dyn SearchFactory>,
    pub projectors: ProjectorRegistry,
    pub process_runner: Arc<dyn ProcessRunner>,
}
```

Использование:

```rust
let composition = RuntimeComposition::production();
let app = AthanorApplication::new(composition);
```

Профили:

```rust
RuntimeComposition::production()
RuntimeComposition::minimal()
RuntimeComposition::test()
RuntimeComposition::with_store(...)
```

Глобальный `install()` сначала объявить deprecated, затем удалить.

---

## 4.7. `athanor-app` экспортирует слишком много

### Проблема

`athanor-app` делает wildcard re-export большого количества модулей.

### Последствия

- внутренние типы становятся случайным публичным API;
- сложно определить стабильные boundaries;
- рефакторинг ломает consumers;
- документация crate перегружена.

### Целевое решение

Оставить namespaces:

```text
athanor_app::indexing
athanor_app::query
athanor_app::publication
athanor_app::runtime
athanor_app::daemon
athanor_app::projects
```

Экспортировать явно:

```rust
pub use indexing::{IndexService, IndexRequest, IndexReport};
pub use runtime::{RuntimeComposition, RuntimeBuilder};
```

Внутренние типы перевести на `pub(crate)`.

---

## 4.8. `runtime.rs`, `pipeline.rs` и daemon являются God modules

### Рекомендуемая декомпозиция `athd`

```text
apps/athd/src/
    main.rs
    cli.rs
    output.rs
    client_commands.rs
    server_command.rs
    background.rs
    doctor.rs
    logging.rs
    service/
        mod.rs
        linux.rs
        windows.rs
```

### Декомпозиция runtime

```text
crates/athanor-app/src/runtime/
    mod.rs
    composition.rs
    registry.rs
    plugin_manifest.rs
    plugin_discovery.rs
    plugin_trust.rs
    process_runner.rs
    process_limits.rs
```

### Декомпозиция indexing

```text
crates/athanor-app/src/indexing/
    mod.rs
    pipeline.rs
    discovery.rs
    extraction.rs
    linking.rs
    checking.rs
    merge.rs
    validation.rs
    metrics.rs
    invalidation.rs
```

### Правило

Сначала выполнять внутреннюю декомпозицию модулей, а не создавать большое количество новых crates без необходимости.

---

## 4.9. Ошибки недостаточно типизированы

### Проблема

Большинство ошибок сводится к строкам.

Daemon response также содержит только строковый `error`.

### Целевой error code contract

```rust
pub enum AthanorErrorCode {
    InvalidInput,
    NotFound,
    Conflict,
    Busy,
    Unauthorized,
    Forbidden,
    Cancelled,
    DeadlineExceeded,
    AdapterProtocol,
    AdapterExecution,
    StorageUnavailable,
    StorageCorruption,
    SnapshotNotCommitted,
    Unsupported,
    Internal,
}
```

Daemon error:

```json
{
  "ok": false,
  "error": {
    "code": "snapshot_not_committed",
    "message": "Snapshot is not committed",
    "retryable": false,
    "details": {}
  }
}
```

### Требования

- human-readable context сохраняется внутри application layer;
- public API возвращает stable error codes;
- sensitive paths и raw stderr не выдаются неподходящим клиентам;
- retryable ошибки помечаются явно.

---

## 4.10. SurrealDB собирается по умолчанию

### Проблема

Default runtime feature включает SurrealDB, хотя default storage mode — JSONL.

### Последствия

- увеличенный build time;
- большой dependency graph;
- BUSL dependency в default build;
- больший release footprint;
- больше platform-specific рисков.

### Целевое решение

```toml
[features]
default = []
store-surreal = ["dep:athanor-store-surrealdb"]
js-ts-precision = [...]
full = ["store-surreal", "js-ts-precision"]
```

Release profiles:

- minimal;
- full;
- optional backend-specific builds.

---

# 5. Проблемы P2

## 5.1. Hosted CI не полностью подтверждён

### Работы

- сделать required checks обязательными;
- запретить direct push в `main`;
- требовать актуальную ветку перед merge;
- синхронизировать README и roadmap;
- публиковать test/coverage artifacts;
- добавить weekly full-feature matrix.

Обязательные checks:

```text
ci / security
ci / ubuntu
ci / windows
ci / macos
production / linux-daemon
production / windows-daemon
```

---

## 5.2. Нет measurable coverage gate

### План

1. Добавить `cargo-llvm-cov`.
2. Сначала фиксировать baseline.
3. Затем включить no-regression.
4. Добавить changed-lines coverage.
5. Для core/store/publication целиться минимум в 90%.
6. Для parser adapters измерять fixture/construct coverage.

---

## 5.3. Недостаточная AppSec автоматизация

### Добавить

- CodeQL или другой SAST с SARIF;
- dependency review;
- gitleaks;
- workflow security lint, например `zizmor`;
- `cargo-geiger`;
- unsafe policy;
- SHA pinning GitHub Actions;
- SBOM;
- подпись SBOM;
- release verification job.

---

## 5.4. Config молча принимает неизвестные поля

### Исправление

Использовать:

```rust
#[serde(default, deny_unknown_fields)]
```

И добавить:

```text
ath config validate
ath config doctor
```

Команды должны показывать:

- unknown fields;
- deprecated fields;
- effective config;
- feature availability;
- backend compatibility.

---

## 5.5. Open-source governance не завершена

### Добавить

- `CONTRIBUTING.md`;
- `SECURITY.md`;
- `CODE_OF_CONDUCT.md`;
- PR template;
- issue templates;
- ADR template;
- Dependabot или Renovate;
- conventional commits;
- release checklist;
- changelog policy.

Commit messages вида `fix` следует запретить policy и CI-lint.

---

# 6. Целевая архитектура

```text
CLI / Daemon / MCP
        |
        v
AthanorApplication
        |
        +-- IndexService
        +-- QueryService
        +-- DocumentationService
        +-- GraphService
        +-- RepairService
        |
        v
IndexCoordinator
        |
        +-- InvalidationPlanner
        +-- ExtractionExecutor
        +-- LinkingExecutor
        +-- CheckingExecutor
        +-- CanonicalValidator
        +-- PublicationCoordinator
        |
        v
Ports
    SnapshotReader
    SnapshotWriter
    EntityResolver
    KnowledgeQuery
    SearchIndex
    Projector
    ProcessRunner
        |
        v
Adapters
    JSONL
    SurrealDB
    File system source
    Extractors
    Linkers
    Checkers
    Tantivy
    Wiki/HTML projectors
    External process adapters
```

## Архитектурные правила

1. Pipeline вычисляет результат.
2. Publication coordinator публикует результат.
3. Storage гарантирует snapshot isolation.
4. Query layer всегда использует committed snapshot.
5. Stable key разрешается в entity id вне storage query.
6. Concrete adapters не импортируются application services.
7. Runtime composition передаётся явно.
8. Read models остаются disposable projections.
9. Canonical snapshot остаётся source of truth.
10. Agent-facing API всегда bounded и evidence-backed.

---

# 7. Поэтапный roadmap

## Этап 0. Зафиксировать baseline

**Приоритет:** P0  
**Сложность:** S

### Работы

- запустить полный CI на текущем `main`;
- зафиксировать failing steps;
- включить branch protection;
- добавить `cargo metadata --locked`;
- добавить minimal feature build;
- синхронизировать README и roadmap;
- создать ADR текущего baseline.

### Definition of Done

- required checks отображаются на каждом PR;
- merge без checks невозможен;
- build воспроизводим из чистого checkout;
- фактический статус CI отражён в документации.

---

## Этап 1. Исправить query и snapshot contracts

**Приоритет:** P0  
**Сложность:** L

### Области изменений

```text
crates/athanor-core
crates/athanor-store-jsonl
crates/athanor-store-memory
crates/athanor-store-surrealdb
crates/athanor-app query services
```

### Работы

- добавить `SnapshotSelector`;
- убрать stable key из low-level relation/diagnostic query;
- добавить `EntityResolver`;
- разделить reader/writer/query ports;
- запретить чтение uncommitted snapshot;
- исправить фильтры JSONL;
- исправить фильтры SurrealDB;
- создать общий store conformance suite.

### Contract tests

- latest возвращает только committed snapshot;
- exact snapshot не смешивает историю;
- relation `from`/`to` работает;
- diagnostic entity filter работает;
- stable key разрешается детерминированно;
- backend возвращают эквивалентные результаты;
- uncommitted snapshot невидим.

---

## Этап 2. Атомарная публикация

**Приоритет:** P0  
**Сложность:** XL

### Работы

- добавить abort/prepare/commit lifecycle;
- добавить staging canonical snapshot;
- добавить staging read model;
- атомарно сохранять index state;
- переключать pointers последними;
- добавить writer lock;
- добавить startup recovery;
- добавить SurrealDB transaction model;
- перенести latest pointer в publication coordinator.

### Fault injection matrix

```text
fail after begin
fail after entities
fail after facts
fail after relations
fail before manifest
fail before snapshot rename
fail after snapshot rename
fail before current pointer
fail after current pointer
```

### Definition of Done

После любого сбоя доступна только последняя полная валидная generation.

---

## Этап 3. Async external process runner

**Приоритет:** P0  
**Сложность:** L

### Работы

- создать `ProcessRunner`;
- перейти на Tokio process API;
- добавить cancellation;
- добавить deadline;
- завершать process tree;
- очистить environment;
- добавить executable trust v2;
- добавить protocol version negotiation;
- типизировать adapter errors;
- расширить metrics;
- добавить optional sandbox profile.

### Definition of Done

- process adapter не блокирует runtime;
- cancellation завершает descendants;
- изменение executable инвалидирует trust;
- timeout не оставляет orphan process;
- limits работают без unbounded memory.

---

## Этап 4. Оптимизация pipeline

**Приоритет:** P1  
**Сложность:** XL

### Работы

- Arc/lazy source content;
- lazy task stream;
- configurable concurrency;
- memory budget;
- per-adapter concurrency;
- affected subset через ids/Arc;
- batch output;
- invalidation declarations;
- dependency-closure invalidation;
- performance budgets.

### Benchmark matrix

```text
10k files
50k files
100k files
large single file
multiple extractors per file
many cross-file relations
slow external adapter
single-file incremental update
file add/remove
```

### Метрики

- peak RSS;
- allocations;
- total duration;
- extraction/link/check/storage/publication timing;
- incremental/full ratio.

---

## Этап 5. Убрать глобальный composition state

**Приоритет:** P1  
**Сложность:** M-L

### Работы

- добавить `RuntimeComposition`;
- передавать composition в CLI/daemon/MCP;
- удалить silent `OnceLock::set`;
- убрать дублирование registry/resolver;
- добавить production/minimal/test profiles;
- поддержать несколько runtime в одном процессе;
- добавить integration tests.

---

## Этап 6. Декомпозиция application layer

**Приоритет:** P1  
**Сложность:** L

### Работы

- убрать wildcard re-exports;
- зафиксировать public API;
- разделить runtime;
- разделить pipeline;
- разделить daemon;
- отделить protocol/server/jobs/cache/watcher/service management;
- добавить dependency-boundary tests.

### Dependency rules

```text
domain <- core ports <- application services <- composition/entrypoints
adapters -> core/domain
entrypoints -> application/composition
application services must not import concrete adapters
```

---

## Этап 7. Typed errors и protocol v3

**Приоритет:** P1  
**Сложность:** M

### Работы

- ввести error codes;
- отделить internal context от public error;
- обновить daemon schema;
- обновить MCP mapping;
- добавить retryable flag;
- защитить sensitive details;
- сохранить временную совместимость с protocol v2.

---

## Этап 8. Security и supply chain

**Приоритет:** P2  
**Сложность:** M-L

### Работы

- pin actions по SHA;
- automated update bot;
- dependency review;
- SAST;
- secret scan;
- workflow security lint;
- unsafe report;
- `SECURITY.md`;
- threat model;
- SurrealDB opt-in;
- ужесточение `deny.toml`;
- SBOM;
- подпись SBOM;
- release verify job.

---

## Этап 9. Test strategy и quality gates

**Приоритет:** P2  
**Сложность:** L

### Unit tests

- domain invariants;
- stable ids;
- config;
- invalidation;
- error mapping.

### Contract tests

- storage backend;
- external adapters;
- projectors;
- search;
- daemon protocol.

### Integration tests

- index -> query;
- index -> generate;
- incremental update;
- cancellation;
- crash recovery;
- concurrent readers/writer;
- daemon during indexing.

### Property tests

- canonicalization idempotency;
- deterministic ordering;
- stable id determinism;
- serialization round-trip;
- arbitrary query filters.

### Fault tests

- partial writes;
- corrupted manifests;
- stale pointers;
- missing snapshot;
- killed adapter;
- database conflict;
- disk full simulation.

---

## Этап 10. Documentation и release discipline

**Приоритет:** P2  
**Сложность:** S-M

### Работы

- contributor/security/governance docs;
- ADR process;
- commit message policy;
- release readiness checklist;
- roadmap consistency check;
- compatibility matrix;
- migration notes для schema/protocol changes.

---

# 8. Рекомендуемая разбивка по Pull Request

1. **PR-1:** CI truth, branch protection, documentation status.
2. **PR-2:** store conformance test harness.
3. **PR-3:** typed query contract и snapshot selector.
4. **PR-4:** JSONL query semantics.
5. **PR-5:** SurrealDB query semantics.
6. **PR-6:** abort/prepare/commit storage API.
7. **PR-7:** JSONL atomic staging и writer lock.
8. **PR-8:** coordinated publication canonical/read-model/state.
9. **PR-9:** SurrealDB transaction и atomic sequence.
10. **PR-10:** async process runner.
11. **PR-11:** process cancellation и executable trust v2.
12. **PR-12:** lazy source content и memory budgets.
13. **PR-13:** adapter invalidation capabilities.
14. **PR-14:** explicit runtime composition.
15. **PR-15:** module decomposition и explicit exports.
16. **PR-16:** typed errors и protocol v3.
17. **PR-17:** security, coverage и release gates.

## Правило PR

Каждый PR должен:

- оставлять workspace собираемым;
- иметь unit/contract/integration tests;
- обновлять релевантную документацию;
- не смешивать архитектурный перенос с добавлением новых extractor features;
- содержать verification commands и результаты;
- явно перечислять deferred limitations.

---

# 9. Definition of Done проекта улучшений

Athanor можно считать инженерно зрелым, когда:

- каждый query привязан к committed snapshot;
- JSONL, Memory и SurrealDB проходят один conformance suite;
- два конкурентных index job не повреждают storage;
- crash во время publication оставляет последнюю полную generation;
- external adapter не блокирует Tokio runtime;
- cancellation проходит до adapter и subprocess;
- trust связан с manifest и executable;
- pipeline имеет измеримый memory budget;
- добавление файла не вызывает безусловный full rebuild;
- runtime не зависит от глобальных `OnceLock`;
- публичный API не состоит из wildcard re-export;
- daemon возвращает typed errors;
- hosted CI checks реально обязательны;
- coverage, contract, crash, concurrency и security gates включены;
- minimal build не тянет SurrealDB;
- документация, roadmap и фактический статус не противоречат друг другу.

---

# 10. Первая рекомендуемая задача

Первым крупным техническим направлением следует выбрать:

> **Исправление snapshot/query contract и создание общего conformance suite для storage backend.**

Причина: текущие проблемы в storage query способны выдавать логически неверные результаты даже при успешной сборке и прохождении обычных тестов.

Декомпозицию файлов и косметический рефакторинг следует выполнять после фиксации корректной семантики snapshot и query.

---

# 11. Чек-лист начала реализации

- [ ] Зафиксирован green/red baseline текущего `main`.
- [ ] Создан ADR по snapshot/query semantics.
- [ ] Создан общий store conformance test crate/module.
- [ ] Определён `SnapshotSelector`.
- [ ] Определён `EntityResolver`.
- [ ] Определены новые reader/writer/query ports.
- [ ] Зафиксирована atomic publication state machine.
- [ ] Определена fault-injection matrix.
- [ ] Определён migration plan для daemon/MCP schema.
- [ ] PR-1 и PR-2 созданы до начала массового рефакторинга.
