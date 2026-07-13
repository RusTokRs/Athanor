use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

/// Transport-neutral operation metadata propagated across adapter boundaries.
///
/// Application layers may attach a stable operation id for logs and a wall-clock deadline. The
/// context deliberately does not contain application cancellation primitives, so adapters and
/// stores remain reusable outside a particular runtime.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_unix_ms: Option<u64>,
}

impl OperationContext {
    pub fn new(operation_id: impl Into<String>) -> Self {
        Self {
            operation_id: Some(operation_id.into()),
            deadline_unix_ms: None,
        }
    }

    pub fn with_deadline_unix_ms(mut self, deadline_unix_ms: u64) -> Self {
        self.deadline_unix_ms = Some(deadline_unix_ms);
        self
    }

    pub fn remaining(&self) -> Option<Duration> {
        self.deadline_unix_ms
            .map(|deadline| Duration::from_millis(deadline.saturating_sub(current_unix_ms())))
    }

    pub fn check_deadline(&self) -> CoreResult<()> {
        if self
            .deadline_unix_ms
            .is_some_and(|deadline| current_unix_ms() >= deadline)
        {
            let operation = self.operation_id.as_deref().unwrap_or("operation");
            return Err(CoreError::DeadlineExceeded(format!(
                "{operation} exceeded its configured deadline"
            )));
        }
        Ok(())
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

/// Stable machine-readable categories for errors crossing application boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreErrorCode {
    NotFound,
    InvalidInput,
    AdapterProtocol,
    AdapterExecution,
    SnapshotNotCommitted,
    Conflict,
    Busy,
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("adapter error: {0}")]
    Adapter(String),
    #[error("adapter protocol error: {0}")]
    AdapterProtocol(String),
    #[error("snapshot is not committed: {0}")]
    SnapshotNotCommitted(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("busy: {0}")]
    Busy(String),
    #[error("cancelled: {0}")]
    Cancelled(String),
    #[error("deadline exceeded: {0}")]
    DeadlineExceeded(String),
}

impl CoreError {
    pub fn code(&self) -> CoreErrorCode {
        match self {
            Self::NotFound(_) => CoreErrorCode::NotFound,
            Self::InvalidInput(_) => CoreErrorCode::InvalidInput,
            Self::AdapterProtocol(_) => CoreErrorCode::AdapterProtocol,
            Self::Adapter(_) => CoreErrorCode::AdapterExecution,
            Self::SnapshotNotCommitted(_) => CoreErrorCode::SnapshotNotCommitted,
            Self::Conflict(_) => CoreErrorCode::Conflict,
            Self::Busy(_) => CoreErrorCode::Busy,
            Self::Cancelled(_) => CoreErrorCode::Cancelled,
            Self::DeadlineExceeded(_) => CoreErrorCode::DeadlineExceeded,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Busy(_) | Self::DeadlineExceeded(_))
    }
}

/// Selects the committed canonical snapshot visible to a query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapshotSelector {
    Exact(SnapshotId),
    LatestCommitted,
}

/// Declares how an adapter must be rerun after a source-file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidationScope {
    FileLocal,
    DependencyClosure,
    GlobalOnAdd,
    GlobalOnRemove,
    AlwaysGlobal,
}

/// Conservative invalidation declaration for one adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidationPolicy {
    pub on_change: InvalidationScope,
    pub on_add: InvalidationScope,
    pub on_remove: InvalidationScope,
}

