use athanor_app::{ContextLimits, generate_context_pack};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    ContextLevel, Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId,
    EntityKind, Relation, RelationId, RelationKind, RelationStatus, Severity, SnapshotId,
    SourceLocation, StableKey,
};
use serde_json::json;

#[test]
fn selects_direct_matches_relational_neighbors_and_diagnostics() {
    let file = entity("ent_file", "file://docs/auth.md", "auth.md", "docs/auth.md");
    let section = entity(
        "ent_login",
        "test://tests/login.rs",
        "Login test",
        "tests/login.rs",
    );
    let unrelated = entity("ent_other", "file://src/lib.rs", "lib.rs", "src/lib.rs");
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_test".to_string())),
        entities: vec![file.clone(), section.clone(), unrelated],
        facts: Vec::new(),
        relations: vec![Relation {
            id: RelationId("rel_contains".to_string()),
            kind: RelationKind::Contains,
            from: file.id.clone(),
            to: section.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            payload: json!({}),
        }],
        diagnostics: vec![Diagnostic {
            id: DiagnosticId("diag_login".to_string()),
            kind: DiagnosticKind::MissingDocumentation,
            severity: Severity::Medium,
            status: DiagnosticStatus::Open,
            title: "Login documentation".to_string(),
            message: "Test diagnostic".to_string(),
            entities: vec![section.id.clone()],
            evidence: Vec::new(),
            ownership: Vec::new(),
            snapshot: SnapshotId("snap_test".to_string()),
            suggested_fix: None,
            payload: json!({}),
        }],
    };

    let pack = generate_context_pack(
        &snapshot,
        "change auth",
        ContextLevel::Normal,
        ContextLimits::for_level(ContextLevel::Normal),
        None,
    );

    assert_eq!(pack.entities, vec![file.id, section.id]);
    assert_eq!(pack.files, vec!["docs/auth.md", "tests/login.rs"]);
    assert_eq!(pack.diagnostics, vec![DiagnosticId("diag_login".to_string())]);
    assert_eq!(pack.payload["snapshot"], "snap_test");
    assert!(pack.confidence > 0.0);
}

#[test]
fn returns_empty_pack_when_nothing_matches() {
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_test".to_string())),
        entities: vec![entity(
            "ent_file",
            "file://README.md",
            "README.md",
            "README.md",
        )],
        ..CanonicalSnapshot::default()
    };

    let pack = generate_context_pack(
        &snapshot,
        "authentication",
        ContextLevel::Normal,
        ContextLimits::for_level(ContextLevel::Normal),
        None,
    );

    assert!(pack.entities.is_empty());
    assert_eq!(pack.confidence, 0.0);
    assert!(pack.summary.starts_with("No canonical entities matched"));
}

#[test]
fn explicit_limits_bound_files_entities_and_diagnostics() {
    let first = entity("ent_first", "file://docs/first.md", "auth", "docs/first.md");
    let second = entity(
        "ent_second",
        "file://docs/second.md",
        "auth",
        "docs/second.md",
    );
    let diagnostic = |id: &str, entity: EntityId| Diagnostic {
        id: DiagnosticId(id.to_string()),
        kind: DiagnosticKind::MissingDocumentation,
        severity: Severity::Medium,
        status: DiagnosticStatus::Open,
        title: id.to_string(),
        message: id.to_string(),
        entities: vec![entity],
        evidence: Vec::new(),
        ownership: Vec::new(),
        snapshot: SnapshotId("snap_test".to_string()),
        suggested_fix: None,
        payload: json!({}),
    };
    let snapshot = CanonicalSnapshot {
        entities: vec![first.clone(), second],
        diagnostics: vec![
            diagnostic("diag_first", first.id.clone()),
            diagnostic("diag_second", first.id.clone()),
        ],
        ..CanonicalSnapshot::default()
    };
    let limits = ContextLimits {
        max_tokens: 2_000,
        max_files: 1,
        max_entities: 1,
        max_diagnostics: 1,
        max_depth: 0,
    };

    let pack = generate_context_pack(&snapshot, "auth", ContextLevel::Normal, limits, None);

    assert_eq!(pack.entities.len(), 1);
    assert_eq!(pack.files.len(), 1);
    assert_eq!(pack.diagnostics.len(), 1);
    assert!(pack.payload["estimated_tokens"].as_u64().unwrap() <= 2_000);
    assert_eq!(pack.payload["omitted"]["reason"], "relevance_or_context_limits");
}

fn entity(id: &str, stable_key: &str, name: &str, path: &str) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind: EntityKind::File,
        name: name.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: path.to_string(),
            line_start: None,
            line_end: None,
        }),
        language: None,
        aliases: Vec::new(),
        ownership: Vec::new(),
        payload: json!({}),
    }
}
