//! Core ports for Athanor.
//!
//! Implementations live in adapter crates. This crate owns only contracts and
//! request/response shapes used by the application layer.

pub mod atomic_publication;
pub mod cancellation;
pub mod fact_query;
pub mod latest_pointer;
pub mod ports;
pub mod prepared_publication;
pub mod read_operation;

pub use atomic_publication::AtomicSnapshotPublication;
pub use cancellation::{CancellationHandle, OperationContextCancellation};
pub use fact_query::{FactQuery, FactQueryStore, filter_facts};
pub use latest_pointer::{CanonicalLatestIdentity, CanonicalLatestPointer};
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
pub use read_operation::{
    CanonicalSnapshotStoreOperationExt, EntityResolverOperationExt, FactQueryStoreOperationExt,
    KnowledgeStoreQueryOperationExt, SearchIndexOperationExt,
};
