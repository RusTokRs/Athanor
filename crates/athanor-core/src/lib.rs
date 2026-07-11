//! Core ports for Athanor.
//!
//! Implementations live in adapter crates. This crate owns only contracts and
//! request/response shapes used by the application layer.

pub mod ports;

pub use ports::{
    AffectedSubset, AgentInterface, AgentRequest, AgentResponse, CanonicalSnapshot,
    CanonicalSnapshotStore, CheckInput, Checker, CoreError, CoreResult, DiagnosticQuery,
    EmbeddingInput, EmbeddingProvider, EntityQuery, EntityResolver, ExtractInput, ExtractOutput,
    Extractor, KnowledgeStore, LinkInput, Linker, ProjectInput, Projector, RelationQuery,
    SearchDocument, SearchIndex, SearchQuery, SearchResult, SnapshotSelector, SourceFile,
    SourceProvider, Transport, VectorIndex, VectorItem, VectorQuery, VectorSearchResult,
};
