use async_trait::async_trait;
use athanor_core::{AtomicSnapshotPublication, CoreResult, OperationContext, SnapshotBatch};
use athanor_domain::{RepoId, SnapshotBase, SnapshotId};

use super::{SurrealKnowledgeStore, classify_backend_result, retry_busy_with_context};

#[async_trait]
impl AtomicSnapshotPublication for SurrealKnowledgeStore {
    async fn begin_snapshot_allocation(
        &self,
        repo: RepoId,
        base: SnapshotBase,
        context: &OperationContext,
    ) -> CoreResult<SnapshotId> {
        retry_busy_with_context(context, || async {
            classify_backend_result(
                self.inner
                    .begin_snapshot_allocation_atomic(repo.clone(), base.clone(), context)
                    .await,
            )
        })
        .await
    }

    async fn recover_orphan_snapshot_allocations(
        &self,
        repo: &RepoId,
        stale_before_unix_ms: u64,
        limit: usize,
    ) -> CoreResult<Vec<SnapshotId>> {
        classify_backend_result(
            self.inner
                .recover_orphan_snapshot_allocations_bounded(repo, stale_before_unix_ms, limit)
                .await,
        )
    }

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
