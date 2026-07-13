use std::collections::HashSet;
use std::sync::Arc;

use athanor_core::KnowledgeStore;
use athanor_domain::{RepoId, SnapshotBase};
use athanor_store_conformance::{
    verify_query_contract, verify_snapshot_lifecycle_contract,
};
use athanor_store_surrealdb::SurrealKnowledgeStore;
use tokio::task::JoinSet;

#[tokio::test]
async fn satisfies_shared_query_contract() {
    let store = SurrealKnowledgeStore::connect("mem://")
        .await
        .expect("connect in-memory SurrealDB");
    verify_query_contract(&store).await;
}

#[tokio::test]
async fn satisfies_shared_snapshot_lifecycle_contract() {
    let store = SurrealKnowledgeStore::connect("mem://")
        .await
        .expect("connect in-memory SurrealDB");
    verify_snapshot_lifecycle_contract(&store).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cloned_handles_allocate_unique_snapshot_ids_concurrently() {
    let store = Arc::new(
        SurrealKnowledgeStore::connect("mem://")
            .await
            .expect("connect in-memory SurrealDB"),
    );
    let mut tasks = JoinSet::new();

    for index in 0..16 {
        let store = Arc::clone(&store);
        tasks.spawn(async move {
            store
                .begin_snapshot(
                    RepoId(format!("repo_conformance_{index}")),
                    SnapshotBase {
                        branch: None,
                        commit: None,
                        parent_snapshot: None,
                        working_tree: true,
                    },
                )
                .await
                .expect("begin concurrent snapshot")
        });
    }

    let mut ids = HashSet::new();
    while let Some(result) = tasks.join_next().await {
        let snapshot = result.expect("join concurrent begin task");
        assert!(ids.insert(snapshot.0), "snapshot ids must be unique");
    }

    assert_eq!(ids.len(), 16);
}
