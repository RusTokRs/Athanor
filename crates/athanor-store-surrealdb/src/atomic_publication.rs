use async_trait::async_trait;
use athanor_core::{
    AtomicSnapshotPublication, CoreResult, OperationContext, SnapshotBatch,
};
use athanor_domain::SnapshotId;

use super::{SurrealKnowledgeStore, classify_backend_result, retry_busy_with_context};

#[async_trait]
impl AtomicSnapshotPublication for SurrealKnowledgeStore {
    async fn publish_snapshot_batch(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()> {
        classify_backend_result(
            self.inner
                .publish_snapshot_batch_atomic(snapshot, batch)
                .await,
        )
    }

    async fn publish_snapshot_batch_with_context(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
        context: &OperationContext,
    ) -> CoreResult<()> {
        retry_busy_with_context(context, || {
            self.publish_snapshot_batch(snapshot.clone(), batch.clone())
        })
        .await
    }
}
