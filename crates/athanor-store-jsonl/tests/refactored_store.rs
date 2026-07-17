use athanor_core::{
    AtomicSnapshotPublication, CanonicalLatestPointer, CanonicalSnapshotStore, KnowledgeStore,
    SnapshotBatch,
};
use athanor_domain::{Entity, EntityId, EntityKind, RepoId, SnapshotBase, StableKey};
use athanor_store_jsonl::{JsonlKnowledgeStore, PathIndex, StableKeyIndex};
use serde_json::json;

#[tokio::test]
async fn strict_latest_is_written_and_legacy_pointer_remains_queryable_until_repair() {
    let root = test_root("latest");
    let store = JsonlKnowledgeStore::new(&root);
    let snapshot = store
        .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
        .await
        .unwrap();
    store
        .publish_snapshot_batch(snapshot.clone(), SnapshotBatch::default())
        .await
        .unwrap();

    let strict: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("latest.json")).unwrap()).unwrap();
    assert_eq!(strict["schema"], "athanor.canonical_latest.v1");
    assert_eq!(strict["snapshot"].as_str(), Some(snapshot.0.as_str()));
    let expected_generation = format!("gen_{}", snapshot.0);
    assert_eq!(
        strict["generation"].as_str(),
        Some(expected_generation.as_str())
    );

    std::fs::write(
        root.join("latest.json"),
        serde_json::to_vec_pretty(&json!({ "snapshot": snapshot.0.clone() })).unwrap(),
    )
    .unwrap();
    assert!(store.load_latest_snapshot().await.unwrap().is_some());
    assert!(store.load_latest_identity().await.is_err());

    let target = store.discover_latest_identity().await.unwrap().unwrap();
    store.repair_latest_identity(target.clone()).await.unwrap();
    assert_eq!(store.load_latest_identity().await.unwrap(), Some(target));
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn sequence_and_indexes_survive_the_module_split() {
    let root = test_root("indexes");
    let first = JsonlKnowledgeStore::new(&root);
    let second = JsonlKnowledgeStore::new(&root);
    let first_snapshot = first
        .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
        .await
        .unwrap();
    let second_snapshot = second
        .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
        .await
        .unwrap();
    assert_ne!(first_snapshot, second_snapshot);

    let entity = Entity {
        id: EntityId("ent_file_readme".to_string()),
        stable_key: StableKey("file://README.md".to_string()),
        kind: EntityKind::File,
        name: "README.md".to_string(),
        title: None,
        source: None,
        language: None,
        aliases: Vec::new(),
        ownership: Vec::new(),
        payload: json!({}),
    };
    first
        .put_entities(first_snapshot.clone(), vec![entity])
        .await
        .unwrap();
    first.commit_snapshot(first_snapshot.clone()).await.unwrap();

    let snapshot_dir = root.join("snapshots").join(first_snapshot.0);
    let stable: StableKeyIndex = serde_json::from_slice(
        &std::fs::read(snapshot_dir.join("stable_key_index.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(stable.entries["file://README.md"], "ent_file_readme");
    let paths: PathIndex =
        serde_json::from_slice(&std::fs::read(snapshot_dir.join("path_index.json")).unwrap())
            .unwrap();
    assert!(paths.entries.is_empty());

    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn startup_removes_only_known_staging_files() {
    let root = test_root("recovery");
    let snapshots = root.join("snapshots");
    std::fs::create_dir_all(snapshots.join(".snap_jsonl_00000099.staging-crash")).unwrap();
    std::fs::write(root.join(".latest.json.staging-crash"), "stale").unwrap();
    std::fs::write(root.join(".latest.json.identity-staging-crash"), "stale").unwrap();
    std::fs::write(root.join(".snapshot-sequence.staging-crash"), "stale").unwrap();
    std::fs::write(root.join(".unrelated.staging-crash"), "keep").unwrap();

    JsonlKnowledgeStore::new(&root)
        .begin_snapshot(RepoId("repo".to_string()), snapshot_base())
        .await
        .unwrap();

    assert!(!snapshots.join(".snap_jsonl_00000099.staging-crash").exists());
    assert!(!root.join(".latest.json.staging-crash").exists());
    assert!(!root.join(".latest.json.identity-staging-crash").exists());
    assert!(!root.join(".snapshot-sequence.staging-crash").exists());
    assert!(root.join(".unrelated.staging-crash").exists());
    std::fs::remove_dir_all(root).unwrap();
}

fn snapshot_base() -> SnapshotBase {
    SnapshotBase {
        branch: None,
        commit: None,
        parent_snapshot: None,
        working_tree: true,
    }
}

fn test_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "athanor-jsonl-refactor-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
