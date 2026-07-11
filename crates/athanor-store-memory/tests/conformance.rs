use athanor_store_conformance::verify_query_contract;
use athanor_store_memory::MemoryKnowledgeStore;

#[tokio::test]
async fn satisfies_shared_query_contract() {
    verify_query_contract(&MemoryKnowledgeStore::new()).await;
}
