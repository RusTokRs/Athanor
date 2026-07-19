use async_trait::async_trait;
use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshotStore, CoreError, CoreResult, OperationContext,
    SnapshotBatch,
};
use athanor_domain::{RepoId, SnapshotBase, SnapshotId};

use super::AthanorStore;

impl AthanorStore {
    async fn reconcile_terminal_publication_error(
        &self,
        snapshot: &SnapshotId,
        error: CoreError,
    ) -> CoreResult<()> {
        match self.inner.load_snapshot(snapshot).await {
            Ok(Some(canonical)) => {
                if canonical.snapshot.as_ref() == Some(snapshot) {
                    Ok(())
                } else {
                    Err(CoreError::AdapterProtocol(format!(
                        "exact canonical snapshot {} returned identity {:?} after terminal publication error",
                        snapshot.0, canonical.snapshot
                    )))
                }
            }
            Ok(None)
            | Err(CoreError::NotFound(_))
            | Err(CoreError::SnapshotNotCommitted(_)) => Err(error),
            Err(status_error) => Err(CoreError::Adapter(format!(
                "{error}; failed to determine exact publication state for {}: {status_error}",
                snapshot.0
            ))),
        }
    }
}

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
        match self
            .inner
            .publish_snapshot_batch_with_context(snapshot.clone(), batch, context)
            .await
        {
            Ok(()) => Ok(()),
            Err(error)
                if matches!(
                    &error,
                    CoreError::Cancelled(_) | CoreError::DeadlineExceeded(_)
                ) =>
            {
                self.reconcile_terminal_publication_error(&snapshot, error)
                    .await
            }
            Err(error) => Err(error),
        }
    }
}
