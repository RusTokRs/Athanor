use std::future::Future;

use athanor_store_conformance::{
    verify_query_contract, verify_snapshot_lifecycle_contract,
};
use athanor_store_jsonl::JsonlKnowledgeStore;

#[tokio::test]
async fn satisfies_shared_query_contract() {
    with_store("query", |store| async move {
        verify_query_contract(&store).await;
    })
    .await;
}

#[tokio::test]
async fn satisfies_shared_snapshot_lifecycle_contract() {
    with_store("lifecycle", |store| async move {
        verify_snapshot_lifecycle_contract(&store).await;
    })
    .await;
}

async fn with_store<F, Fut>(name: &str, test: F)
where
    F: FnOnce(JsonlKnowledgeStore) -> Fut,
    Fut: Future<Output = ()>,
{
    let root = std::env::temp_dir().join(format!(
        "athanor-jsonl-conformance-{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("current time")
            .as_nanos()
    ));
    test(JsonlKnowledgeStore::new(&root)).await;
    std::fs::remove_dir_all(root).expect("remove temporary store");
}
