use std::fs;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use athanor_core::{
    AtomicSnapshotPublication, CancellationHandle, CanonicalSnapshot, CanonicalSnapshotStore,
    CoreError, CoreResult, DiagnosticQuery, EntityQuery, EntityResolver, KnowledgeStore,
    OperationContext, OperationContextCancellation, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, Entity, EntityId, Fact, Relation, RepoId, SnapshotBase, SnapshotId, StableKey,
};
use athanor_store_jsonl::JsonlKnowledgeStore;

use crate::AthanorStore;

#[tokio::test]
async fn pre_commit_cancellation_remains_an_error() {
    let fixture = fixture(PublishMode::BeforeCommitCancelled).await;

    let error = fixture
        .store
        .publish_snapshot_batch_with_context(
            fixture.snapshot.clone(),
            SnapshotBatch::default(),
            &fixture.operation,
        )
        .await
        .expect_err("pre-commit cancellation must remain an error");

    assert!(matches!(error, CoreError::Cancelled(_)));
    assert_uncommitted(&fixture.store, &fixture.snapshot).await;
    fixture.cleanup();
}

#[tokio::test]
async fn committed_terminal_errors_are_reconciled_to_success() {
    for mode in [
        PublishMode::CommitThenCancelled,
        PublishMode::CommitThenDeadline,
    ] {
        let fixture = fixture(mode).await;

        fixture
            .store
            .publish_snapshot_batch_with_context(
                fixture.snapshot.clone(),
                SnapshotBatch::default(),
                &fixture.operation,
            )
            .await
            .expect("exact committed snapshot must preserve durable success");

        assert_committed(&fixture.store, &fixture.snapshot).await;
        fixture.cleanup();
    }
}

#[tokio::test]
async fn cancellation_after_commit_does_not_override_success() {
    let fixture = fixture(PublishMode::CommitThenCancelContext).await;

    fixture
        .store
        .publish_snapshot_batch_with_context(
            fixture.snapshot.clone(),
            SnapshotBatch::default(),
            &fixture.operation,
        )
        .await
        .expect("post-commit cancellation must not replace durable success");

    assert!(fixture.cancellation.is_cancelled());
    assert!(fixture.operation.is_cancelled());
    assert_committed(&fixture.store, &fixture.snapshot).await;
    fixture.cleanup();
}

struct Fixture {
    root: PathBuf,
    store: AthanorStore,
    snapshot: SnapshotId,
    operation: OperationContext,
    cancellation: CancellationHandle,
}

impl Fixture {
    fn cleanup(self) {
        fs::remove_dir_all(self.root).expect("remove publication cancellation fixture");
    }
}

async fn fixture(mode: PublishMode) -> Fixture {
    let root = test_root(mode.label());
    let operation = OperationContext::new(format!(
        "test.store.publication.{}.{}",
        mode.label(),
        publication_nonce()
    ));
    let cancellation = operation
        .cancellation_handle()
        .expect("register publication cancellation");
    let backend = TerminalPublicationStore {
        inner: JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl")),
        mode,
        cancellation: cancellation.clone(),
    };
    let store = AthanorStore::new(backend);
    let snapshot = store
        .begin_snapshot(
            RepoId(format!("repo_{}", mode.label())),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin publication snapshot");

    Fixture {
        root,
        store,
        snapshot,
        operation,
        cancellation,
    }
}

async fn assert_committed(store: &AthanorStore, snapshot: &SnapshotId) {
    let canonical = store
        .load_snapshot(snapshot)
        .await
        .expect("load exact committed snapshot")
        .expect("committed snapshot must exist");
    assert_eq!(canonical.snapshot.as_ref(), Some(snapshot));
}

async fn assert_uncommitted(store: &AthanorStore, snapshot: &SnapshotId) {
    match store.load_snapshot(snapshot).await {
        Ok(None) | Err(CoreError::NotFound(_)) | Err(CoreError::SnapshotNotCommitted(_)) => {}
        Ok(Some(canonical)) => panic!(
            "pre-commit cancellation exposed committed snapshot {:?}",
            canonical.snapshot
        ),
        Err(error) => panic!("unexpected exact snapshot probe error: {error}"),
    }
}

#[derive(Debug, Clone, Copy)]
enum PublishMode {
    BeforeCommitCancelled,
    CommitThenCancelled,
    CommitThenDeadline,
    CommitThenCancelContext,
}

impl PublishMode {
    fn label(self) -> &'static str {
        match self {
            Self::BeforeCommitCancelled => "before-commit-cancelled",
            Self::CommitThenCancelled => "commit-then-cancelled",
            Self::CommitThenDeadline => "commit-then-deadline",
            Self::CommitThenCancelContext => "commit-then-cancel-context",
        }
    }
}

