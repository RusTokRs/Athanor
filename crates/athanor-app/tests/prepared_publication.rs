use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use athanor_app::{AthanorStore, PreparedSnapshotPublication};
use athanor_core::{
    CanonicalSnapshot, CanonicalSnapshotStore, CoreResult, DiagnosticQuery, EntityQuery,
    EntityResolver, KnowledgeStore, OperationContext, RelationQuery, SnapshotBatch,
    SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};

#[derive(Clone)]
struct RecordingStore {
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl RecordingStore {
    fn record(&self, call: &'static str) {
        self.calls.lock().unwrap().push(call);
    }
}

#[async_trait]
impl KnowledgeStore for RecordingStore {
    async fn begin_snapshot(&self, _repo: RepoId, _base: SnapshotBase) -> CoreResult<SnapshotId> {
        self.record("begin_plain");
        Ok(SnapshotId("snap_recording_0001".to_string()))
    }

    async fn put_entities(&self, _snapshot: SnapshotId, _entities: Vec<Entity>) -> CoreResult<()> {
        self.record("entities_plain");
        Ok(())
    }

    async fn put_facts(&self, _snapshot: SnapshotId, _facts: Vec<Fact>) -> CoreResult<()> {
        self.record("facts_plain");
        Ok(())
    }

    async fn put_relations(
        &self,
        _snapshot: SnapshotId,
        _relations: Vec<Relation>,
    ) -> CoreResult<()> {
        self.record("relations_plain");
        Ok(())
    }

    async fn put_diagnostics(
        &self,
        _snapshot: SnapshotId,
        _diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        self.record("diagnostics_plain");
        Ok(())
    }

    async fn put_snapshot(&self, _snapshot: SnapshotId, _batch: SnapshotBatch) -> CoreResult<()> {
        self.record("batch_plain");
        Ok(())
    }

    async fn put_snapshot_with_context(
        &self,
        _snapshot: SnapshotId,
        _batch: SnapshotBatch,
        _context: &OperationContext,
    ) -> CoreResult<()> {
        self.record("batch_context");
        Ok(())
    }

    async fn prepare_snapshot(&self, _snapshot: SnapshotId) -> CoreResult<()> {
        self.record("prepare_plain");
        Ok(())
    }

    async fn prepare_snapshot_with_context(
        &self,
        _snapshot: SnapshotId,
        _context: &OperationContext,
    ) -> CoreResult<()> {
        self.record("prepare_context");
        Ok(())
    }

    async fn query_entities(
        &self,
        _snapshot: SnapshotSelector,
        _query: EntityQuery,
    ) -> CoreResult<Vec<Entity>> {
        Ok(Vec::new())
    }

    async fn query_relations(
        &self,
        _snapshot: SnapshotSelector,
        _query: RelationQuery,
    ) -> CoreResult<Vec<Relation>> {
        Ok(Vec::new())
    }

    async fn query_diagnostics(
        &self,
        _snapshot: SnapshotSelector,
        _query: DiagnosticQuery,
    ) -> CoreResult<Vec<Diagnostic>> {
        Ok(Vec::new())
    }

    async fn commit_snapshot(&self, _snapshot: SnapshotId) -> CoreResult<()> {
        self.record("commit_plain");
        Ok(())
    }

    async fn commit_snapshot_with_context(
        &self,
        _snapshot: SnapshotId,
        _context: &OperationContext,
    ) -> CoreResult<()> {
        self.record("commit_context");
        Ok(())
    }

    async fn abort_snapshot(&self, _snapshot: SnapshotId) -> CoreResult<()> {
        self.record("abort_plain");
        Ok(())
    }

    async fn abort_snapshot_with_context(
        &self,
        _snapshot: SnapshotId,
        _context: &OperationContext,
    ) -> CoreResult<()> {
        self.record("abort_context");
        Ok(())
    }
}

#[async_trait]
impl CanonicalSnapshotStore for RecordingStore {
    async fn load_snapshot(&self, _snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        Ok(None)
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        Ok(None)
    }
}

#[async_trait]
impl EntityResolver for RecordingStore {
    async fn resolve_stable_key(
        &self,
        _snapshot: SnapshotSelector,
        _stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        Ok(None)
    }
}

#[tokio::test]
async fn store_wrapper_preserves_context_overrides_and_plain_cleanup() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let store = AthanorStore::new(RecordingStore {
        calls: Arc::clone(&calls),
    });
    let snapshot = SnapshotId("snap_recording_0001".to_string());
    let context = OperationContext::new("daemon.index.request-42");

    store
        .put_snapshot_with_context(snapshot.clone(), SnapshotBatch::default(), &context)
        .await
        .unwrap();
    let prepared = store
        .prepare_publication(snapshot.clone(), &context)
        .await
        .unwrap();
    assert_eq!(prepared.snapshot(), &snapshot);

    store.publish_prepared(&prepared, &context).await.unwrap();
    store.abort_prepared(&prepared).await.unwrap();

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        [
            "batch_context",
            "prepare_context",
            "commit_context",
            "abort_plain"
        ]
    );
}
