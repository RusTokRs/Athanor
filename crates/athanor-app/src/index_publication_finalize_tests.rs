include!("index_publication_finalize_tests_inner.rs");

#[async_trait]
impl athanor_core::AtomicSnapshotPublication for SabotagingStore {
    async fn publish_snapshot_batch(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()> {
        athanor_core::AtomicSnapshotPublication::publish_snapshot_batch(
            &self.inner,
            snapshot,
            batch,
        )
        .await?;
        self.sabotage()
    }

    async fn publish_snapshot_batch_with_context(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
        context: &OperationContext,
    ) -> CoreResult<()> {
        athanor_core::AtomicSnapshotPublication::publish_snapshot_batch_with_context(
            &self.inner,
            snapshot,
            batch,
            context,
        )
        .await?;
        self.sabotage()
    }
}
