use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshotStore, CoreError, KnowledgeStore, OperationContext,
    SnapshotBatch,
};
use athanor_domain::{Entity, EntityId, EntityKind, RepoId, SnapshotBase, StableKey};
use athanor_store_memory::MemoryKnowledgeStore;
use serde_json::json;

#[tokio::test]
async fn publishes_complete_batch_and_commit_marker_in_one_boundary() {
    let store = MemoryKnowledgeStore::new();
    let snapshot = store
        .begin_snapshot(
            RepoId("repo_atomic_memory".to_string()),
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
        .put_entities(snapshot.clone(), vec![entity("ent_partial", "partial.md")])
        .await
        .expect("stage partial contents");
    let error = store
        .load_snapshot(&snapshot)
        .await
        .expect_err("staged data must remain uncommitted");
    assert!(matches!(error, CoreError::SnapshotNotCommitted(_)));

    let committed = entity("ent_committed", "committed.md");
    store
        .publish_snapshot_batch_with_context(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![committed.clone()],
                ..SnapshotBatch::default()
            },
            &OperationContext::new("test.memory.atomic-publication"),
        )
        .await
        .expect("atomically publish batch and marker");

    let exact = store
        .load_snapshot(&snapshot)
        .await
        .expect("load exact snapshot")
        .expect("committed snapshot exists");
    assert_eq!(exact.entities, vec![committed.clone()]);
    assert!(
        exact
            .entities
            .iter()
            .all(|entity| entity.id.0 != "ent_partial"),
        "partial staged contents must not leak into the committed generation"
    );

    let latest = store
        .load_latest_snapshot()
        .await
        .expect("load latest snapshot")
        .expect("latest committed snapshot exists");
    assert_eq!(latest.snapshot.as_ref(), Some(&snapshot));
    assert_eq!(latest.entities, vec![committed]);

    let republish = store
        .publish_snapshot_batch(snapshot.clone(), SnapshotBatch::default())
        .await
        .expect_err("committed snapshot must not be republished");
    assert!(matches!(republish, CoreError::Conflict(_)));

    let abort = store
        .abort_snapshot(snapshot)
        .await
        .expect_err("committed snapshot must not be aborted");
    assert!(matches!(abort, CoreError::Conflict(_)));
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
