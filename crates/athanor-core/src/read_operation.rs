//! Cooperative cancellation and deadline checks for read-only core ports.

use async_trait::async_trait;
use athanor_domain::{Entity, EntityId, SnapshotId, StableKey};

use crate::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreResult, DiagnosticQuery, EntityQuery,
    EntityResolver, FactQuery, FactQueryStore, KnowledgeStore, OperationContext,
    OperationContextCancellation, RelationQuery, SearchIndex, SearchQuery, SearchResult,
    SnapshotSelector,
};
use athanor_domain::{Diagnostic, Fact, Relation};

/// Context-aware canonical snapshot reads without changing existing store implementers.
#[async_trait]
pub trait CanonicalSnapshotStoreOperationExt: CanonicalSnapshotStore {
    async fn load_snapshot_with_operation_context(
        &self,
        snapshot: &SnapshotId,
        operation: &OperationContext,
    ) -> CoreResult<Option<CanonicalSnapshot>> {
        operation.check_active()?;
        let result = self.load_snapshot(snapshot).await?;
        operation.check_active()?;
        Ok(result)
    }

    async fn load_latest_snapshot_with_operation_context(
        &self,
        operation: &OperationContext,
    ) -> CoreResult<Option<CanonicalSnapshot>> {
        operation.check_active()?;
        let result = self.load_latest_snapshot().await?;
        operation.check_active()?;
        Ok(result)
    }
}

impl<T> CanonicalSnapshotStoreOperationExt for T where T: CanonicalSnapshotStore + ?Sized {}

/// Context-aware entity/relation/diagnostic queries for existing `KnowledgeStore` implementations.
#[async_trait]
pub trait KnowledgeStoreQueryOperationExt: KnowledgeStore {
    async fn query_entities_with_operation_context(
        &self,
        snapshot: SnapshotSelector,
        query: EntityQuery,
        operation: &OperationContext,
    ) -> CoreResult<Vec<Entity>> {
        operation.check_active()?;
        let result = self.query_entities(snapshot, query).await?;
        operation.check_active()?;
        Ok(result)
    }

    async fn query_relations_with_operation_context(
        &self,
        snapshot: SnapshotSelector,
        query: RelationQuery,
        operation: &OperationContext,
    ) -> CoreResult<Vec<Relation>> {
        operation.check_active()?;
        let result = self.query_relations(snapshot, query).await?;
        operation.check_active()?;
        Ok(result)
    }

    async fn query_diagnostics_with_operation_context(
        &self,
        snapshot: SnapshotSelector,
        query: DiagnosticQuery,
        operation: &OperationContext,
    ) -> CoreResult<Vec<Diagnostic>> {
        operation.check_active()?;
        let result = self.query_diagnostics(snapshot, query).await?;
        operation.check_active()?;
        Ok(result)
    }
}

impl<T> KnowledgeStoreQueryOperationExt for T where T: KnowledgeStore + ?Sized {}

/// Context-aware stable-key resolution for existing resolvers.
#[async_trait]
pub trait EntityResolverOperationExt: EntityResolver {
    async fn resolve_stable_key_with_operation_context(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
        operation: &OperationContext,
    ) -> CoreResult<Option<EntityId>> {
        operation.check_active()?;
        let result = self.resolve_stable_key(snapshot, stable_key).await?;
        operation.check_active()?;
        Ok(result)
    }
}

impl<T> EntityResolverOperationExt for T where T: EntityResolver + ?Sized {}

/// Context-aware committed fact queries.
#[async_trait]
pub trait FactQueryStoreOperationExt: FactQueryStore {
    async fn query_facts_with_operation_context(
        &self,
        snapshot: SnapshotSelector,
        query: FactQuery,
        operation: &OperationContext,
    ) -> CoreResult<Vec<Fact>> {
        operation.check_active()?;
        let result = self.query_facts(snapshot, query).await?;
        operation.check_active()?;
        Ok(result)
    }
}

impl<T> FactQueryStoreOperationExt for T where T: FactQueryStore + ?Sized {}

/// Context-aware search. Implementations may override the underlying `search` method with finer-grained
/// polling; the extension guarantees preflight and postflight cancellation/deadline checks.
#[async_trait]
pub trait SearchIndexOperationExt: SearchIndex {
    async fn search_with_operation_context(
        &self,
        query: SearchQuery,
        operation: &OperationContext,
    ) -> CoreResult<Vec<SearchResult>> {
        operation.check_active()?;
        let result = self.search(query).await?;
        operation.check_active()?;
        Ok(result)
    }
}

impl<T> SearchIndexOperationExt for T where T: SearchIndex + ?Sized {}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use futures::executor::block_on;
    use serde_json::Value;

    use super::*;
    use crate::{CancellationHandle, CoreError, SearchDocument};

    struct CancellingSearch {
        cancellation: Mutex<Option<CancellationHandle>>,
    }

    #[async_trait]
    impl SearchIndex for CancellingSearch {
        async fn index_document(&self, _doc: SearchDocument) -> CoreResult<()> {
            Ok(())
        }

        async fn remove_document(&self, _id: &str) -> CoreResult<()> {
            Ok(())
        }

        async fn search(&self, _query: SearchQuery) -> CoreResult<Vec<SearchResult>> {
            self.cancellation
                .lock()
                .expect("cancellation fixture lock")
                .take()
                .expect("cancellation handle")
                .cancel();
            Ok(vec![SearchResult {
                id: "entity".to_string(),
                score: 1.0,
                payload: Value::Null,
            }])
        }
    }

    #[test]
    fn pre_cancelled_read_is_rejected_before_backend_work() {
        let operation = OperationContext::new("read.pre-cancelled");
        let cancellation = operation.cancellation_handle().unwrap();
        cancellation.cancel();
        let index = CancellingSearch {
            cancellation: Mutex::new(Some(cancellation)),
        };

        let error = block_on(index.search_with_operation_context(
            SearchQuery {
                query: "entity".to_string(),
                limit: 1,
            },
            &operation,
        ))
        .expect_err("pre-cancelled read must fail");

        assert!(matches!(error, CoreError::Cancelled(_)));
        assert!(
            index
                .cancellation
                .lock()
                .expect("cancellation fixture lock")
                .is_some()
        );
    }

    #[test]
    fn cancellation_observed_during_backend_read_rejects_success() {
        let operation = OperationContext::new("read.mid-flight");
        let cancellation = operation.cancellation_handle().unwrap();
        let index = CancellingSearch {
            cancellation: Mutex::new(Some(cancellation)),
        };

        let error = block_on(index.search_with_operation_context(
            SearchQuery {
                query: "entity".to_string(),
                limit: 1,
            },
            &operation,
        ))
        .expect_err("cancelled in-flight read must not return a successful result");

        assert!(matches!(error, CoreError::Cancelled(_)));
    }

    #[test]
    fn expired_deadline_is_rejected_before_backend_work() {
        let operation = OperationContext::new("read.deadline").with_deadline_unix_ms(0);
        let index = CancellingSearch {
            cancellation: Mutex::new(None),
        };

        let error = block_on(index.search_with_operation_context(
            SearchQuery {
                query: "entity".to_string(),
                limit: 1,
            },
            &operation,
        ))
        .expect_err("expired read deadline must fail");

        assert!(matches!(error, CoreError::DeadlineExceeded(_)));
    }
}
