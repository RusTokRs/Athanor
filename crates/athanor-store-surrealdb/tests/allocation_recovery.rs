use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshotStore, CoreError, KnowledgeStore, OperationContext,
    SnapshotBatch,
};
use athanor_domain::{RepoId, SnapshotBase, SnapshotId};
use athanor_store_surrealdb::SurrealKnowledgeStore;

#[tokio::test]
async fn cleanup_respects_cutoff_and_is_idempotent() {
    let store = SurrealKnowledgeStore::connect("mem://")
        .await
        .expect("connect in-memory SurrealDB");
    let repo = RepoId("repo_allocation_cutoff".to_string());
    let snapshot = allocate(&store, repo.clone(), "test.allocation.cutoff").await;

    let fresh = store
        .recover_orphan_snapshot_allocations(&repo, 0, 128)
        .await
        .expect("skip fresh allocation");
    assert!(fresh.is_empty());
    assert!(matches!(
        store
            .load_snapshot(&snapshot)
            .await
            .expect_err("fresh allocation remains uncommitted"),
        CoreError::SnapshotNotCommitted(_)
    ));

    let removed = store
        .recover_orphan_snapshot_allocations(&repo, u64::MAX, 128)
        .await
        .expect("remove stale allocation");
    assert_eq!(removed, vec![snapshot.clone()]);
    assert!(matches!(
        store
            .load_snapshot(&snapshot)
            .await
            .expect_err("removed allocation no longer exists"),
        CoreError::NotFound(_)
    ));
    assert!(
        store
            .recover_orphan_snapshot_allocations(&repo, u64::MAX, 128)
            .await
            .expect("repeat orphan cleanup")
            .is_empty()
    );
}

#[tokio::test]
async fn cleanup_is_bounded_by_requested_limit() {
    let store = SurrealKnowledgeStore::connect("mem://")
        .await
        .expect("connect in-memory SurrealDB");
    let repo = RepoId("repo_allocation_limit".to_string());
    let first = allocate(&store, repo.clone(), "test.allocation.limit.1").await;
    let second = allocate(&store, repo.clone(), "test.allocation.limit.2").await;
    let third = allocate(&store, repo.clone(), "test.allocation.limit.3").await;

    let removed = store
        .recover_orphan_snapshot_allocations(&repo, u64::MAX, 2)
        .await
        .expect("remove bounded orphan batch");
    assert_eq!(removed.len(), 2);
    assert!(removed.contains(&first));
    assert!(removed.contains(&second));

    let remaining = store
        .recover_orphan_snapshot_allocations(&repo, u64::MAX, 2)
        .await
        .expect("remove remaining orphan");
    assert_eq!(remaining, vec![third]);
}

#[tokio::test]
async fn cleanup_never_removes_prepared_or_committed_allocations() {
    let store = SurrealKnowledgeStore::connect("mem://")
        .await
        .expect("connect in-memory SurrealDB");
    let repo = RepoId("repo_allocation_protected".to_string());

    let prepared = allocate(&store, repo.clone(), "test.allocation.prepared").await;
    store
        .prepare_snapshot(prepared.clone())
        .await
        .expect("prepare protected allocation");

    let committed = allocate(&store, repo.clone(), "test.allocation.committed").await;
    store
        .publish_snapshot_batch(committed.clone(), SnapshotBatch::default())
        .await
        .expect("commit protected allocation");

    assert!(
        store
            .recover_orphan_snapshot_allocations(&repo, u64::MAX, 128)
            .await
            .expect("scan protected allocations")
            .is_empty()
    );
    assert!(matches!(
        store
            .load_snapshot(&prepared)
            .await
            .expect_err("prepared allocation remains hidden"),
        CoreError::SnapshotNotCommitted(_)
    ));
    assert_eq!(
        store
            .load_snapshot(&committed)
            .await
            .expect("load committed allocation")
            .expect("committed allocation exists")
            .snapshot,
        Some(committed.clone())
    );

    store
        .abort_snapshot(prepared)
        .await
        .expect("remove prepared test allocation");
}

async fn allocate(
    store: &SurrealKnowledgeStore,
    repo: RepoId,
    operation_id: &str,
) -> SnapshotId {
    store
        .begin_snapshot_allocation(
            repo,
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
            &OperationContext::new(operation_id),
        )
        .await
        .expect("allocate context-owned snapshot")
}
