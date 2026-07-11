//! Reusable async conformance checks for `KnowledgeStore` implementations.

use athanor_core::{
    CoreError, DiagnosticQuery, EntityQuery, EntityResolver, KnowledgeStore, RelationQuery,
    SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Relation, RelationId, RelationKind, RelationStatus, RepoId, Severity, SnapshotBase, SnapshotId,
    StableKey,
};
use serde_json::json;

/// Verifies committed-snapshot isolation, ID-based relation/diagnostic filtering, and stable-key
/// resolution for a store implementation.
pub async fn verify_query_contract<S>(store: &S)
where
    S: KnowledgeStore + EntityResolver,
{
    let first = begin(store).await;
    let first_entity = entity("ent_first", "file://first.md");
    store
        .put_entities(first.clone(), vec![first_entity.clone()])
        .await
        .expect("store first entity");
    store
        .put_relations(
            first.clone(),
            vec![relation("rel_first", &first, "ent_first", "ent_target")],
        )
        .await
        .expect("store first relation");
    store
        .put_diagnostics(
            first.clone(),
            vec![diagnostic("diag_first", &first, "ent_first")],
        )
        .await
        .expect("store first diagnostic");
    store
        .commit_snapshot(first.clone())
        .await
        .expect("commit first");

    let second = begin(store).await;
    let second_entity = entity("ent_second", "file://second.md");
    store
        .put_entities(second.clone(), vec![second_entity.clone()])
        .await
        .expect("store second entity");
    store
        .put_relations(
            second.clone(),
            vec![relation("rel_second", &second, "ent_second", "ent_target")],
        )
        .await
        .expect("store second relation");
    store
        .put_diagnostics(
            second.clone(),
            vec![diagnostic("diag_second", &second, "ent_second")],
        )
        .await
        .expect("store second diagnostic");
    store
        .commit_snapshot(second.clone())
        .await
        .expect("commit second");

    let first_relations = store
        .query_relations(
            SnapshotSelector::Exact(first.clone()),
            RelationQuery {
                from_entity: Some(EntityId("ent_first".to_string())),
                ..RelationQuery::default()
            },
        )
        .await
        .expect("query first relations");
    assert_eq!(first_relations.len(), 1);
    assert_eq!(first_relations[0].id.0, "rel_first");

    let latest_relations = store
        .query_relations(
            SnapshotSelector::LatestCommitted,
            RelationQuery {
                from_entity: Some(EntityId("ent_second".to_string())),
                ..RelationQuery::default()
            },
        )
        .await
        .expect("query latest relations");
    assert_eq!(latest_relations.len(), 1);
    assert_eq!(latest_relations[0].id.0, "rel_second");

    let diagnostics = store
        .query_diagnostics(
            SnapshotSelector::Exact(second.clone()),
            DiagnosticQuery {
                entity: Some(EntityId("ent_second".to_string())),
                ..DiagnosticQuery::default()
            },
        )
        .await
        .expect("query diagnostics");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id.0, "diag_second");

    let resolved = store
        .resolve_stable_key(
            SnapshotSelector::Exact(first.clone()),
            &StableKey("file://first.md".to_string()),
        )
        .await
        .expect("resolve stable key");
    assert_eq!(resolved, Some(first_entity.id));

    let uncommitted = begin(store).await;
    let error = store
        .query_entities(SnapshotSelector::Exact(uncommitted), EntityQuery::default())
        .await
        .expect_err("uncommitted snapshots must not be queryable");
    assert!(matches!(error, CoreError::SnapshotNotCommitted(_)));
}

async fn begin<S: KnowledgeStore>(store: &S) -> SnapshotId {
    store
        .begin_snapshot(
            RepoId("repo_conformance".to_string()),
            SnapshotBase {
                branch: None,
                commit: None,
                parent_snapshot: None,
                working_tree: true,
            },
        )
        .await
        .expect("begin snapshot")
}

fn entity(id: &str, stable_key: &str) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind: EntityKind::File,
        name: id.to_string(),
        title: None,
        source: None,
        language: None,
        aliases: Vec::new(),
        ownership: Vec::new(),
        payload: json!({}),
    }
}

fn relation(id: &str, snapshot: &SnapshotId, from: &str, to: &str) -> Relation {
    Relation {
        id: RelationId(id.to_string()),
        kind: RelationKind::Contains,
        from: EntityId(from.to_string()),
        to: EntityId(to.to_string()),
        status: RelationStatus::Verified,
        confidence: 1.0,
        evidence: Vec::new(),
        ownership: Vec::new(),
        snapshot: snapshot.clone(),
        payload: json!({}),
    }
}

fn diagnostic(id: &str, snapshot: &SnapshotId, entity: &str) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId(id.to_string()),
        kind: DiagnosticKind::Other("conformance".to_string()),
        severity: Severity::Low,
        status: DiagnosticStatus::Open,
        title: id.to_string(),
        message: id.to_string(),
        entities: vec![EntityId(entity.to_string())],
        evidence: Vec::new(),
        ownership: Vec::new(),
        snapshot: snapshot.clone(),
        suggested_fix: None,
        payload: json!({}),
    }
}