#[derive(Clone)]
struct TerminalPublicationStore {
    inner: JsonlKnowledgeStore,
    mode: PublishMode,
    cancellation: CancellationHandle,
}

#[async_trait]
impl AtomicSnapshotPublication for TerminalPublicationStore {
    async fn publish_snapshot_batch(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
    ) -> CoreResult<()> {
        self.inner.publish_snapshot_batch(snapshot, batch).await
    }

    async fn publish_snapshot_batch_with_context(
        &self,
        snapshot: SnapshotId,
        batch: SnapshotBatch,
        context: &OperationContext,
    ) -> CoreResult<()> {
        match self.mode {
            PublishMode::BeforeCommitCancelled => Err(CoreError::Cancelled(
                "publication cancelled before durable commit".to_string(),
            )),
            PublishMode::CommitThenCancelled => {
                self.inner
                    .publish_snapshot_batch_with_context(snapshot, batch, context)
                    .await?;
                Err(CoreError::Cancelled(
                    "publication cancellation raced with durable commit".to_string(),
                ))
            }
            PublishMode::CommitThenDeadline => {
                self.inner
                    .publish_snapshot_batch_with_context(snapshot, batch, context)
                    .await?;
                Err(CoreError::DeadlineExceeded(
                    "publication deadline raced with durable commit".to_string(),
                ))
            }
            PublishMode::CommitThenCancelContext => {
                self.inner
                    .publish_snapshot_batch_with_context(snapshot, batch, context)
                    .await?;
                self.cancellation.cancel();
                Ok(())
            }
        }
    }
}

#[async_trait]
impl KnowledgeStore for TerminalPublicationStore {
    async fn begin_snapshot(&self, repo: RepoId, base: SnapshotBase) -> CoreResult<SnapshotId> {
        self.inner.begin_snapshot(repo, base).await
    }

    async fn put_entities(&self, snapshot: SnapshotId, entities: Vec<Entity>) -> CoreResult<()> {
        self.inner.put_entities(snapshot, entities).await
    }

    async fn put_facts(&self, snapshot: SnapshotId, facts: Vec<Fact>) -> CoreResult<()> {
        self.inner.put_facts(snapshot, facts).await
    }

    async fn put_relations(
        &self,
        snapshot: SnapshotId,
        relations: Vec<Relation>,
    ) -> CoreResult<()> {
        self.inner.put_relations(snapshot, relations).await
    }

    async fn put_diagnostics(
        &self,
        snapshot: SnapshotId,
        diagnostics: Vec<Diagnostic>,
    ) -> CoreResult<()> {
        self.inner.put_diagnostics(snapshot, diagnostics).await
    }

    async fn put_snapshot(&self, snapshot: SnapshotId, batch: SnapshotBatch) -> CoreResult<()> {
        self.inner.put_snapshot(snapshot, batch).await
    }

    async fn prepare_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.inner.prepare_snapshot(snapshot).await
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
        self.inner.commit_snapshot(snapshot).await
    }

    async fn abort_snapshot(&self, snapshot: SnapshotId) -> CoreResult<()> {
        self.inner.abort_snapshot(snapshot).await
    }
}

#[async_trait]
impl CanonicalSnapshotStore for TerminalPublicationStore {
    async fn load_snapshot(&self, snapshot: &SnapshotId) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_snapshot(snapshot).await
    }

    async fn load_latest_snapshot(&self) -> CoreResult<Option<CanonicalSnapshot>> {
        self.inner.load_latest_snapshot().await
    }
}

#[async_trait]
impl EntityResolver for TerminalPublicationStore {
    async fn resolve_stable_key(
        &self,
        snapshot: SnapshotSelector,
        stable_key: &StableKey,
    ) -> CoreResult<Option<EntityId>> {
        self.inner.resolve_stable_key(snapshot, stable_key).await
    }
}

fn test_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "athanor-store-publication-{label}-{}",
        publication_nonce()
    ))
}

fn publication_nonce() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos()
}
