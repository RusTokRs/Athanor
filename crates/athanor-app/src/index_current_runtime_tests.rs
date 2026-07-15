use std::fs;
use std::path::{Path, PathBuf};

use athanor_core::{CanonicalSnapshotStore, KnowledgeStore};
use athanor_domain::{RepoId, SnapshotBase};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde_json::json;

use crate::index_current::IndexCurrent;
use crate::index_publication::recover_interrupted_publication;
use crate::{AthanorStore, IndexOptions, index_project_with_composition};

#[tokio::test]
async fn production_index_publishes_valid_immutable_current_generation() {
    let root = test_root("published");
    fs::create_dir_all(root.join("src")).expect("create source directory");
    fs::write(root.join("src/lib.rs"), "pub fn index_current() {}\n").expect("write source file");
    let composition = crate::test_runtime::composition();

    let report = run_index(&root, &composition).await;
    let current = IndexCurrent::load(&root)
        .expect("load index current pointer")
        .expect("index current pointer exists");

    assert_eq!(current.snapshot().0.as_str(), report.snapshot.as_str());
    assert_eq!(
        current.generation().as_str(),
        format!("gen_{}", report.snapshot)
    );
    assert!(
        current
            .read_model_path(&root)
            .join("manifest.json")
            .is_file()
    );
    assert!(current.index_state_path(&root).is_file());
    assert!(
        root.join(".athanor/generated/current/jsonl/manifest.json")
            .is_file()
    );
    assert!(root.join(".athanor/state/index-state.json").is_file());
    assert!(!pointer_publication_journal(&root).exists());

    let second = run_index(&root, &composition).await;
    assert_eq!(second.snapshot, report.snapshot);
    assert_eq!(IndexCurrent::load(&root).unwrap(), Some(current));

    fs::remove_dir_all(root).expect("remove index current fixture");
}

#[tokio::test]
async fn committed_pointer_journal_recovers_immutable_generation_and_pointer() {
    let root = test_root("recovery");
    fs::create_dir_all(root.join("src")).expect("create source directory");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn recover_index_current() {}\n",
    )
    .expect("write source file");
    let composition = crate::test_runtime::composition();

    let report = run_index(&root, &composition).await;
    let current = IndexCurrent::load(&root)
        .expect("load index current pointer")
        .expect("index current pointer exists");
    fs::remove_file(IndexCurrent::path(&root)).expect("remove current pointer");
    fs::remove_dir_all(current.read_model_path(&root)).expect("remove immutable read model");
    fs::remove_file(current.index_state_path(&root)).expect("remove immutable index state");
    fs::write(
        pointer_publication_journal(&root),
        serde_json::to_vec_pretty(&json!({
            "schema": "athanor.index_current_publication.v1",
            "snapshot": report.snapshot.clone(),
            "generation": format!("gen_{}", report.snapshot)
        }))
        .unwrap(),
    )
    .expect("write pending pointer journal");

    let store = AthanorStore::new(JsonlKnowledgeStore::new(
        root.join(".athanor/store/canonical/jsonl"),
    ));
    recover_interrupted_publication(&root, &store)
        .await
        .expect("recover committed pointer publication");

    let recovered = IndexCurrent::load(&root)
        .expect("load recovered pointer")
        .expect("recovered pointer exists");
    assert_eq!(recovered.snapshot().0.as_str(), report.snapshot.as_str());
    assert!(
        recovered
            .read_model_path(&root)
            .join("manifest.json")
            .is_file()
    );
    assert!(recovered.index_state_path(&root).is_file());
    assert!(!pointer_publication_journal(&root).exists());

    fs::remove_dir_all(root).expect("remove pointer recovery fixture");
}

#[tokio::test]
async fn pointer_journal_without_legacy_journal_aborts_uncommitted_snapshot() {
    let root = test_root("orphan");
    fs::create_dir_all(root.join(".athanor/state")).expect("create state directory");
    let store = AthanorStore::new(JsonlKnowledgeStore::new(
        root.join(".athanor/store/canonical/jsonl"),
    ));
    let snapshot = store
        .begin_snapshot(
            RepoId("repo_pointer_orphan".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin orphan snapshot");
    fs::write(
        pointer_publication_journal(&root),
        serde_json::to_vec_pretty(&json!({
            "schema": "athanor.index_current_publication.v1",
            "snapshot": snapshot.0.clone(),
            "generation": format!("gen_{}", snapshot.0)
        }))
        .unwrap(),
    )
    .expect("write orphan pointer journal");

    recover_interrupted_publication(&root, &store)
        .await
        .expect("recover orphan pointer publication");

    assert!(store.load_snapshot(&snapshot).await.unwrap().is_none());
    assert!(!pointer_publication_journal(&root).exists());
    assert!(!IndexCurrent::path(&root).exists());

    fs::remove_dir_all(root).expect("remove pointer orphan fixture");
}

async fn run_index(root: &Path, composition: &crate::RuntimeComposition) -> crate::IndexReport {
    index_project_with_composition(
        IndexOptions {
            root: root.to_path_buf(),
            validation_report: None,
            validation_result: None,
            validate_only: false,
        },
        composition,
    )
    .await
    .expect("run production index")
}

fn pointer_publication_journal(root: &Path) -> PathBuf {
    root.join(".athanor/state/index-current-publication.json")
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-index-current-{label}-{nonce}"))
}
