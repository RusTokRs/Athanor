use athanor_store_conformance::{
    verify_atomic_publication_contract, verify_query_contract, verify_snapshot_lifecycle_contract,
};
use athanor_store_memory::MemoryKnowledgeStore;

#[tokio::test]
async fn satisfies_shared_query_contract() {
    verify_query_contract(&MemoryKnowledgeStore::new()).await;
}

#[tokio::test]
async fn satisfies_shared_snapshot_lifecycle_contract() {
    verify_snapshot_lifecycle_contract(&MemoryKnowledgeStore::new()).await;
}

#[tokio::test]
async fn satisfies_shared_atomic_publication_contract() {
    verify_atomic_publication_contract(&MemoryKnowledgeStore::new()).await;
}
