use athanor_store_conformance::verify_query_contract;
use athanor_store_jsonl::JsonlKnowledgeStore;

#[tokio::test]
async fn satisfies_shared_query_contract() {
    let root = std::env::temp_dir().join(format!(
        "athanor-jsonl-conformance-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("current time")
            .as_nanos()
    ));
    verify_query_contract(&JsonlKnowledgeStore::new(&root)).await;
    std::fs::remove_dir_all(root).expect("remove temporary store");
}