/// A bounded request to execute one explicitly resolved external process.
///
/// The caller must supply an absolute executable and working directory. Implementations must not
/// search `PATH` or implicitly inherit a working directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessRequest {
    /// Human-readable adapter/process identity used only in bounded diagnostic messages.
    pub label: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    /// Removes inherited environment variables before spawning when explicitly requested by the
    /// application sandbox policy. The executable and working directory remain explicit.
    #[serde(default)]
    pub clear_environment: bool,
    pub stdin: Vec<u8>,
    pub limits: ProcessLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessLimits {
    pub timeout_ms: u64,
    pub max_stdin_bytes: usize,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            max_stdin_bytes: 8 * 1024 * 1024,
            max_stdout_bytes: 8 * 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

/// Executes explicitly requested external processes under bounded I/O and time limits.
#[async_trait]
pub trait ProcessRunner: Send + Sync {
    async fn run(&self, request: ProcessRequest) -> CoreResult<ProcessOutput>;
}

impl InvalidationPolicy {
    pub const ALWAYS_GLOBAL: Self = Self {
        on_change: InvalidationScope::AlwaysGlobal,
        on_add: InvalidationScope::AlwaysGlobal,
        on_remove: InvalidationScope::AlwaysGlobal,
    };

    pub const FILE_LOCAL: Self = Self {
        on_change: InvalidationScope::FileLocal,
        on_add: InvalidationScope::FileLocal,
        on_remove: InvalidationScope::FileLocal,
    };
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityQuery {
    pub stable_key: Option<StableKey>,
    pub kind: Option<String>,
    pub text: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationQuery {
    pub from_entity: Option<EntityId>,
    pub to_entity: Option<EntityId>,
    pub kind: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiagnosticQuery {
    pub severity: Option<String>,
    pub status: Option<String>,
    pub entity: Option<EntityId>,
    pub limit: Option<usize>,
}

#[async_trait]
pub trait KnowledgeStore: Send + Sync {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId>;
    async fn begin_snapshot_with_context(
        &self,
        repo: RepoId,
        base: SnapshotBase,
        context: &OperationContext,
    ) -> CoreResult<SnapshotId> {
        context.check_deadline()?;
        self.begin_snapshot(repo, base).await
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()>;
    async fn put_entities_with_context(
        &self,
        snapshot: SnapshotId,
        entities: Vec<Entity>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_deadline()?;
        self.put_entities(snapshot, entities).await
    }
    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()>;
    async fn put_facts_with_context(
        &self,
        snapshot: SnapshotId,
        facts: Vec<Fact>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_deadline()?;
        self.put_facts(snapshot, facts).await
    }
    async fn put_relations(&self, snapshot: SnapshotId, relations: Vec<Relation>)
    -> CoreResult<()>;
    async fn put_relations_with_context(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_deadline()?;
        self.put_relations(snapshot, relations).await
    }
    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()>;
    async fn put_diagnostics_with_context(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_deadline()?;
        self.put_diagnostics(snapshot, diagnostics).await
    }

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

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()>;
    async fn commit_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_deadline()?;
        self.commit_snapshot(snapshot).await
    }
    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()>;
    async fn abort_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_deadline()?;
        self.abort_snapshot(snapshot).await
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanonicalSnapshot {
    pub snapshot: Option<SnapshotId>,
    pub entities: Vec<Entity>,
    pub facts: Vec<Fact>,
    pub relations: Vec<Relation>,
    pub diagnostics: Vec<Diagnostic>,
}

#[async_trait]
pub trait CanonicalSnapshotStore: Send + Sync {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>>;
    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: String,
    pub language_hint: Option<String>,
    pub content_hash: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractInput {
    pub repo: RepoId,
    pub snapshot: SnapshotId,
    pub source: SourceFile,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractOutput {
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub facts: Vec<Fact>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

/// Resolves an external stable key before issuing an ID-based storage query.
#[async_trait]
pub trait EntityResolver: Send + Sync {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>>;
}

#[async_trait]
pub trait SourceProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn discover(&self) -> CoreResult<Vec<SourceFile>>;
    async fn discover_with_context(
        &self,
        context: &OperationContext,
    ) -> CoreResult<Vec<SourceFile>> {
        context.check_deadline()?;
        self.discover().await
    }
}

#[async_trait]
pub trait Extractor: Send + Sync {
    fn name(&self) -> &str;
    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy::ALWAYS_GLOBAL
    }
    fn supports(&self, source: &SourceFile) -> bool;
    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput>;
    async fn extract_with_context(
        &self,
        input: ExtractInput,
        context: &OperationContext,
    ) -> CoreResult<ExtractOutput> {
        context.check_deadline()?;
        self.extract(input).await
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AffectedSubset {
    pub entities: Vec<Entity>,
    pub facts: Vec<Fact>,
    pub relations: Vec<Relation>,
}

impl AffectedSubset {
    pub fn from_extracted(entities: Vec<Entity>, facts: Vec<Fact>) -> Self {
        Self {
            entities,
            facts,
            relations: Vec::new(),
        }
    }

    pub fn with_relations(mut self, relations: Vec<Relation>) -> Self {
        self.relations = relations;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkInput {
    pub snapshot: SnapshotId,
    pub entities: Arc<Vec<Entity>>,
    pub facts: Arc<Vec<Fact>>,
    pub affected: AffectedSubset,
}

#[async_trait]
pub trait Linker: Send + Sync {
    fn name(&self) -> &str;
    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy::ALWAYS_GLOBAL
    }
    async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>>;
    async fn link_with_context(
        &self,
        input: LinkInput,
        context: &OperationContext,
    ) -> CoreResult<Vec<Relation>> {
        context.check_deadline()?;
        self.link(input).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckInput {
    pub snapshot: SnapshotId,
    pub entities: Arc<Vec<Entity>>,
    pub facts: Arc<Vec<Fact>>,
    pub relations: Arc<Vec<Relation>>,
    pub affected: AffectedSubset,
}

#[async_trait]
pub trait Checker: Send + Sync {
    fn name(&self) -> &str;
    fn invalidation_policy(&self) -> InvalidationPolicy {
        InvalidationPolicy::ALWAYS_GLOBAL
    }
    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>>;
    async fn check_with_context(
        &self,
        input: CheckInput,
        context: &OperationContext,
    ) -> CoreResult<Vec<Diagnostic>> {
        context.check_deadline()?;
        self.check(input).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInput {
    pub snapshot: SnapshotId,
    pub target: String,
    pub payload: Value,
}

#[async_trait]
pub trait Projector: Send + Sync {
    fn name(&self) -> &str;
    async fn project(&self, input: ProjectInput) -> CoreResult<()>;
    async fn project_with_context(
        &self,
        input: ProjectInput,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_deadline()?;
        self.project(input).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDocument {
    pub id: String,
    pub title: String,
    pub body: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub payload: Value,
}

#[async_trait]
pub trait SearchIndex: Send + Sync {
    async fn index_document(&self, doc: SearchDocument) -> CoreResult<()>;
    async fn remove_document(&self, id: &str) -> CoreResult<()>;
    async fn search(&self, query: SearchQuery) -> CoreResult<Vec<SearchResult>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingInput {
    pub text: String,
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, input: EmbeddingInput) -> CoreResult<Vec<f32>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorItem {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorQuery {
    pub vector: Vec<f32>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub id: String,
    pub score: f32,
    pub payload: Value,
}

#[async_trait]
pub trait VectorIndex: Send + Sync {
    async fn upsert(&self, item: VectorItem) -> CoreResult<()>;
    async fn search(&self, query: VectorQuery) -> CoreResult<Vec<VectorSearchResult>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub command: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub payload: Value,
}

#[async_trait]
pub trait AgentInterface: Send + Sync {
    async fn handle(&self, request: AgentRequest) -> CoreResult<AgentResponse>;
}

#[async_trait]
pub trait Transport: Send + Sync {
    fn name(&self) -> &str;
    async fn serve(&self) -> CoreResult<()>;
}

#[cfg(test)]
mod tests {
    use super::{CoreError, CoreErrorCode, OperationContext};

    #[test]
    fn exposes_stable_error_codes_and_retryability() {
        let invalid = CoreError::InvalidInput("bad query".to_string());
        assert_eq!(invalid.code(), CoreErrorCode::InvalidInput);
        assert!(!invalid.is_retryable());

        let adapter = CoreError::Adapter("process error".to_string());
        assert_eq!(adapter.code(), CoreErrorCode::AdapterExecution);
        assert!(!adapter.is_retryable());

        let adapter_protocol = CoreError::AdapterProtocol("invalid JSON".to_string());
        assert_eq!(adapter_protocol.code(), CoreErrorCode::AdapterProtocol);
        assert!(!adapter_protocol.is_retryable());

        let busy = CoreError::Busy("index is running".to_string());
        assert_eq!(busy.code(), CoreErrorCode::Busy);
        assert!(busy.is_retryable());

        let conflict = CoreError::Conflict("duplicate key".to_string());
        assert_eq!(conflict.code(), CoreErrorCode::Conflict);
        assert!(!conflict.is_retryable());
    }

    #[test]
    fn operation_context_rejects_elapsed_deadline() {
        let context = OperationContext::new("index-42").with_deadline_unix_ms(0);
        let error = context
            .check_deadline()
            .expect_err("zero deadline must be elapsed");

        assert!(matches!(error, CoreError::DeadlineExceeded(_)));
        assert!(error.to_string().contains("index-42"));
        assert_eq!(context.remaining(), Some(std::time::Duration::ZERO));
    }
}
