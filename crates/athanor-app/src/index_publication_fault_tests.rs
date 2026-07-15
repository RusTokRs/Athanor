use std::fs;
use std::path::{Path, PathBuf};

use athanor_core::{
    CanonicalSnapshotStore, KnowledgeStore, OperationContext, OperationContextCancellation,
    SnapshotBatch,
};
use athanor_domain::{RepoId, SnapshotBase, SnapshotId};
use athanor_store_jsonl::JsonlKnowledgeStore;

use crate::index_publication::publish_index_snapshot;
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
    let fixture = snapshot_fixture(&root, "test.publication.read-model-prepare").await;
    let state_store = IndexStateStore::new(root.join(".athanor/state/index-state.json"));
    let output_dir = root.join(".athanor/generated/current/jsonl");

    let error = publish_index_snapshot(
        &root,
        &fixture.store,
        &state_store,
        &output_dir,
        &fixture.output,
        fixture.snapshot.clone(),
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
    let fixture = snapshot_fixture(&root, "test.publication.index-state-prepare").await;
    let state_store = IndexStateStore::new(blocked_parent.join("index-state.json"));
    let output_dir = root.join(".athanor/generated/current/jsonl");

    let error = publish_index_snapshot(
        &root,
        &fixture.store,
        &state_store,
        &output_dir,
        &fixture.output,
        fixture.snapshot.clone(),
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
    assert!(
        !output_dir.exists(),
        "prepared read model must be rolled back"
    );
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove index-state prepare fixture");
}

#[tokio::test]
async fn cancelled_canonical_publish_restores_previous_artifacts_and_aborts_snapshot() {
    let root = test_root("canonical-publish-cancelled");
    let fixture = snapshot_fixture(&root, "test.publication.canonical-publish").await;
    let output_dir = root.join(".athanor/generated/current/jsonl");
    let state_path = root.join(".athanor/state/index-state.json");
    fs::create_dir_all(&output_dir).expect("create previous read model");
    fs::create_dir_all(state_path.parent().expect("state parent"))
        .expect("create previous state directory");
    fs::write(
        output_dir.join("manifest.json"),
        r#"{"snapshot":"snap_previous"}"#,
    )
    .expect("write previous read-model manifest");
    fs::write(&state_path, "previous-state").expect("write previous index state");
    let cancellation = fixture
        .operation
        .cancellation_handle()
        .expect("register publication cancellation");
    cancellation.cancel();
    let state_store = IndexStateStore::new(&state_path);

    let error = publish_index_snapshot(
        &root,
        &fixture.store,
        &state_store,
        &output_dir,
        &fixture.output,
        fixture.snapshot.clone(),
        &fixture.operation,
    )
    .await
    .expect_err("cancelled canonical publish must fail");

    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("was cancelled"))
    );
    assert_snapshot_aborted(&fixture.store, &fixture.snapshot).await;
    assert_eq!(
        fs::read_to_string(output_dir.join("manifest.json"))
            .expect("read restored read-model manifest"),
        r#"{"snapshot":"snap_previous"}"#
    );
    assert_eq!(
        fs::read_to_string(&state_path).expect("read restored index state"),
        "previous-state"
    );
    assert!(!publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove canonical publish fixture");
}

struct SnapshotFixture {
    store: AthanorStore,
    snapshot: SnapshotId,
    output: IndexPipelineOutput,
    operation: OperationContext,
}

async fn snapshot_fixture(root: &Path, operation_id: &str) -> SnapshotFixture {
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

    SnapshotFixture {
        store,
        snapshot,
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
        "canonical snapshot allocation must be removed"
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
