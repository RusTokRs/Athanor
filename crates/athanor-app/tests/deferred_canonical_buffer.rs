use std::fs;
use std::path::PathBuf;

use athanor_app::{
    AthanorStore, IncrementalIndexContext, IndexPipeline, PreparedSnapshotPublication,
};
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
            working_tree_base(),
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

    assert_unpublished(&store, &canonical_root, &snapshot).await;

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
    assert!(
        canonical_root
            .join("snapshots")
            .join(&snapshot.0)
            .join("commit.json")
            .is_file()
    );
    assert!(matches!(
        store
            .abort_prepared(&prepared)
            .await
            .expect_err("committed exact generation must not be aborted"),
        CoreError::Conflict(_)
    ));

    fs::remove_dir_all(root).expect("remove deferred buffer fixture");
}

#[tokio::test]
async fn deferred_pipeline_returns_complete_output_without_canonical_prewrite() {
    let root = test_root("pipeline-buffer");
    let canonical_root = root.join("canonical");
    let store = AthanorStore::new(JsonlKnowledgeStore::new(&canonical_root));
    let operation = OperationContext::new("test.deferred-pipeline-buffer");

    let output = IndexPipeline::new(store.clone())
        .run_with_incremental_deferred_operation_context(
            RepoId("repo_deferred_pipeline".to_string()),
            working_tree_base(),
            IncrementalIndexContext::default(),
            operation.clone(),
        )
        .await
        .expect("run deferred pipeline");

    assert_unpublished(&store, &canonical_root, &output.snapshot).await;

    store
        .publish_snapshot_batch_with_context(
            output.snapshot.clone(),
            SnapshotBatch {
                entities: output.entities.clone(),
                facts: output.facts.clone(),
                relations: output.relations.clone(),
                diagnostics: output.diagnostics.clone(),
            },
            &operation,
        )
        .await
        .expect("publish deferred pipeline output atomically");

    let exact = store
        .load_snapshot(&output.snapshot)
        .await
        .expect("load published pipeline snapshot")
        .expect("published pipeline snapshot exists");
    assert_eq!(exact.entities, output.entities);
    assert_eq!(exact.facts, output.facts);
    assert_eq!(exact.relations, output.relations);
    assert_eq!(exact.diagnostics, output.diagnostics);

    fs::remove_dir_all(root).expect("remove deferred pipeline fixture");
}

async fn assert_unpublished(
    store: &AthanorStore,
    canonical_root: &std::path::Path,
    snapshot: &athanor_domain::SnapshotId,
) {
    assert!(
        store
            .load_snapshot(snapshot)
            .await
            .expect("probe exact snapshot before publication")
            .is_none(),
        "pending context batch must not create an exact canonical generation"
    );
    let snapshots = canonical_root.join("snapshots");
    assert!(!snapshots.join(&snapshot.0).exists());
    assert!(!snapshots.join(format!(".{}.prepared", snapshot.0)).exists());
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

fn working_tree_base() -> SnapshotBase {
    SnapshotBase {
        branch: None,
        commit: None,
        parent_snapshot: None,
        working_tree: true,
    }
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-{label}-{nonce}"))
}
