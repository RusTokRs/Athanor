include!("transient_store_inner.rs");

#[async_trait]
impl athanor_core::AtomicSnapshotPublication for TransientKnowledgeStore {
    async fn publish_snapshot_batch(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()> {
        let mut state = self.lock_state()?;
        let data = state.snapshot_mut(&snapshot)?;
        if data.committed {
            return Err(CoreError::Conflict(format!(
                "cannot republish committed snapshot {}",
                snapshot.0
            )));
        }
        *data = SnapshotData {
            committed: true,
            entities: batch.entities,
            facts: batch.facts,
            relations: batch.relations,
            diagnostics: batch.diagnostics,
        };
        Ok(())
    }
}
