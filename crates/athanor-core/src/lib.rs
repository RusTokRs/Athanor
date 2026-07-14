//! Core ports for Athanor.
//!
//! Implementations live in adapter crates. This crate owns only contracts and
//! request/response shapes used by the application layer.

pub mod cancellation;
pub mod ports;
pub mod prepared_publication;

pub use cancellation::{CancellationHandle, OperationContextCancellation};
pub use ports::{
    AffectedSubset, AgentInterface, AgentRequest, AgentResponse, CanonicalSnapshot,
    CanonicalSnapshotStore, CheckInput, Checker, CoreError, CoreErrorCode, CoreResult,
    DiagnosticQuery, EmbeddingInput, EmbeddingProvider, EntityQuery, EntityResolver, ExtractInput,
    ExtractOutput, Extractor, InvalidationPolicy, InvalidationScope, KnowledgeStore, LinkInput,
    Linker, OperationContext, ProcessLimits, ProcessOutput, ProcessRequest, ProcessRunner,
    ProjectInput, Projector, RelationQuery, SearchDocument, SearchIndex, SearchQuery, SearchResult,
    SnapshotBatch, SnapshotSelector, SourceFile, SourceProvider, Transport, VectorIndex,
    VectorItem, VectorQuery, VectorSearchResult,
};
pub use prepared_publication::{PreparedSnapshot, PreparedSnapshotPublication};
