use std::fs;
use std::path::{Path, PathBuf};

use athanor_core::{CanonicalSnapshotStore, KnowledgeStore, SnapshotBatch};
use athanor_domain::{RepoId, SnapshotBase, SnapshotId};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde_json::json;

use crate::AthanorStore;
use crate::index_publication::recover_interrupted_publication;
use crate::index_publication_journal::INDEX_PUBLICATION_JOURNAL_SCHEMA_V2;
use crate::index_state::INDEX_STATE_SCHEMA;
use crate::read_model::JSONL_MANIFEST_SCHEMA;

#[tokio::test]
async fn recovery_rejects_committed_current_identity_mismatch_before_cleanup() {
    let fixture = recovery_fixture("committed-current-mismatch", true).await;
    write_read_model(&fixture.output_dir, "snap_wrong_current");
    write_index_state(&fixture.state_path, &fixture.snapshot.0);
    write_read_model(&fixture.read_backup(), "snap_previous");
    write_index_state(&fixture.state_backup(), "snap_previous");
    fixture.write_journal();

    let error = recover_interrupted_publication(&fixture.root, &fixture.store)
        .await
        .expect_err("committed current identity mismatch must fail closed");

    assert!(error.to_string().contains("current read model"));
    assert!(error.to_string().contains("does not match journal"));
    assert_eq!(
        read_model_snapshot(&fixture.output_dir),
        "snap_wrong_current"
    );
    assert!(fixture.read_backup().is_dir());
    assert!(fixture.state_backup().is_file());
    assert!(fixture.journal.is_file());
    assert_latest(&fixture.store, &fixture.snapshot).await;

    fs::remove_dir_all(fixture.root).expect("remove committed mismatch fixture");
}

#[tokio::test]
async fn recovery_rejects_uncommitted_replaced_current_mismatch_before_rollback() {
    let fixture = recovery_fixture("uncommitted-current-mismatch", false).await;
    write_read_model(&fixture.output_dir, "snap_unrelated_current");
    write_index_state(&fixture.state_path, &fixture.snapshot.0);
    write_read_model(&fixture.read_backup(), "snap_previous");
    write_index_state(&fixture.state_backup(), "snap_previous");
    fixture.write_journal();

    let error = recover_interrupted_publication(&fixture.root, &fixture.store)
        .await
        .expect_err("rollback must not replace an unrelated current generation");

    assert!(error.to_string().contains("replaced during rollback"));
    assert!(error.to_string().contains("does not match journal"));
    assert_eq!(
        read_model_snapshot(&fixture.output_dir),
        "snap_unrelated_current"
    );
    assert!(fixture.read_backup().is_dir());
    assert!(fixture.state_backup().is_file());
    assert!(fixture.journal.is_file());
    assert!(
        fixture
            .store
            .load_latest_snapshot()
            .await
            .expect("load latest snapshot")
            .is_none()
    );

    fs::remove_dir_all(fixture.root).expect("remove uncommitted mismatch fixture");
}

#[tokio::test]
async fn recovery_rejects_mixed_backup_generations_before_rollback() {
    let fixture = recovery_fixture("mixed-backups", false).await;
    write_read_model(&fixture.output_dir, &fixture.snapshot.0);
    write_index_state(&fixture.state_path, &fixture.snapshot.0);
    write_read_model(&fixture.read_backup(), "snap_previous_read");
    write_index_state(&fixture.state_backup(), "snap_previous_state");
    fixture.write_journal();

    let error = recover_interrupted_publication(&fixture.root, &fixture.store)
        .await
        .expect_err("mixed backup generations must fail closed");

    assert!(error.to_string().contains("backups"));
    assert!(error.to_string().contains("different snapshots"));
    assert_eq!(read_model_snapshot(&fixture.output_dir), fixture.snapshot.0);
    assert_eq!(state_snapshot(&fixture.state_path), fixture.snapshot.0);
    assert!(fixture.journal.is_file());

    fs::remove_dir_all(fixture.root).expect("remove mixed backup fixture");
}

#[tokio::test]
async fn uncommitted_recovery_is_idempotent_after_cleanup() {
    let fixture = recovery_fixture("uncommitted-idempotent", false).await;
    write_read_model(&fixture.output_dir, &fixture.snapshot.0);
    write_index_state(&fixture.state_path, &fixture.snapshot.0);
    fixture.write_journal();

    recover_interrupted_publication(&fixture.root, &fixture.store)
        .await
        .expect("recover uncommitted generation");
    recover_interrupted_publication(&fixture.root, &fixture.store)
        .await
        .expect("repeat uncommitted recovery");

    assert!(!fixture.output_dir.exists());
    assert!(!fixture.state_path.exists());
    assert!(!fixture.journal.exists());
    assert!(
        fixture
            .store
            .load_snapshot(&fixture.snapshot)
            .await
            .expect("load recovered snapshot")
            .is_none()
    );
    assert!(
        fixture
            .store
            .load_latest_snapshot()
            .await
            .expect("load latest snapshot")
            .is_none()
    );

    fs::remove_dir_all(fixture.root).expect("remove uncommitted idempotence fixture");
}

