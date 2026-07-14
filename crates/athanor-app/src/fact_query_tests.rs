use std::fs;
use std::path::PathBuf;

use athanor_core::{
    CoreError, FactQuery, FactQueryStore, KnowledgeStore, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    EntityId, Fact, FactId, FactKind, RepoId, SnapshotBase, SnapshotId,
};
use athanor_store_jsonl::JsonlKnowledgeStore;
use serde_json::json;

use crate::AthanorStore;

#[tokio::test]
async fn fact_queries_are_committed_only_and_filter_consistently() {
    let root = test_root("filters");
    let store = AthanorStore::new(JsonlKnowledgeStore::new(&root));
    let snapshot = store
        .begin_snapshot(
            RepoId("repo_fact_query".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin snapshot");
    let route = fact(
        "fact_route",
        FactKind::RouteDeclared,
        "api_login",
        Some("schema_login"),
        "openapi",
        &snapshot,
    );
    let symbol = fact(
        "fact_symbol",
        FactKind::SymbolDefined,
        "rust_login",
        None,
        "rust",
        &snapshot,
    );
    store
        .put_snapshot(
            snapshot.clone(),
            SnapshotBatch {
                facts: vec![route.clone(), symbol],
                ..SnapshotBatch::default()
            },
        )
        .await
        .expect("write facts");

    let error = store
        .query_facts(
            SnapshotSelector::Exact(snapshot.clone()),
            FactQuery::default(),
        )
        .await
        .expect_err("uncommitted facts must not be queryable");
    assert!(matches!(
        error,
        CoreError::NotFound(_) | CoreError::SnapshotNotCommitted(_)
    ));

    store
        .commit_snapshot(snapshot.clone())
        .await
        .expect("commit snapshot");

    let found = store
        .query_facts(
            SnapshotSelector::LatestCommitted,
            FactQuery {
                subject: Some(EntityId("api_login".to_string())),
                object: Some(EntityId("schema_login".to_string())),
                kind: Some("route_declared".to_string()),
                extractor: Some("openapi".to_string()),
                limit: Some(1),
            },
        )
        .await
        .expect("query committed facts");
    assert_eq!(found, vec![route]);

    let exact = store
        .query_facts(
            SnapshotSelector::Exact(snapshot),
            FactQuery {
                extractor: Some("rust".to_string()),
                ..FactQuery::default()
            },
        )
        .await
        .expect("query exact committed snapshot");
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].id.0, "fact_symbol");

    fs::remove_dir_all(root).expect("remove fact query fixture");
}

#[tokio::test]
async fn latest_fact_query_without_a_committed_snapshot_is_empty() {
    let root = test_root("empty-latest");
    let store = AthanorStore::new(JsonlKnowledgeStore::new(&root));

    assert!(
        store
            .query_facts(SnapshotSelector::LatestCommitted, FactQuery::default())
            .await
            .expect("query empty latest snapshot")
            .is_empty()
    );

    if root.exists() {
        fs::remove_dir_all(root).expect("remove empty fact query fixture");
    }
}

fn fact(
    id: &str,
    kind: FactKind,
    subject: &str,
    object: Option<&str>,
    extractor: &str,
    snapshot: &SnapshotId,
) -> Fact {
    Fact {
        id: FactId(id.to_string()),
        kind,
        subject: EntityId(subject.to_string()),
        object: object.map(|object| EntityId(object.to_string())),
        value: json!({ "verified": true }),
        evidence: Vec::new(),
        ownership: Vec::new(),
        snapshot: snapshot.clone(),
        extractor: extractor.to_string(),
        confidence: 1.0,
    }
}

fn test_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-fact-query-{label}-{nonce}"))
}
