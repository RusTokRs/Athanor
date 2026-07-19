use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Entity, EntityId, EntityKind, Evidence, EvidenceStatus, Relation, RelationId, RelationKind,
    RelationStatus, SnapshotId, SourceLocation, StableKey,
};
use serde_json::json;

use super::*;

#[test]
fn exports_bounded_graph_by_degree_then_stable_key() {
    let endpoint = entity(
        "ent_endpoint",
        "api://GET:/health",
        EntityKind::ApiEndpoint,
        "health",
    );
    let handler = entity(
        "ent_handler",
        "rust://src/lib.rs#health",
        EntityKind::Function,
        "health",
    );
    let doc = entity(
        "ent_doc",
        "doc://docs/api/health.md",
        EntityKind::DocumentationPage,
        "Health API",
    );
    let orphan = entity("ent_orphan", "file://orphan", EntityKind::File, "orphan");
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_test".to_string())),
        entities: vec![handler.clone(), orphan, doc.clone(), endpoint.clone()],
        relations: vec![
            relation("rel_docs", RelationKind::Documents, &doc, &endpoint),
            relation(
                "rel_impl",
                RelationKind::ImplementedBy,
                &endpoint,
                &handler,
            ),
        ],
        ..CanonicalSnapshot::default()
    };

    let export = build_graph_export(&snapshot, 3, 1);

    assert_eq!(export.schema, GRAPH_EXPORT_SCHEMA);
    assert_eq!(export.snapshot, "snap_test");
    assert_eq!(
        export
            .nodes
            .iter()
            .map(|node| node.stable_key.as_str())
            .collect::<Vec<_>>(),
        vec![
            "api://GET:/health",
            "doc://docs/api/health.md",
            "rust://src/lib.rs#health"
        ]
    );
    assert_eq!(export.edges.len(), 1);
    assert_eq!(export.edges[0].id, "rel_docs");
    assert_eq!(export.edges[0].evidence, vec!["docs/api/health.md:1"]);
    assert_eq!(export.omitted.nodes, 1);
    assert_eq!(export.omitted.edges, 1);
}

#[test]
fn renders_graph_export_as_graphml_with_escaped_values() {
    let endpoint = entity(
        "ent_endpoint",
        "api://GET:/health?format=<json>",
        EntityKind::ApiEndpoint,
        "health & status",
    );
    let handler = entity(
        "ent_handler",
        "rust://src/lib.rs#health",
        EntityKind::Function,
        "health",
    );
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_test".to_string())),
        entities: vec![handler.clone(), endpoint.clone()],
        relations: vec![relation(
            "rel_impl",
            RelationKind::ImplementedBy,
            &endpoint,
            &handler,
        )],
        ..CanonicalSnapshot::default()
    };

    let graphml = graph_export_to_graphml(&build_graph_export(&snapshot, 10, 10));

    assert!(graphml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(graphml.contains("<graph id=\"snap_test\" edgedefault=\"directed\">"));
    assert!(
        graphml.contains("<data key=\"stable_key\">api://GET:/health?format=&lt;json&gt;</data>")
    );
    assert!(graphml.contains("<data key=\"name\">health &amp; status</data>"));
    assert!(graphml.ends_with("</graphml>\n"));
}

#[test]
fn pure_related_graph_uses_canonical_cooperative_traversal() {
    let endpoint = entity(
        "ent_endpoint",
        "api://GET:/health",
        EntityKind::ApiEndpoint,
        "health",
    );
    let handler = entity(
        "ent_handler",
        "rust://src/lib.rs#health",
        EntityKind::Function,
        "health",
    );
    let doc = entity(
        "ent_doc",
        "doc://docs/api/health.md",
        EntityKind::DocumentationPage,
        "Health API",
    );
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_test".to_string())),
        entities: vec![handler.clone(), doc.clone(), endpoint.clone()],
        relations: vec![
            relation("rel_docs", RelationKind::Documents, &doc, &endpoint),
            relation(
                "rel_impl",
                RelationKind::ImplementedBy,
                &endpoint,
                &handler,
            ),
        ],
        ..CanonicalSnapshot::default()
    };

    let related = build_related_graph(&snapshot, "api://GET:/health", 1, 3, 10).unwrap();

    assert_eq!(related.schema, GRAPH_RELATED_SCHEMA);
    assert_eq!(related.root.entity.stable_key, "api://GET:/health");
    assert_eq!(
        related
            .nodes
            .iter()
            .map(|node| (node.distance, node.entity.stable_key.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (0, "api://GET:/health"),
            (1, "doc://docs/api/health.md"),
            (1, "rust://src/lib.rs#health"),
        ]
    );
    assert!(!related.truncated);
}

fn entity(id: &str, stable_key: &str, kind: EntityKind, name: &str) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind,
        name: name.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: stable_key
                .strip_prefix("doc://")
                .unwrap_or("src/lib.rs")
                .to_string(),
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
        evidence: vec![Evidence {
            source_file: Some("docs/api/health.md".to_string()),
            line_start: Some(1),
            line_end: Some(1),
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
