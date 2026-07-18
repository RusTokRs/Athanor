use async_trait::async_trait;
use athanor_core::{AtomicSnapshotPublication, CoreResult, OperationContext, SnapshotBatch};
use athanor_domain::{RepoId, SnapshotBase, SnapshotId};

use super::AthanorStore;

#[async_trait]
impl AtomicSnapshotPublication for AthanorStore {
    async fn begin_snapshot_allocation(
        &self,
        repo: RepoId,
        base: SnapshotBase,
        context: &OperationContext,
    ) -> CoreResult<SnapshotId> {
        self.inner
            .begin_snapshot_allocation(repo, base, context)
            .await
    }

    async fn recover_orphan_snapshot_allocations(
        &self,
        repo: &RepoId,
        stale_before_unix_ms: u64,
        limit: usize,
    ) -> CoreResult<Vec<SnapshotId>> {
        self.inner
            .recover_orphan_snapshot_allocations(repo, stale_before_unix_ms, limit)
            .await
    }

    async fn publish_snapshot_batch(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()> {
        self.clear_pending_batch(&snapshot);
        self.inner.publish_snapshot_batch(snapshot, batch).await
    }

    async fn publish_snapshot_batch_with_context(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.clear_pending_batch(&snapshot);
        self.inner
            .publish_snapshot_batch_with_context(snapshot, batch, context)
            .await
    }
}
