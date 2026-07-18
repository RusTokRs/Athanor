use async_trait::async_trait;
use athanor_core::{
    AtomicSnapshotPublication, CoreResult, DiagnosticQuery, EntityQuery, KnowledgeStore,
    OperationContext, OperationContextCancellation, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, Fact, Relation, RepoId, SnapshotBase, SnapshotId,
};

use super::AthanorStore;

#[async_trait]
impl KnowledgeStore for AthanorStore {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
        self.inner.begin_snapshot(repo, base).await
    }

    async fn begin_snapshot_with_context(
        &self,
        repo: RepoId,
        base: SnapshotBase,
        context: &OperationContext,
    ) -> CoreResult<SnapshotId> {
        AtomicSnapshotPublication::begin_snapshot_allocation(self, repo, base, context).await
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        self.inner.put_entities(snapshot, entities).await
    }

    async fn put_entities_with_context(
        &self,
        snapshot: SnapshotId,
        entities: Vec<Entity>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.inner
            .put_entities_with_context(snapshot, entities, context)
            .await
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        self.inner.put_facts(snapshot, facts).await
    }

    async fn put_facts_with_context(
        &self,
        snapshot: SnapshotId,
        facts: Vec<Fact>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.inner
            .put_facts_with_context(snapshot, facts, context)
            .await
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        self.inner.put_relations(snapshot, relations).await
    }

    async fn put_relations_with_context(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.inner
            .put_relations_with_context(snapshot, relations, context)
            .await
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        self.inner.put_diagnostics(snapshot, diagnostics).await
    }

    async fn put_diagnostics_with_context(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.inner
            .put_diagnostics_with_context(snapshot, diagnostics, context)
            .await
    }

    async fn put_snapshot(&self, snapshot: SnapshotId, batch: SnapshotBatch) -> CoreResult<()> {
        self.inner.put_snapshot(snapshot, batch).await
    }

    async fn put_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
        context: &OperationContext,
    ) -> CoreResult<()> {
        context.check_active()?;
        self.stage_pending_batch(snapshot, batch);
        Ok(())
    }

    async fn prepare_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        if self.has_pending_batch(&snapshot) {
            return Ok(());
        }
        self.inner.prepare_snapshot(snapshot).await
    }

    async fn prepare_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        if self.has_pending_batch(&snapshot) {
            context.check_active()?;
            return Ok(());
        }
        self.inner
            .prepare_snapshot_with_context(snapshot, context)
            .await
    }

    async fn query_entities(
        &self,
        snapshot: SnapshotSelector,
        query: EntityQuery,
    ) -> CoreResult<Vec<Entity>> {
        self.inner.query_entities(snapshot, query).await
    }

    async fn query_relations(
        &self,
        snapshot: SnapshotSelector,
        query: RelationQuery,
    ) -> CoreResult<Vec<Relation>> {
        self.inner.query_relations(snapshot, query).await
    }

    async fn query_diagnostics(
        &self,
        snapshot: SnapshotSelector,
        query: DiagnosticQuery,
    ) -> CoreResult<Vec<Diagnostic>> {
        self.inner.query_diagnostics(snapshot, query).await
    }

    async fn commit_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        if let Some(batch) = self.take_pending_batch(&snapshot) {
            return self.inner.publish_snapshot_batch(snapshot, batch).await;
        }
        self.inner.commit_snapshot(snapshot).await
    }

    async fn commit_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        if let Some(batch) = self.take_pending_batch(&snapshot) {
            return self
                .inner
                .publish_snapshot_batch_with_context(snapshot, batch, context)
                .await;
        }
        self.inner
            .commit_snapshot_with_context(snapshot, context)
            .await
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.clear_pending_batch(&snapshot);
        self.inner.abort_snapshot(snapshot).await
    }

    async fn abort_snapshot_with_context(
        &self,
        snapshot: SnapshotId,
        context: &OperationContext,
    ) -> CoreResult<()> {
        self.clear_pending_batch(&snapshot);
        self.inner
            .abort_snapshot_with_context(snapshot, context)
            .await
    }
}
