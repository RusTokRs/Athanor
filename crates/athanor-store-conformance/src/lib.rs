//! Reusable async conformance checks for `KnowledgeStore` implementations.

use athanor_core::{
    CoreError, DiagnosticQuery, EntityQuery, EntityResolver, FactQuery, FactQueryStore,
    KnowledgeStore, RelationQuery, SnapshotBatch, SnapshotSelector,
};
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind, Fact,
    FactId, FactKind, Relation, RelationId, RelationKind, RelationStatus, RepoId, Severity,
    SnapshotBase, SnapshotId, StableKey,
};
use serde_json::json;

/// Verifies committed-snapshot isolation, entity/fact/relation/diagnostic filtering, and stable-key
/// resolution for a store implementation.
pub async fn verify_query_contract<S>(store: &S)
where
    S: KnowledgeStore + EntityResolver + FactQueryStore,
{
    let first = begin(store).await;
    let first_entity = entity("ent_first", "file://first.md");
    store
        .put_entities(first.clone(), vec![first_entity.clone()])
        .await
        .expect("store first entity");
    store
        .put_facts(
            first.clone(),
            vec![fact("fact_first", &first, "ent_first")],
        )
        .await
        .expect("store first fact");
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
        .put_facts(
            second.clone(),
            vec![
                fact("fact_second_a", &second, "ent_second"),
                fact("fact_second_b", &second, "ent_second"),
            ],
        )
        .await
        .expect("store second facts");
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

    let first_facts = store
        .query_facts(
            SnapshotSelector::Exact(first.clone()),
            FactQuery {
                subject: Some(EntityId("ent_first".to_string())),
                extractor: Some("store-conformance".to_string()),
                ..FactQuery::default()
            },
        )
        .await
        .expect("query first facts");
    assert_eq!(first_facts.len(), 1);
    assert_eq!(first_facts[0].id.0, "fact_first");

    let latest_facts = store
        .query_facts(
            SnapshotSelector::LatestCommitted,
            FactQuery {
                subject: Some(EntityId("ent_second".to_string())),
                extractor: Some("store-conformance".to_string()),
                limit: Some(1),
                ..FactQuery::default()
            },
        )
        .await
        .expect("query latest facts");
    assert_eq!(latest_facts.len(), 1);
    assert_eq!(latest_facts[0].subject.0, "ent_second");

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
    store
        .put_facts(
            uncommitted.clone(),
            vec![fact("fact_uncommitted", &uncommitted, "ent_uncommitted")],
        )
        .await
        .expect("store uncommitted fact");
    let error = store
        .query_facts(
            SnapshotSelector::Exact(uncommitted.clone()),
            FactQuery::default(),
        )
        .await
        .expect_err("uncommitted facts must not be queryable");
    assert!(matches!(
        error,
        CoreError::NotFound(_) | CoreError::SnapshotNotCommitted(_)
    ));
    let error = store
        .query_entities(
            SnapshotSelector::Exact(uncommitted.clone()),
            EntityQuery::default(),
        )
        .await
        .expect_err("uncommitted snapshots must not be queryable");
    assert!(matches!(error, CoreError::SnapshotNotCommitted(_)));

    store
        .abort_snapshot(uncommitted.clone())
        .await
        .expect("abort uncommitted snapshot");
    let error = store
        .query_entities(SnapshotSelector::Exact(uncommitted), EntityQuery::default())
        .await
        .expect_err("aborted snapshots must not be queryable");
    assert!(matches!(error, CoreError::NotFound(_)));
}

