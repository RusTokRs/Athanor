use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshotStore, CoreError, KnowledgeStore, OperationContext,
    SnapshotBatch,
};
use athanor_domain::{Entity, EntityId, EntityKind, RepoId, SnapshotBase, StableKey};
use athanor_store_surrealdb::SurrealKnowledgeStore;
use serde_json::json;

#[tokio::test]
async fn publishes_complete_batch_and_commit_marker_in_one_transaction() {
    let store = SurrealKnowledgeStore::connect("mem://")
        .await
        .expect("connect in-memory SurrealDB");
    let snapshot = begin(&store, "repo_surreal_atomic_complete").await;

    store
        .put_entities(snapshot.clone(), vec![entity("ent_partial", "partial.md")])
        .await
        .expect("stage partial row");

    let committed = entity("ent_committed", "committed.md");
    store
        .publish_snapshot_batch_with_context(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![committed.clone()],
                ..SnapshotBatch::default()
            },
            &OperationContext::new("test.surreal.atomic-publication"),
        )
        .await
        .expect("publish rows and committed marker atomically");

    let exact = store
        .load_snapshot(&snapshot)
        .await
        .expect("load exact snapshot")
        .expect("committed exact snapshot exists");
    assert_eq!(exact.entities, vec![committed.clone()]);
    assert!(exact.entities.iter().all(|item| item.id.0 != "ent_partial"));

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
}

#[tokio::test]
async fn statement_failure_rolls_back_rows_and_commit_marker() {
    let store = SurrealKnowledgeStore::connect("mem://")
        .await
        .expect("connect in-memory SurrealDB");
    let snapshot = begin(&store, "repo_surreal_atomic_rollback").await;

    store
        .put_entities(snapshot.clone(), vec![entity("ent_partial", "partial.md")])
        .await
        .expect("stage partial row");
    let duplicate = entity("ent_duplicate", "duplicate.md");
    let error = store
        .publish_snapshot_batch(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![duplicate.clone(), duplicate],
                ..SnapshotBatch::default()
            },
        )
        .await
        .expect_err("duplicate record ids must rollback the atomic transaction");
    assert!(matches!(error, CoreError::Adapter(_)));

    let visibility = store
        .load_snapshot(&snapshot)
        .await
        .expect_err("rolled-back snapshot must remain uncommitted");
    assert!(matches!(visibility, CoreError::SnapshotNotCommitted(_)));

    let committed = entity("ent_after_rollback", "after-rollback.md");
    store
        .publish_snapshot_batch(
            snapshot.clone(),
            SnapshotBatch {
                entities: vec![committed.clone()],
                ..SnapshotBatch::default()
            },
        )
        .await
        .expect("retry valid complete batch after rollback");

    let exact = store
        .load_snapshot(&snapshot)
        .await
        .expect("load snapshot after retry")
        .expect("snapshot committed after retry");
    assert_eq!(exact.entities, vec![committed]);
}

async fn begin(store: &SurrealKnowledgeStore, repo: &str) -> athanor_domain::SnapshotId {
    store
        .begin_snapshot(
            RepoId(repo.to_string()),
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
