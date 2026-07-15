use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshot, CanonicalSnapshotStore, CoreError, CoreResult,
    DiagnosticQuery, EntityQuery, EntityResolver, KnowledgeStore, OperationContext, PreparedSnapshot,
    RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};

use crate::index_publication::publish_prepared_index;
use crate::{
    AffectedFileSet, AthanorStore, IndexPipelineMetrics, IndexPipelineOutput, IndexStateStore,
};

#[tokio::test]
async fn publication_preserves_publish_rollback_and_abort_errors() {
    let root = test_root("combined-error");
    let backend = FailingCleanupStore::new(&root);
    let abort_count = backend.abort_count.clone();
    let store = AthanorStore::new(backend);
    let snapshot = SnapshotId("snap_combined_error".to_string());
    let prepared = PreparedSnapshot::new(snapshot.clone());
    let operation = OperationContext::new("test.publication.combined-error");
    let output_dir = root.join(".athanor/generated/current/jsonl");
    let state_path = root.join(".athanor/state/index-state.json");
    let state_store = IndexStateStore::new(&state_path);
    let output = IndexPipelineOutput {
        snapshot,
        files: Vec::new(),
        entities: Vec::new(),
        facts: Vec::new(),
        relations: Vec::new(),
        diagnostics: Vec::new(),
        affected_files: AffectedFileSet::default(),
        metrics: IndexPipelineMetrics::default(),
    };

    let error = publish_prepared_index(
        &root,
        &store,
        &state_store,
        &output_dir,
        &output,
        prepared,
        &operation,
    )
    .await
    .expect_err("combined publication failure must be reported");
    let messages = error.chain().map(ToString::to_string).collect::<Vec<_>>();

    assert!(
        messages
            .iter()
            .any(|message| message.contains("failed to publish prepared canonical snapshot"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("injected canonical publish failure"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("failed to rollback index state"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("failed to rollback read model"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("injected canonical abort failure"))
    );
    assert!(abort_count.load(Ordering::Acquire) >= 1);
    assert!(!root.join(".athanor/state/index-publication.json").exists());
    assert!(output_dir.is_file());
    assert!(state_path.is_dir());

    fs::remove_dir_all(root).expect("remove combined-error fixture");
}

#[derive(Clone)]
struct FailingCleanupStore {
    root: PathBuf,
    abort_count: Arc<AtomicUsize>,
}

impl FailingCleanupStore {
    fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            abort_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn corrupt_application_artifacts(&self) -> CoreResult<()> {
        let output_dir = self.root.join(".athanor/generated/current/jsonl");
        let state_path = self.root.join(".athanor/state/index-state.json");
        fs::remove_dir_all(&output_dir).map_err(adapter_error)?;
        fs::write(&output_dir, "blocked read model").map_err(adapter_error)?;
        fs::remove_file(&state_path).map_err(adapter_error)?;
        fs::create_dir(&state_path).map_err(adapter_error)?;
        Ok(())
    }
}

#[async_trait]
impl KnowledgeStore for FailingCleanupStore {
    async fn begin_snapshot(&self, _repo: RepoId, _base: SnapshotBase) -> CoreResult<SnapshotId> {
        Ok(SnapshotId("snap_combined_error".to_string()))
    }

    async fn put_entities(&self, _snapshot: SnapshotId, _entities: Vec<Entity>) -> CoreResult<()> {
        Ok(())
    }

    async fn put_facts(&self, _snapshot: SnapshotId, _facts: Vec<Fact>) -> CoreResult<()> {
        Ok(())
    }

    async fn put_relations(
        &self,
        _snapshot: SnapshotId,
        _relations: Vec<Relation>,
    ) -> CoreResult<()> {
        Ok(())
    }

    async fn put_diagnostics(
        &self,
        _snapshot: SnapshotId,
        _diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
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
        Err(CoreError::Adapter(
            "injected canonical publish failure".to_string(),
        ))
    }

    async fn commit_snapshot_with_context(
        &self,
        _snapshot: SnapshotId,
        _context: &OperationContext,
    ) -> CoreResult<()> {
        self.corrupt_application_artifacts()?;
        Err(CoreError::Adapter(
            "injected canonical publish failure".to_string(),
        ))
    }

    async fn abort_snapshot(&self, _snapshot: SnapshotId) -> CoreResult<()> {
        self.abort_count.fetch_add(1, Ordering::AcqRel);
        Err(CoreError::Adapter(
            "injected canonical abort failure".to_string(),
        ))
    }
}

#[async_trait]
impl AtomicSnapshotPublication for FailingCleanupStore {
    async fn publish_snapshot_batch(
        &self,
        _snapshot: SnapshotId,
        _batch: SnapshotBatch,
    ) -> CoreResult<()> {
        Err(CoreError::Adapter(
            "injected canonical publish failure".to_string(),
        ))
    }

    async fn publish_snapshot_batch_with_context(
        &self,
        _snapshot: SnapshotId,
        _batch: SnapshotBatch,
        _context: &OperationContext,
    ) -> CoreResult<()> {
        self.corrupt_application_artifacts()?;
        Err(CoreError::Adapter(
            "injected canonical publish failure".to_string(),
        ))
    }
}

#[async_trait]
impl CanonicalSnapshotStore for FailingCleanupStore {
    async fn load_snapshot(&self, _snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        Ok(None)
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        Ok(None)
    }
}

#[async_trait]
impl EntityResolver for FailingCleanupStore {
    async fn resolve_stable_key(
        &self,
        _snapshot: SnapshotSelector,
        _stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        Ok(None)
    }
}

fn adapter_error(error: std::io::Error) -> CoreError {
    CoreError::Adapter(format!("combined error injection failed: {error}"))
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-publication-{label}-{nonce}"))
}
