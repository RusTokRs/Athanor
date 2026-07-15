use std::fs;
use std::path::PathBuf;

use athanor_app::{AthanorStore, PreparedSnapshotPublication};
use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshotStore, CoreError, KnowledgeStore, OperationContext,
    SnapshotBatch,
};
use athanor_domain::{Entity, EntityId, EntityKind, RepoId, SnapshotBase, StableKey};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde_json::json;

#[tokio::test]
async fn deferred_context_batch_does_not_mutate_jsonl_before_atomic_boundary() {
    let root = test_root("jsonl-buffer");
    let canonical_root = root.join("canonical");
    let store = AthanorStore::new(JsonlKnowledgeStore::new(&canonical_root));
    let snapshot = store
        .begin_snapshot(
            RepoId("repo_deferred_buffer".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin deferred snapshot");
    let operation = OperationContext::new("test.deferred-canonical-buffer");

    store
        .put_snapshot_with_context(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![entity("ent_pending", "pending.md")],
                ..SnapshotBatch::default()
            },
            &operation,
        )
        .await
        .expect("buffer pending batch");
    let prepared = store
        .prepare_publication(snapshot.clone(), &operation)
        .await
        .expect("create compatibility cleanup handle");

    assert!(
        store
            .load_snapshot(&snapshot)
            .await
            .expect("probe exact snapshot before publication")
            .is_none(),
        "pending context batch must not create an exact canonical generation"
    );
    let snapshots = canonical_root.join("snapshots");
    assert!(!snapshots.join(&snapshot.0).exists());
    assert!(!snapshots.join(format!(".{}.prepared", snapshot.0)).exists());

    let committed = entity("ent_committed", "committed.md");
    store
        .publish_snapshot_batch_with_context(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![committed.clone()],
                ..SnapshotBatch::default()
            },
            &operation,
        )
        .await
        .expect("publish coordinator batch atomically");

    let exact = store
        .load_snapshot(&snapshot)
        .await
        .expect("load exact committed snapshot")
        .expect("committed exact snapshot exists");
    assert_eq!(exact.entities, vec![committed]);
    assert!(
        exact
            .entities
            .iter()
            .all(|entity| entity.id.0 != "ent_pending")
    );
    assert!(snapshots.join(&snapshot.0).join("commit.json").is_file());
    assert!(matches!(
        store
            .abort_prepared(&prepared)
            .await
            .expect_err("committed exact generation must not be aborted"),
        CoreError::Conflict(_)
    ));

    fs::remove_dir_all(root).expect("remove deferred buffer fixture");
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
    std::env::temp_dir().join(format!("athanor-{label}-{nonce}"))
}
