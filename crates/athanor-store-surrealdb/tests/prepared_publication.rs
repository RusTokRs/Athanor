use athanor_core::{
    CanonicalSnapshotStore, KnowledgeStore, OperationContext, PreparedSnapshotPublication,
    SnapshotBatch,
};
use athanor_domain::{RepoId, SnapshotBase};
use athanor_store_surrealdb::SurrealKnowledgeStore;

#[tokio::test]
async fn typed_prepare_publish_and_abort_preserve_latest_committed() {
    let store = SurrealKnowledgeStore::connect("mem://")
        .await
        .expect("connect in-memory SurrealDB");
    let repo = RepoId("repo_surreal_prepared_publication".to_string());

    let published = store
        .begin_snapshot(repo.clone(), working_tree_base())
        .await
        .expect("begin published snapshot");
    let publish_context = OperationContext::new("test.surreal.publish");
    store
        .put_snapshot_with_context(
            published.clone(),
            SnapshotBatch::default(),
            &publish_context,
        )
        .await
        .expect("write published snapshot");
    let prepared = store
        .prepare_publication(published.clone(), &publish_context)
        .await
        .expect("prepare published snapshot");
    assert_eq!(prepared.snapshot(), &published);
    store
        .publish_prepared(&prepared, &publish_context)
        .await
        .expect("publish prepared snapshot");

    let latest = store
        .load_latest_snapshot()
        .await
        .expect("load latest published snapshot")
        .expect("published snapshot exists");
    assert_eq!(latest.snapshot.as_ref(), Some(&published));

    let aborted = store
        .begin_snapshot(repo, working_tree_base())
        .await
        .expect("begin aborted snapshot");
    let abort_context = OperationContext::new("test.surreal.abort");
    store
        .put_snapshot_with_context(
            aborted.clone(),
            SnapshotBatch::default(),
            &abort_context,
        )
        .await
        .expect("write aborted snapshot");
    let prepared = store
        .prepare_publication(aborted, &abort_context)
        .await
        .expect("prepare aborted snapshot");
    store
        .abort_prepared(&prepared)
        .await
        .expect("abort prepared snapshot");

    let latest_after_abort = store
        .load_latest_snapshot()
        .await
        .expect("load latest snapshot after abort")
        .expect("published snapshot remains visible");
    assert_eq!(latest_after_abort.snapshot.as_ref(), Some(&published));
}

fn working_tree_base() -> SnapshotBase {
    SnapshotBase {
        branch: None,
        commit: None,
        parent_snapshot: None,
        working_tree: true,
    }
}
