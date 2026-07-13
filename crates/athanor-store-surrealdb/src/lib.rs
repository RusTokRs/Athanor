//! Public SurrealDB store boundary with stable retry classification.

use async_trait::async_trait;
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult, DiagnosticQuery, EntityQuery,
    EntityResolver, KnowledgeStore, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};

#[path = "transactional.rs"]
mod backend;

/// SurrealDB store facade that translates confirmed transient backend failures into `CoreError::Busy`.
#[derive(Debug, Clone)]
pub struct SurrealKnowledgeStore {
    inner: backend::SurrealKnowledgeStore,
}

impl SurrealKnowledgeStore {
    pub async fn connect(uri: &str) -> CoreResult<Self> {
        classify_backend_result(backend::SurrealKnowledgeStore::connect(uri).await)
            .map(|inner| Self { inner })
    }
}

fn classify_backend_result<T>(result: CoreResult<T>) -> CoreResult<T> {
    result.map_err(classify_backend_error)
}

fn classify_backend_error(error: CoreError) -> CoreError {
    match error {
        CoreError::Adapter(message) if is_retryable_surreal_message(&message) => {
            CoreError::Busy(message)
        }
        other => other,
    }
}

fn is_retryable_surreal_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("already locked by another process")
        || message.contains("transaction write conflict")
        || message.contains("transaction retry required")
}

#[async_trait]
impl KnowledgeStore for SurrealKnowledgeStore {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
        classify_backend_result(self.inner.begin_snapshot(repo, base).await)
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        classify_backend_result(self.inner.put_entities(snapshot, entities).await)
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        classify_backend_result(self.inner.put_facts(snapshot, facts).await)
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        classify_backend_result(self.inner.put_relations(snapshot, relations).await)
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        classify_backend_result(self.inner.put_diagnostics(snapshot, diagnostics).await)
    }

    async fn put_snapshot(&self, snapshot: SnapshotId, batch: SnapshotBatch) -> CoreResult<()> {
        classify_backend_result(self.inner.put_snapshot(snapshot, batch).await)
    }

    async fn prepare_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        classify_backend_result(self.inner.prepare_snapshot(snapshot).await)
    }

    async fn query_entities(
        &self,
        snapshot: SnapshotSelector,
        query: EntityQuery,
    ) -> CoreResult<Vec<Entity>> {
        classify_backend_result(self.inner.query_entities(snapshot, query).await)
    }

    async fn query_relations(
        &self,
        snapshot: SnapshotSelector,
        query: RelationQuery,
    ) -> CoreResult<Vec<Relation>> {
        classify_backend_result(self.inner.query_relations(snapshot, query).await)
    }

    async fn query_diagnostics(
        &self,
        snapshot: SnapshotSelector,
        query: DiagnosticQuery,
    ) -> CoreResult<Vec<Diagnostic>> {
        classify_backend_result(self.inner.query_diagnostics(snapshot, query).await)
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        classify_backend_result(self.inner.commit_snapshot(snapshot).await)
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        classify_backend_result(self.inner.abort_snapshot(snapshot).await)
    }
}

#[async_trait]
impl EntityResolver for SurrealKnowledgeStore {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        classify_backend_result(self.inner.resolve_stable_key(snapshot, stable_key).await)
    }
}

#[async_trait]
impl CanonicalSnapshotStore for SurrealKnowledgeStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        classify_backend_result(self.inner.load_snapshot(snapshot).await)
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        classify_backend_result(self.inner.load_latest_snapshot().await)
    }
}

#[cfg(test)]
mod tests {
    use athanor_core::CoreErrorCode;

    use super::*;

    #[test]
    fn classifies_persistent_lock_contention_as_retryable_busy() {
        let error = classify_backend_error(CoreError::Adapter(
            "failed to connect to surrealdb: Database at /tmp/test/LOCK is already locked by another process"
                .to_string(),
        ));

        assert_eq!(error.code(), CoreErrorCode::Busy);
        assert!(error.is_retryable());
    }

    #[test]
    fn classifies_transaction_conflicts_as_retryable_busy() {
        for message in [
            "Transaction write conflict",
            "Transaction retry required: snapshot is older than the commit oracle's GC window",
        ] {
            let error = classify_backend_error(CoreError::Adapter(message.to_string()));
            assert_eq!(error.code(), CoreErrorCode::Busy);
            assert!(error.is_retryable());
        }
    }

    #[test]
    fn leaves_data_and_statement_failures_non_retryable() {
        let error = classify_backend_error(CoreError::Adapter(
            "snapshot batch transaction rolled back: duplicate record id".to_string(),
        ));

        assert_eq!(error.code(), CoreErrorCode::AdapterExecution);
        assert!(!error.is_retryable());
    }
}
