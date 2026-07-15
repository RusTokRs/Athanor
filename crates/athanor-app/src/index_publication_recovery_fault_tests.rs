use std::fs;
use std::path::PathBuf;

use athanor_core::{CanonicalSnapshotStore, KnowledgeStore, SnapshotBatch};
use athanor_domain::{RepoId, SnapshotBase, SnapshotId};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde_json::json;

use crate::index_publication::recover_interrupted_publication;
use crate::index_publication_journal::INDEX_PUBLICATION_JOURNAL_SCHEMA_V2;
use crate::index_state::INDEX_STATE_SCHEMA;
use crate::read_model::JSONL_MANIFEST_SCHEMA;
use crate::AthanorStore;

#[tokio::test]
async fn recovery_rejects_file_read_model_backup_before_mutation() {
    let fixture = prepared_recovery_fixture("corrupt-read-backup").await;
    let backup = read_model_backup(&fixture, "corrupt-read-backup");
    fs::write(&backup, "not a directory").expect("write malformed read-model backup");

    let error = recover_interrupted_publication(&fixture.root, &fixture.store)
        .await
        .expect_err("malformed read-model backup must fail closed");

    assert!(error.to_string().contains("read model backup"));
    assert!(error.to_string().contains("must be a directory"));
    assert_current_artifacts(&fixture);
    assert!(backup.is_file());
    assert!(fixture.journal.is_file());
    assert!(
        fixture
            .store
            .load_latest_snapshot()
            .await
            .expect("load latest snapshot")
            .is_none()
    );

    fs::remove_dir_all(fixture.root).expect("remove read-model recovery fixture");
}

#[tokio::test]
async fn recovery_rejects_directory_index_state_backup_before_mutation() {
    let fixture = prepared_recovery_fixture("corrupt-state-backup").await;
    let backup = index_state_backup(&fixture, "corrupt-state-backup");
    fs::create_dir(&backup).expect("create malformed index-state backup");

    let error = recover_interrupted_publication(&fixture.root, &fixture.store)
        .await
        .expect_err("malformed index-state backup must fail closed");

    assert!(error.to_string().contains("index state backup"));
    assert!(error.to_string().contains("must be a regular file"));
    assert_current_artifacts(&fixture);
    assert!(backup.is_dir());
    assert!(fixture.journal.is_file());
    assert!(
        fixture
            .store
            .load_latest_snapshot()
            .await
            .expect("load latest snapshot")
            .is_none()
    );

    fs::remove_dir_all(fixture.root).expect("remove index-state recovery fixture");
}

#[tokio::test]
async fn recovery_without_backups_removes_uncommitted_generation() {
    let fixture = prepared_recovery_fixture("missing-backups").await;
    let prepared_dir = fixture
        .root
        .join(".athanor/store/canonical/jsonl/snapshots")
        .join(format!(".{}.prepared", fixture.snapshot.0));
    assert!(prepared_dir.is_dir());

    recover_interrupted_publication(&fixture.root, &fixture.store)
        .await
        .expect("recover uncommitted generation without backups");

    assert!(!fixture.output_dir.exists());
    assert!(!fixture.state_path.exists());
    assert!(!fixture.journal.exists());
    assert!(!prepared_dir.exists());
    assert!(
        fixture
            .store
            .load_latest_snapshot()
            .await
            .expect("load latest snapshot")
            .is_none()
    );

    fs::remove_dir_all(fixture.root).expect("remove missing-backup recovery fixture");
}

struct RecoveryFixture {
    root: PathBuf,
    store: AthanorStore,
    snapshot: SnapshotId,
    output_dir: PathBuf,
    state_path: PathBuf,
    journal: PathBuf,
}

async fn prepared_recovery_fixture(id: &str) -> RecoveryFixture {
    let root = test_root(id);
    let output_dir = root.join(".athanor/generated/current/jsonl");
    let state_path = root.join(".athanor/state/index-state.json");
    let journal = root.join(".athanor/state/index-publication.json");
    fs::create_dir_all(&output_dir).expect("create current read model");
    fs::create_dir_all(state_path.parent().expect("state parent"))
        .expect("create state directory");

    let store = AthanorStore::new(JsonlKnowledgeStore::new(
        root.join(".athanor/store/canonical/jsonl"),
    ));
    let snapshot = store
        .begin_snapshot(
            RepoId(format!("repo_recovery_{id}")),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin recovery snapshot");
    store
        .put_snapshot(snapshot.clone(), SnapshotBatch::default())
        .await
        .expect("write recovery snapshot");
    store
        .prepare_snapshot(snapshot.clone())
        .await
        .expect("prepare recovery snapshot");

    fs::write(
        output_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schema": JSONL_MANIFEST_SCHEMA,
            "snapshot": snapshot.0.clone()
        }))
        .expect("serialize manifest"),
    )
    .expect("write current manifest");
    fs::write(
        &state_path,
        serde_json::to_vec_pretty(&json!({
            "schema": INDEX_STATE_SCHEMA,
            "snapshot": snapshot.0.clone(),
            "files": {}
        }))
        .expect("serialize state"),
    )
    .expect("write current state");
    fs::write(
        &journal,
        serde_json::to_vec_pretty(&json!({
            "schema": INDEX_PUBLICATION_JOURNAL_SCHEMA_V2,
            "prepared": snapshot.0.clone(),
            "id": id,
            "read_model": output_dir.clone(),
            "index_state": state_path.clone(),
        }))
        .expect("serialize recovery journal"),
    )
    .expect("write recovery journal");

    RecoveryFixture {
        root,
        store,
        snapshot,
        output_dir,
        state_path,
        journal,
    }
}

fn assert_current_artifacts(fixture: &RecoveryFixture) {
    assert!(fixture.output_dir.is_dir());
    assert!(fixture.state_path.is_file());
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.output_dir.join("manifest.json")).expect("read current manifest"),
    )
    .expect("parse current manifest");
    let state: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.state_path).expect("read current state"))
            .expect("parse current state");
    assert_eq!(
        manifest["snapshot"].as_str(),
        Some(fixture.snapshot.0.as_str())
    );
    assert_eq!(
        state["snapshot"].as_str(),
        Some(fixture.snapshot.0.as_str())
    );
}

fn read_model_backup(fixture: &RecoveryFixture, id: &str) -> PathBuf {
    fixture
        .output_dir
        .parent()
        .expect("read-model parent")
        .join(format!(".jsonl.backup-{id}"))
}

fn index_state_backup(fixture: &RecoveryFixture, id: &str) -> PathBuf {
    fixture
        .state_path
        .parent()
        .expect("index-state parent")
        .join(format!(".index-state.json.backup-{id}"))
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-recovery-{label}-{nonce}"))
}
