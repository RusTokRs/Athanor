use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, EvidenceStatus, Relation, RelationId, RelationKind, RelationStatus, Severity,
    SnapshotId, StableKey,
};
use serde_json::json;

use super::traversal::impact_snapshot;

fn entity(id: &str, stable_key: &str, kind: EntityKind) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind,
        name: stable_key.to_string(),
        title: None,
        source: None,
        language: None,
        aliases: Vec::new(),
        ownership: Vec::new(),
        payload: json!({}),
    }
}

fn relation(id: &str, kind: RelationKind, from: &Entity, to: &Entity) -> Relation {
    Relation {
        id: RelationId(id.to_string()),
        kind,
        from: from.id.clone(),
        to: to.id.clone(),
        status: RelationStatus::Verified,
        confidence: 1.0,
        evidence: vec![Evidence {
            source_file: None,
            line_start: None,
            line_end: None,
            extractor: Some("test".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: EvidenceStatus::Verified,
        }],
        ownership: Vec::new(),
        snapshot: SnapshotId("snap_test".to_string()),
        payload: json!({}),
    }
}

#[test]
fn traverses_impact_correctly() {
    let callee = entity("callee_id", "symbol://rust:callee", EntityKind::Function);
    let caller = entity("caller_id", "symbol://rust:caller", EntityKind::Function);
    let endpoint = entity("endpoint_id", "api://POST:/login", EntityKind::ApiEndpoint);
    let doc = entity(
        "doc_id",
        "doc://docs/login.md",
        EntityKind::DocumentationPage,
    );
    let test = entity("test_id", "symbol://rust:test_login", EntityKind::TestCase);

    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_test".to_string())),
        entities: vec![
            callee.clone(),
            caller.clone(),
            endpoint.clone(),
            doc.clone(),
            test.clone(),
        ],
        facts: Vec::new(),
        relations: vec![
            relation("rel_call", RelationKind::Calls, &caller, &callee),
            relation("rel_impl", RelationKind::ImplementedBy, &endpoint, &caller),
            relation("rel_doc", RelationKind::Documents, &doc, &endpoint),
            relation("rel_test", RelationKind::TestedBy, &caller, &test),
        ],
        diagnostics: vec![Diagnostic {
            id: DiagnosticId("diag_1".to_string()),
            kind: DiagnosticKind::UncoveredSymbol,
            severity: Severity::Low,
            status: DiagnosticStatus::Open,
            title: "Uncovered symbol".to_string(),
            message: "Callee not covered by tests".to_string(),
            entities: vec![callee.id.clone()],
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            suggested_fix: None,
            payload: json!({}),
        }],
    };

    let analysis = impact_snapshot(&snapshot, vec![callee.clone()], 10);

    assert_eq!(analysis.starting_entities.len(), 1);
    assert_eq!(analysis.starting_entities[0].id, callee.id);

    let impacted_keys = analysis
        .impacted_entities
        .iter()
        .map(|impact| impact.entity.stable_key.0.clone())
        .collect::<Vec<_>>();

    assert!(impacted_keys.contains(&caller.stable_key.0));
    assert!(impacted_keys.contains(&endpoint.stable_key.0));
    assert!(impacted_keys.contains(&doc.stable_key.0));
    assert!(impacted_keys.contains(&test.stable_key.0));

    assert_eq!(analysis.impacted_diagnostics.len(), 1);
    assert_eq!(analysis.impacted_diagnostics[0].id.0, "diag_1");

    let doc_impact = analysis
        .impacted_entities
        .iter()
        .find(|impact| impact.entity.id == doc.id)
        .expect("documentation page should be impacted through endpoint");
    assert_eq!(doc_impact.path_steps.len(), 3);
    assert_eq!(doc_impact.path_steps[0].relation_id, "rel_call");
    assert_eq!(doc_impact.path_steps[0].relation_kind, "calls");
    assert_eq!(
        doc_impact.path_steps[0].from.stable_key,
        callee.stable_key.0
    );
    assert_eq!(
        doc_impact.path_steps[0].to.stable_key,
        caller.stable_key.0
    );
    assert_eq!(doc_impact.path_steps[2].relation_id, "rel_doc");
    assert_eq!(
        doc_impact.path_steps[2].from.stable_key,
        endpoint.stable_key.0
    );
    assert_eq!(doc_impact.path_steps[2].to.stable_key, doc.stable_key.0);
}
