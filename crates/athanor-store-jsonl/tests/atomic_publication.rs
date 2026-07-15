use std::fs;
use std::path::PathBuf;

use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshotStore, CoreError, EntityQuery, KnowledgeStore,
    OperationContext, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{Entity, EntityId, EntityKind, RepoId, SnapshotBase, StableKey};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde_json::{Value, json};

#[tokio::test]
async fn publishes_complete_exact_generation_with_commit_marker() {
    let root = test_root("complete");
    let store = JsonlKnowledgeStore::new(&root);
    let snapshot = begin(&store).await;

    store
        .put_entities(snapshot.clone(), vec![entity("ent_partial", "partial.md")])
        .await
        .expect("stage partial contents");
    let error = store
        .query_entities(
            SnapshotSelector::Exact(snapshot.clone()),
            EntityQuery::default(),
        )
        .await
        .expect_err("partial staged contents must remain uncommitted");
    assert!(matches!(error, CoreError::SnapshotNotCommitted(_)));

    let committed = entity("ent_committed", "committed.md");
    store
        .publish_snapshot_batch_with_context(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![committed.clone()],
                ..SnapshotBatch::default()
            },
            &OperationContext::new("test.jsonl.atomic-publication"),
        )
        .await
        .expect("publish complete batch and marker");

    let exact = store
        .load_snapshot(&snapshot)
        .await
        .expect("load exact snapshot")
        .expect("committed exact snapshot exists");
    assert_eq!(exact.entities, vec![committed.clone()]);
    assert!(exact.entities.iter().all(|item| item.id.0 != "ent_partial"));

    let marker_path = root
        .join("snapshots")
        .join(&snapshot.0)
        .join("commit.json");
    let marker: Value = serde_json::from_slice(&fs::read(marker_path).expect("read commit marker"))
        .expect("parse commit marker");
    assert_eq!(
        marker["schema"].as_str(),
        Some("athanor.canonical_commit.v1")
    );
    assert_eq!(marker["snapshot"].as_str(), Some(snapshot.0.as_str()));

    let latest = store
        .load_latest_snapshot()
        .await
        .expect("load latest snapshot")
        .expect("latest committed snapshot exists");
    assert_eq!(latest.snapshot.as_ref(), Some(&snapshot));
    assert_eq!(latest.entities, vec![committed]);

    assert!(matches!(
        store
            .publish_snapshot_batch(snapshot.clone(), SnapshotBatch::default())
            .await
            .expect_err("committed snapshot must not be republished"),
        CoreError::Conflict(_)
    ));
    assert!(matches!(
        store
            .abort_snapshot(snapshot)
            .await
            .expect_err("committed snapshot must not be aborted"),
        CoreError::Conflict(_)
    ));

    fs::remove_dir_all(root).expect("remove complete publication fixture");
}

#[tokio::test]
async fn latest_pointer_finalization_error_does_not_make_exact_generation_abortable() {
    let root = test_root("latest-finalize");
    let store = JsonlKnowledgeStore::new(&root);
    let snapshot = begin(&store).await;

    fs::create_dir_all(root.join("latest.json")).expect("block latest pointer cleanup");
    let error = store
        .publish_snapshot_batch(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![entity("ent_committed", "committed.md")],
                ..SnapshotBatch::default()
            },
        )
        .await
        .expect_err("latest pointer finalization must report its failure");
    assert!(error.to_string().contains("committed"));

    let exact = store
        .load_snapshot(&snapshot)
        .await
        .expect("load exact snapshot after pointer error")
        .expect("exact committed generation survives pointer error");
    assert_eq!(exact.snapshot.as_ref(), Some(&snapshot));
    assert_eq!(exact.entities.len(), 1);

    assert!(matches!(
        store
            .abort_snapshot(snapshot)
            .await
            .expect_err("durably committed generation must not be aborted"),
        CoreError::Conflict(_)
    ));

    fs::remove_dir_all(root).expect("remove pointer failure fixture");
}

async fn begin(store: &JsonlKnowledgeStore) -> athanor_domain::SnapshotId {
    store
        .begin_snapshot(
            RepoId("repo_atomic_jsonl".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin snapshot")
}

fn entity(id: &str, path: &str) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(format!("file://{path}")),
        kind: EntityKind::File,
        name: path.to_string(),
        title: None,
        source: None,
        language: None,
        aliases: Vec::new(),
        ownership: Vec::new(),
        payload: json!({}),
    }
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-jsonl-atomic-{label}-{nonce}"))
}
