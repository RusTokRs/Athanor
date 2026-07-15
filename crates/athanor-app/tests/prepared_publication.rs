use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use athanor_app::{AthanorStore, PreparedSnapshotPublication};
use athanor_core::{
    AtomicSnapshotPublication, CancellationHandle, CanonicalSnapshot, CanonicalSnapshotStore,
    CoreResult, DiagnosticQuery, EntityQuery, EntityResolver, KnowledgeStore, OperationContext,
    OperationContextCancellation, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use athanor_store_jsonl::JsonlKnowledgeStore;

#[derive(Clone)]
struct RecordingStore {
    calls: Arc<Mutex<Vec<&'static str>>>,
    cancel_on_prepare: Option<CancellationHandle>,
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
        if let Some(cancellation) = &self.cancel_on_prepare {
            cancellation.cancel();
        }
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
impl AtomicSnapshotPublication for RecordingStore {
    async fn publish_snapshot_batch(
        &self,
        _snapshot: SnapshotId,
        _batch: SnapshotBatch,
    ) -> CoreResult<()> {
        self.record("atomic_plain");
        Ok(())
    }

    async fn publish_snapshot_batch_with_context(
        &self,
        _snapshot: SnapshotId,
        _batch: SnapshotBatch,
        _context: &OperationContext,
    ) -> CoreResult<()> {
        self.record("atomic_context");
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
async fn store_wrapper_buffers_context_batch_until_atomic_publish() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let store = AthanorStore::new(RecordingStore {
        calls: Arc::clone(&calls),
        cancel_on_prepare: None,
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
    assert!(
        calls.lock().unwrap().is_empty(),
        "context batch and prepare must remain process-local before commit"
    );

    store.publish_prepared(&prepared, &context).await.unwrap();
    assert_eq!(calls.lock().unwrap().as_slice(), ["atomic_context"]);

    store.abort_prepared(&prepared).await.unwrap();
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["atomic_context", "abort_plain"]
    );
}

#[tokio::test]
async fn successful_prepare_returns_handle_when_cancellation_races_after_backend_prepare() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let snapshot = SnapshotId("snap_recording_prepare_race".to_string());
    let context = OperationContext::new("daemon.index.prepare-race");
    let cancellation = context
        .cancellation_handle()
        .expect("register cancellation lease");
    let store = AthanorStore::new(RecordingStore {
        calls: Arc::clone(&calls),
        cancel_on_prepare: Some(cancellation.clone()),
    });

    let prepared = store
        .prepare_publication(snapshot.clone(), &context)
        .await
        .expect("successful backend prepare must return its cleanup handle");

    assert_eq!(prepared.snapshot(), &snapshot);
    assert!(context.is_cancelled());
    store.abort_prepared(&prepared).await.unwrap();
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["prepare_context", "abort_plain"]
    );
}

#[tokio::test]
async fn jsonl_store_supports_typed_prepare_publish_and_abort() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("athanor-prepared-jsonl-{nonce}"));
    let store = JsonlKnowledgeStore::new(&root);
    let repo = RepoId(format!("repo_prepared_jsonl_{nonce}"));

    let published = store
        .begin_snapshot(repo.clone(), working_tree_base())
        .await
        .expect("begin published snapshot");
    let publish_context = OperationContext::new(format!("test.jsonl.publish.{nonce}"));
    store
        .put_snapshot_with_context(
            published.clone(),
            SnapshotBatch::default(),
            &publish_context,
        )
        .await
        .expect("write published snapshot");
    let prepared = store
        .prepare_publication(published.clone(), &publish_context)
        .await
        .expect("prepare published snapshot");
    store
        .publish_prepared(&prepared, &publish_context)
        .await
        .expect("publish prepared snapshot");

    let latest = store
        .load_latest_snapshot()
        .await
        .expect("load latest published snapshot")
        .expect("published snapshot exists");
    assert_eq!(latest.snapshot.as_ref(), Some(&published));

    let aborted = store
        .begin_snapshot(repo, working_tree_base())
        .await
        .expect("begin aborted snapshot");
    let abort_context = OperationContext::new(format!("test.jsonl.abort.{nonce}"));
    store
        .put_snapshot_with_context(aborted.clone(), SnapshotBatch::default(), &abort_context)
        .await
        .expect("write aborted snapshot");
    let prepared = store
        .prepare_publication(aborted, &abort_context)
        .await
        .expect("prepare aborted snapshot");
    store
        .abort_prepared(&prepared)
        .await
        .expect("abort prepared snapshot");

    let latest_after_abort = store
        .load_latest_snapshot()
        .await
        .expect("load latest snapshot after abort")
        .expect("published snapshot remains visible");
    assert_eq!(latest_after_abort.snapshot.as_ref(), Some(&published));

    drop(store);
    std::fs::remove_dir_all(root).expect("remove JSONL prepared-publication store");
}

fn working_tree_base() -> SnapshotBase {
    SnapshotBase {
        branch: None,
        commit: None,
        parent_snapshot: None,
        working_tree: true,
    }
}
