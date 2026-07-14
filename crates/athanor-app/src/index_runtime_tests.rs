use std::fs;
use std::path::PathBuf;

use athanor_core::{CanonicalSnapshotStore, KnowledgeStore};
use athanor_store_jsonl::JsonlKnowledgeStore;

use crate::{IndexOptions, index_project_with_composition};

#[tokio::test]
async fn production_index_runtime_publishes_one_typed_generation() {
    let root = test_root("typed-runtime");
    fs::create_dir_all(root.join("src")).expect("create source directory");
    fs::write(root.join("src/lib.rs"), "pub fn typed_runtime() {}\n")
        .expect("write source file");

    let composition = crate::test_runtime::composition();
    let report = index_project_with_composition(
        IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        },
        &composition,
    )
    .await
    .expect("publish typed runtime index");

    let state: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join(".athanor/state/index-state.json")).expect("read index state"),
    )
    .expect("parse index state");
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join(".athanor/generated/current/jsonl/manifest.json"))
            .expect("read read-model manifest"),
    )
    .expect("parse read-model manifest");
    assert_eq!(state["snapshot"], report.snapshot);
    assert_eq!(manifest["snapshot"], report.snapshot);
    assert!(!root.join(".athanor/state/index-publication.json").exists());

    let store = JsonlKnowledgeStore::new(root.join(".athanor/store/canonical/jsonl"));
    let latest = store
        .load_latest_snapshot()
        .await
        .expect("load latest canonical snapshot")
        .expect("canonical snapshot exists");
    assert_eq!(
        latest.snapshot.as_ref().map(|snapshot| snapshot.0.as_str()),
        Some(report.snapshot.as_str())
    );

    let second = index_project_with_composition(
        IndexOptions {
            root: root.clone(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        },
        &composition,
    )
    .await
    .expect("repeat unchanged typed runtime index");
    assert_eq!(second.snapshot, report.snapshot);
    assert_eq!(second.changed_files, 0);
    assert_eq!(second.removed_files, 0);
    assert!(!root.join(".athanor/state/index-publication.json").exists());

    fs::remove_dir_all(root).expect("remove typed runtime test root");
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-index-{label}-{nonce}"))
}
