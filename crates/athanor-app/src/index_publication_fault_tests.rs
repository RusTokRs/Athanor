use std::fs;
use std::path::{Path, PathBuf};

use athanor_core::{
    CanonicalSnapshotStore, KnowledgeStore, OperationContext, PreparedSnapshot,
    PreparedSnapshotPublication, SnapshotBatch,
};
use athanor_domain::{RepoId, SnapshotBase, SnapshotId};
use athanor_store_jsonl::JsonlKnowledgeStore;

use crate::index_publication::publish_prepared_index;
use crate::{
    AffectedFileSet, AthanorStore, IndexPipelineMetrics, IndexPipelineOutput, IndexStateStore,
};

#[tokio::test]
async fn read_model_prepare_failure_rolls_back_journal_and_snapshot() {
    let root = test_root("read-model-prepare");
    fs::create_dir_all(root.join(".athanor/state")).expect("create journal directory");
    fs::create_dir_all(root.join(".athanor/generated")).expect("create generated directory");
    fs::write(root.join(".athanor/generated/current"), "blocked")
        .expect("block read-model parent directory");
    let fixture = prepared_fixture(&root, "test.publication.read-model-prepare").await;
    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    let output_dir = root.join(".athanor/generated/current/jsonl");

    let error = publish_prepared_index(
        &root,
        &fixture.store,
        &state_store,
        &output_dir,
        &fixture.output,
        fixture.prepared,
        &fixture.operation,
    )
    .await
    .expect_err("read-model prepare failure must fail publication");

    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("failed to create"))
    );
    assert_snapshot_aborted(&fixture.store, &fixture.snapshot).await;
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove read-model prepare fixture");
}

#[tokio::test]
async fn index_state_prepare_failure_rolls_back_read_model_journal_and_snapshot() {
    let root = test_root("index-state-prepare");
    fs::create_dir_all(root.join(".athanor/state")).expect("create journal directory");
    let blocked_parent = root.join(".athanor/blocked-state");
    fs::write(&blocked_parent, "blocked").expect("block index-state parent directory");
    let fixture = prepared_fixture(&root, "test.publication.index-state-prepare").await;
    let state_store = IndexStateStore::new(blocked_parent.join("index-state.json"));
    let output_dir = root.join(".athanor/generated/current/jsonl");

    let error = publish_prepared_index(
        &root,
        &fixture.store,
        &state_store,
        &output_dir,
        &fixture.output,
        fixture.prepared,
        &fixture.operation,
    )
    .await
    .expect_err("index-state prepare failure must fail publication");

    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("failed to create"))
    );
    assert_snapshot_aborted(&fixture.store, &fixture.snapshot).await;
    assert!(!output_dir.exists(), "prepared read model must be rolled back");
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove index-state prepare fixture");
}

struct PreparedFixture {
    store: AthanorStore,
    snapshot: SnapshotId,
    prepared: PreparedSnapshot,
    output: IndexPipelineOutput,
    operation: OperationContext,
}

async fn prepared_fixture(root: &Path, operation_id: &str) -> PreparedFixture {
    let store = AthanorStore::new(JsonlKnowledgeStore::new(
        root.join(".athanor/store/canonical/jsonl"),
    ));
    let snapshot = store
        .begin_snapshot(
            RepoId(format!("repo_{operation_id}")),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin snapshot");
    store
        .put_snapshot(snapshot.clone(), SnapshotBatch::default())
        .await
        .expect("write snapshot batch");
    let operation = OperationContext::new(operation_id);
    let prepared = store
        .prepare_publication(snapshot.clone(), &operation)
        .await
        .expect("prepare snapshot");
    let output = IndexPipelineOutput {
        snapshot: snapshot.clone(),
        files: Vec::new(),
        entities: Vec::new(),
        facts: Vec::new(),
        relations: Vec::new(),
        diagnostics: Vec::new(),
        affected_files: AffectedFileSet::default(),
        metrics: IndexPipelineMetrics::default(),
    };

    PreparedFixture {
        store,
        snapshot,
        prepared,
        output,
        operation,
    }
}

async fn assert_snapshot_aborted(store: &AthanorStore, snapshot: &SnapshotId) {
    assert!(
        store
            .load_snapshot(snapshot)
            .await
            .expect("load snapshot after rollback")
            .is_none(),
        "prepared canonical snapshot must be removed"
    );
    assert!(
        store
            .load_latest_snapshot()
            .await
            .expect("load latest snapshot after rollback")
            .is_none()
    );
}

fn publication_journal(root: &Path) -> PathBuf {
    root.join(".athanor/state/index-publication.json")
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-publication-{label}-{nonce}"))
}
