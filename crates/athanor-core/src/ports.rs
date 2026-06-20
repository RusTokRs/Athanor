use async_trait::async_trait;
use athanor_domain::{
    Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("adapter error: {0}")]
    Adapter(String),
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
    pub from: Option<StableKey>,
    pub to: Option<StableKey>,
    pub kind: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiagnosticQuery {
    pub severity: Option<String>,
    pub status: Option<String>,
    pub entity: Option<StableKey>,
    pub limit: Option<usize>,
}

#[async_trait]
pub trait KnowledgeStore: Send + Sync {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId>;

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()>;
    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()>;
    async fn put_relations(&self, snapshot: SnapshotId, relations: Vec<Relation>)
    -> CoreResult<()>;
    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()>;

    async fn query_entities(&self, query: EntityQuery) -> CoreResult<Vec<Entity>>;
    async fn query_relations(&self, query: RelationQuery) -> CoreResult<Vec<Relation>>;
    async fn query_diagnostics(&self, query: DiagnosticQuery) -> CoreResult<Vec<Diagnostic>>;

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()>;
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
    pub entities: Vec<Entity>,
    pub facts: Vec<Fact>,
}

#[async_trait]
pub trait SourceProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn discover(&self) -> CoreResult<Vec<SourceFile>>;
}

#[async_trait]
pub trait Extractor: Send + Sync {
    fn name(&self) -> &'static str;
    fn supports(&self, source: &SourceFile) -> bool;
    async fn extract(&self, input: ExtractInput) -> CoreResult<ExtractOutput>;
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
    pub entities: Vec<Entity>,
    pub facts: Vec<Fact>,
    pub affected: AffectedSubset,
}

#[async_trait]
pub trait Linker: Send + Sync {
    fn name(&self) -> &'static str;
    async fn link(&self, input: LinkInput) -> CoreResult<Vec<Relation>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckInput {
    pub snapshot: SnapshotId,
    pub entities: Vec<Entity>,
    pub facts: Vec<Fact>,
    pub relations: Vec<Relation>,
    pub affected: AffectedSubset,
}

#[async_trait]
pub trait Checker: Send + Sync {
    fn name(&self) -> &'static str;
    async fn check(&self, input: CheckInput) -> CoreResult<Vec<Diagnostic>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInput {
    pub snapshot: SnapshotId,
    pub target: String,
    pub payload: Value,
}

#[async_trait]
pub trait Projector: Send + Sync {
    fn name(&self) -> &'static str;
    async fn project(&self, input: ProjectInput) -> CoreResult<()>;
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
    fn name(&self) -> &'static str;
    async fn serve(&self) -> CoreResult<()>;
}