/// Verifies the shared snapshot lifecycle used by publication orchestration.
///
/// The contract deliberately covers behavior common to every backend today: a complete
/// `SnapshotBatch` stays invisible after prepare, becomes visible only after commit, cannot be
/// aborted once committed, and an aborted snapshot never changes `LatestCommitted`.
pub async fn verify_snapshot_lifecycle_contract<S>(store: &S)
where
    S: KnowledgeStore + EntityResolver + FactQueryStore,
{
    let prepared = begin(store).await;
    let prepared_entity = entity("ent_prepared", "file://prepared.md");
    store
        .put_snapshot(
            prepared.clone(),
            SnapshotBatch {
                entities: vec![prepared_entity.clone()],
                facts: vec![fact("fact_prepared", &prepared, "ent_prepared")],
                relations: vec![relation(
                    "rel_prepared",
                    &prepared,
                    "ent_prepared",
                    "ent_target",
                )],
                diagnostics: vec![diagnostic(
                    "diag_prepared",
                    &prepared,
                    "ent_prepared",
                )],
            },
        )
        .await
        .expect("store prepared snapshot batch");

    store
        .prepare_snapshot(prepared.clone())
        .await
        .expect("prepare snapshot");
    store
        .prepare_snapshot(prepared.clone())
        .await
        .expect("prepare must be idempotent before commit");

    let error = store
        .query_facts(
            SnapshotSelector::Exact(prepared.clone()),
            FactQuery::default(),
        )
        .await
        .expect_err("prepared facts must remain invisible");
    assert!(matches!(
        error,
        CoreError::NotFound(_) | CoreError::SnapshotNotCommitted(_)
    ));
    let error = store
        .query_entities(
            SnapshotSelector::Exact(prepared.clone()),
            EntityQuery::default(),
        )
        .await
        .expect_err("prepared snapshot must remain invisible");
    assert!(matches!(error, CoreError::SnapshotNotCommitted(_)));

    store
        .commit_snapshot(prepared.clone())
        .await
        .expect("commit prepared snapshot");

    let facts = store
        .query_facts(
            SnapshotSelector::Exact(prepared.clone()),
            FactQuery {
                subject: Some(prepared_entity.id.clone()),
                extractor: Some("store-conformance".to_string()),
                ..FactQuery::default()
            },
        )
        .await
        .expect("query committed facts");
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].id.0, "fact_prepared");

    let entities = store
        .query_entities(
            SnapshotSelector::Exact(prepared.clone()),
            EntityQuery {
                stable_key: Some(prepared_entity.stable_key.clone()),
                ..EntityQuery::default()
            },
        )
        .await
        .expect("query committed prepared snapshot");
    assert_eq!(entities, vec![prepared_entity.clone()]);

    let relations = store
        .query_relations(
            SnapshotSelector::Exact(prepared.clone()),
            RelationQuery {
                from_entity: Some(prepared_entity.id.clone()),
                ..RelationQuery::default()
            },
        )
        .await
        .expect("query committed relations");
    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].id.0, "rel_prepared");

    let diagnostics = store
        .query_diagnostics(
            SnapshotSelector::Exact(prepared.clone()),
            DiagnosticQuery {
                entity: Some(prepared_entity.id.clone()),
                ..DiagnosticQuery::default()
            },
        )
        .await
        .expect("query committed diagnostics");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id.0, "diag_prepared");

    let abort_error = store
        .abort_snapshot(prepared.clone())
        .await
        .expect_err("committed snapshot must not be abortable");
    assert!(matches!(abort_error, CoreError::Conflict(_)));

    let aborted = begin(store).await;
    store
        .put_snapshot(
            aborted.clone(),
            SnapshotBatch {
                entities: vec![entity("ent_aborted", "file://aborted.md")],
                ..SnapshotBatch::default()
            },
        )
        .await
        .expect("store snapshot that will be aborted");
    store
        .abort_snapshot(aborted.clone())
        .await
        .expect("abort snapshot");

    let error = store
        .query_entities(SnapshotSelector::Exact(aborted), EntityQuery::default())
        .await
        .expect_err("aborted snapshot must not be queryable");
    assert!(matches!(error, CoreError::NotFound(_)));

    let latest = store
        .query_entities(
            SnapshotSelector::LatestCommitted,
            EntityQuery {
                stable_key: Some(prepared_entity.stable_key.clone()),
                ..EntityQuery::default()
            },
        )
        .await
        .expect("query latest committed after abort");
    assert_eq!(latest, vec![prepared_entity]);
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

fn fact(id: &str, snapshot: &SnapshotId, subject: &str) -> Fact {
    Fact {
        id: FactId(id.to_string()),
        kind: FactKind::Other("conformance".to_string()),
        subject: EntityId(subject.to_string()),
        object: None,
        value: json!({"verified": true}),
        evidence: Vec::new(),
        ownership: Vec::new(),
        snapshot: snapshot.clone(),
        extractor: "store-conformance".to_string(),
        confidence: 1.0,
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