#[tokio::test]
async fn committed_recovery_is_idempotent_after_cleanup() {
    let fixture = recovery_fixture("committed-idempotent", true).await;
    write_read_model(&fixture.output_dir, &fixture.snapshot.0);
    write_index_state(&fixture.state_path, &fixture.snapshot.0);
    write_read_model(&fixture.read_backup(), "snap_previous");
    write_index_state(&fixture.state_backup(), "snap_previous");
    fixture.write_journal();

    recover_interrupted_publication(&fixture.root, &fixture.store)
        .await
        .expect("recover committed generation");
    recover_interrupted_publication(&fixture.root, &fixture.store)
        .await
        .expect("repeat committed recovery");

    assert_eq!(read_model_snapshot(&fixture.output_dir), fixture.snapshot.0);
    assert_eq!(state_snapshot(&fixture.state_path), fixture.snapshot.0);
    assert!(!fixture.read_backup().exists());
    assert!(!fixture.state_backup().exists());
    assert!(!fixture.journal.exists());
    assert_latest(&fixture.store, &fixture.snapshot).await;

    fs::remove_dir_all(fixture.root).expect("remove committed idempotence fixture");
}

struct RecoveryFixture {
    root: PathBuf,
    store: AthanorStore,
    snapshot: SnapshotId,
    id: String,
    output_dir: PathBuf,
    state_path: PathBuf,
    journal: PathBuf,
}

impl RecoveryFixture {
    fn read_backup(&self) -> PathBuf {
        self.output_dir
            .parent()
            .expect("read-model parent")
            .join(format!(".jsonl.backup-{}", self.id))
    }

    fn state_backup(&self) -> PathBuf {
        self.state_path
            .parent()
            .expect("index-state parent")
            .join(format!(".index-state.json.backup-{}", self.id))
    }

    fn write_journal(&self) {
        fs::create_dir_all(self.journal.parent().expect("journal parent"))
            .expect("create journal directory");
        fs::write(
            &self.journal,
            serde_json::to_vec_pretty(&json!({
                "schema": INDEX_PUBLICATION_JOURNAL_SCHEMA_V2,
                "prepared": self.snapshot.0.clone(),
                "id": self.id,
                "read_model": self.output_dir,
                "index_state": self.state_path,
            }))
            .expect("serialize journal"),
        )
        .expect("write journal");
    }
}

async fn recovery_fixture(label: &str, committed: bool) -> RecoveryFixture {
    let root = test_root(label);
    let store = AthanorStore::new(JsonlKnowledgeStore::new(
        root.join(".athanor/store/canonical/jsonl"),
    ));
    let snapshot = store
        .begin_snapshot(
            RepoId(format!("repo_recovery_content_{label}")),
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
        .expect("write snapshot");
    store
        .prepare_snapshot(snapshot.clone())
        .await
        .expect("prepare snapshot");
    if committed {
        store
            .commit_snapshot(snapshot.clone())
            .await
            .expect("commit snapshot");
    }

    RecoveryFixture {
        output_dir: root.join(".athanor/generated/current/jsonl"),
        state_path: root.join(".athanor/state/index-state.json"),
        journal: root.join(".athanor/state/index-publication.json"),
        root,
        store,
        snapshot,
        id: label.to_string(),
    }
}

fn write_read_model(path: &Path, snapshot: &str) {
    fs::create_dir_all(path).expect("create read model");
    fs::write(
        path.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schema": JSONL_MANIFEST_SCHEMA,
            "snapshot": snapshot
        }))
        .expect("serialize manifest"),
    )
    .expect("write manifest");
}

fn write_index_state(path: &Path, snapshot: &str) {
    fs::create_dir_all(path.parent().expect("state parent")).expect("create state directory");
    fs::write(
        path,
        serde_json::to_vec_pretty(&json!({
            "schema": INDEX_STATE_SCHEMA,
            "snapshot": snapshot,
            "files": {}
        }))
        .expect("serialize state"),
    )
    .expect("write state");
}

fn read_model_snapshot(path: &Path) -> String {
    serde_json::from_slice::<serde_json::Value>(
        &fs::read(path.join("manifest.json")).expect("read manifest"),
    )
    .expect("parse manifest")["snapshot"]
        .as_str()
        .expect("manifest snapshot")
        .to_string()
}

fn state_snapshot(path: &Path) -> String {
    serde_json::from_slice::<serde_json::Value>(&fs::read(path).expect("read state"))
        .expect("parse state")["snapshot"]
        .as_str()
        .expect("state snapshot")
        .to_string()
}

async fn assert_latest(store: &AthanorStore, expected: &SnapshotId) {
    let latest = store
        .load_latest_snapshot()
        .await
        .expect("load latest snapshot")
        .expect("latest snapshot exists");
    assert_eq!(latest.snapshot.as_ref(), Some(expected));
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-recovery-content-{label}-{nonce}"))
}
