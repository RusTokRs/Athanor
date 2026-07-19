use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Relation, RelationId, RelationKind, RelationStatus, Severity, SnapshotId, SourceLocation,
    StableKey,
};
use serde_json::json;

use super::aggregation::build_repository_overview;

#[test]
fn builds_bounded_overview_with_hubs_and_counts() {
    let endpoint = entity(
        "ent_endpoint",
        EntityKind::ApiEndpoint,
        "api://GET:/health",
        "health",
        "openapi.yaml",
    );
    let handler = entity(
        "ent_handler",
        EntityKind::Function,
        "rust://src/lib.rs#health",
        "health",
        "src/lib.rs",
    );
    let doc = entity(
        "ent_doc",
        EntityKind::DocumentationPage,
        "doc://docs/api/health.md",
        "Health API",
        "docs/api/health.md",
    );
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_test".to_string())),
        entities: vec![endpoint.clone(), handler.clone(), doc.clone()],
        relations: vec![
            relation("rel_impl", RelationKind::ImplementedBy, &endpoint, &handler),
            relation("rel_docs", RelationKind::Documents, &doc, &endpoint),
        ],
        diagnostics: vec![diagnostic("diag_docs", "docs/api/health.md")],
        ..CanonicalSnapshot::default()
    };

    let overview = build_repository_overview(&snapshot, 5);

    assert_eq!(overview.snapshot, "snap_test");
    assert_eq!(overview.totals.entities, 3);
    assert_eq!(overview.totals.open_diagnostics, 1);
    assert_eq!(overview.api.endpoints, 1);
    assert_eq!(overview.api.documented_endpoints, 1);
    assert_eq!(overview.api.implemented_endpoints, 1);
    assert_eq!(overview.graph_hubs[0].stable_key, "api://GET:/health");
    assert_eq!(overview.graph_hubs[0].degree, 2);
    assert_eq!(
        overview.open_diagnostics[0].source.as_deref(),
        Some("docs/api/health.md:1")
    );
    assert_eq!(overview.integration_boundaries.len(), 2);
    assert_eq!(overview.integration_boundaries[0].relations, 1);
    assert_eq!(overview.integration_boundaries[0].relation_ids.len(), 1);
}

#[test]
fn summarizes_modules_and_bounds_boundary_relation_ids() {
    let module = entity(
        "ent_module",
        EntityKind::Module,
        "rust://crates/example/src/lib.rs",
        "example",
        "crates/example/src/lib.rs",
    );
    let first = entity(
        "ent_first",
        EntityKind::Function,
        "rust://crates/example/src/lib.rs#first",
        "first",
        "crates/example/src/lib.rs",
    );
    let second = entity(
        "ent_second",
        EntityKind::Function,
        "rust://crates/example/src/lib.rs#second",
        "second",
        "crates/example/src/lib.rs",
    );
    let doc = entity(
        "ent_doc",
        EntityKind::DocumentationPage,
        "doc://docs/example.md",
        "Example",
        "docs/example.md",
    );
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_test".to_string())),
        entities: vec![module.clone(), first.clone(), second.clone(), doc.clone()],
        relations: vec![
            relation("rel_define_first", RelationKind::Defines, &module, &first),
            relation("rel_define_second", RelationKind::Defines, &module, &second),
            relation("rel_doc_first", RelationKind::Documents, &doc, &first),
            relation("rel_doc_second", RelationKind::Documents, &doc, &second),
        ],
        ..CanonicalSnapshot::default()
    };

    let overview = build_repository_overview(&snapshot, 1);

    assert_eq!(overview.module_structure.len(), 1);
    assert_eq!(overview.module_structure[0].direct_members, 2);
    assert_eq!(
        overview.module_structure[0].relation_ids,
        vec!["rel_define_first"]
    );
    assert_eq!(overview.module_structure[0].omitted_relation_ids, 1);
    assert_eq!(overview.integration_boundaries.len(), 1);
    assert_eq!(
        overview.integration_boundaries[0].from_root,
        "docs/example.md"
    );
    assert_eq!(overview.integration_boundaries[0].to_root, "crates/example");
    assert_eq!(overview.integration_boundaries[0].relations, 2);
    assert_eq!(
        overview.integration_boundaries[0].relation_ids,
        vec!["rel_doc_first"]
    );
    assert_eq!(overview.integration_boundaries[0].omitted_relation_ids, 1);
}

fn entity(id: &str, kind: EntityKind, stable_key: &str, name: &str, path: &str) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind,
        name: name.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: path.to_string(),
            line_start: Some(1),
            line_end: Some(1),
        }),
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
        evidence: Vec::new(),
        ownership: Vec::new(),
        snapshot: SnapshotId("snap_test".to_string()),
        payload: json!({}),
    }
}

fn diagnostic(id: &str, path: &str) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId(id.to_string()),
        kind: DiagnosticKind::MissingDocumentation,
        severity: Severity::High,
        status: DiagnosticStatus::Open,
        title: "Missing docs".to_string(),
        message: "Missing docs".to_string(),
        entities: Vec::new(),
        evidence: vec![athanor_domain::Evidence {
            source_file: Some(path.to_string()),
            line_start: Some(1),
            line_end: Some(1),
            extractor: Some("test".to_string()),
            commit_hash: None,
            confidence: 1.0,
            status: athanor_domain::EvidenceStatus::Missing,
        }],
        ownership: Vec::new(),
        snapshot: SnapshotId("snap_test".to_string()),
        suggested_fix: None,
        payload: json!({}),
    }
}
