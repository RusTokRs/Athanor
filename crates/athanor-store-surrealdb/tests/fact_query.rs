use athanor_core::{
    CoreError, FactQuery, FactQueryStore, KnowledgeStore, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{EntityId, Fact, FactId, FactKind, RepoId, SnapshotBase, SnapshotId};
use athanor_store_surrealdb::SurrealKnowledgeStore;
use serde_json::json;

#[tokio::test]
async fn fact_query_filters_committed_surreal_snapshot() {
    let store = SurrealKnowledgeStore::connect("mem://")
        .await
        .expect("connect in-memory SurrealDB");
    let snapshot = store
        .begin_snapshot(
            RepoId("repo_surreal_fact_query".to_string()),
            working_tree_base(),
        )
        .await
        .expect("begin snapshot");
    let expected = fact(
        "fact_route",
        FactKind::RouteDeclared,
        "api_login",
        Some("schema_login"),
        "openapi",
        &snapshot,
    );
    store
        .put_snapshot(
            snapshot.clone(),
            SnapshotBatch {
                facts: vec![
                    expected.clone(),
                    fact(
                        "fact_symbol",
                        FactKind::SymbolDefined,
                        "rust_login",
                        None,
                        "rust",
                        &snapshot,
                    ),
                ],
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
        .expect_err("uncommitted snapshot must not expose facts");
    assert!(matches!(error, CoreError::SnapshotNotCommitted(_)));

    store
        .commit_snapshot(snapshot)
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
        .expect("query facts");
    assert_eq!(found, vec![expected]);
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

fn working_tree_base() -> SnapshotBase {
    SnapshotBase {
        branch: None,
        commit: None,
        parent_snapshot: None,
        working_tree: true,
    }
}
