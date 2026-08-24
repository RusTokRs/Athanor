use athanor_app::{
    DOCUMENTATION_COMPLETENESS_BASELINE_ADAPTER, DOCUMENTATION_COMPLETENESS_SCHEMA_V1,
    DocumentationCompletenessRequest, build_documentation_completeness_report,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, EvidenceStatus, Fact, FactId, FactKind, LanguageCode, Ownership, Relation, RelationId,
    RelationKind, RelationStatus, Severity, SnapshotId, SourceLocation, StableKey,
};
use serde_json::json;

#[test]
fn completeness_report_is_exact_deterministic_and_exposes_adapter_gaps() {
    let request = DocumentationCompletenessRequest::new("snap-completeness", 16);
    let snapshot = fixture();
    let report = build_documentation_completeness_report(&request, &snapshot).unwrap();

    assert_eq!(report.schema, DOCUMENTATION_COMPLETENESS_SCHEMA_V1);
    assert_eq!(report.snapshot, "snap-completeness");
    assert_eq!(
        report.baseline_adapter,
        DOCUMENTATION_COMPLETENESS_BASELINE_ADAPTER
    );
    assert_eq!(report.totals.tracked_files, 4);
    assert_eq!(report.totals.processed_files, 3);
    assert_eq!(report.totals.unprocessed_files, 1);
    assert_eq!(report.totals.processed_ratio_basis_points, 7_500);
    assert_eq!(report.totals.languages, 4);
    assert_eq!(report.totals.adapters, 4);
    assert_eq!(report.totals.facts, 6);
    assert_eq!(report.totals.relations, 1);
    assert_eq!(report.totals.diagnostics, 1);

    assert_eq!(report.unprocessed_files.len(), 1);
    assert_eq!(report.unprocessed_files[0].path, "templates/page.twig");
    assert_eq!(report.unprocessed_files[0].language, "twig");

    let markdown = report
        .languages
        .iter()
        .find(|row| row.language == "markdown")
        .unwrap();
    assert_eq!(markdown.tracked_files, 1);
    assert_eq!(markdown.processed_files, 1);
    assert_eq!(markdown.processed_ratio_basis_points, 10_000);
    assert_eq!(markdown.adapters, 3);

    let rust = report
        .languages
        .iter()
        .find(|row| row.language == "rust")
        .unwrap();
    assert_eq!(rust.processed_files, 1);
    assert_eq!(rust.adapters, 0, "entity-only processing stays unattributed");

    let markdown_extractor = report
        .adapters
        .iter()
        .find(|row| row.adapter == "markdown")
        .unwrap();
    assert_eq!(markdown_extractor.files, 1);
    assert_eq!(markdown_extractor.facts, 1);
    assert_eq!(markdown_extractor.relations, 0);
    assert_eq!(markdown_extractor.diagnostics, 0);

    let linker = report
        .adapters
        .iter()
        .find(|row| row.adapter == "markdown-linker")
        .unwrap();
    assert_eq!(linker.relations, 1);

    let checker = report
        .adapters
        .iter()
        .find(|row| row.adapter == "checker-markdown")
        .unwrap();
    assert_eq!(checker.diagnostics, 1);

    assert!(
        report
            .adapters
            .iter()
            .all(|row| row.adapter != DOCUMENTATION_COMPLETENESS_BASELINE_ADAPTER)
    );
    assert_eq!(report.omitted.languages, 0);
    assert_eq!(report.omitted.adapters, 0);
    assert_eq!(report.omitted.unprocessed_files, 0);

    let mut reversed = snapshot.clone();
    reversed.entities.reverse();
    reversed.facts.reverse();
    reversed.relations.reverse();
    reversed.diagnostics.reverse();
    assert_eq!(
        build_documentation_completeness_report(&request, &reversed).unwrap(),
        report
    );
}

#[test]
fn completeness_report_normalizes_file_paths_and_bounds_each_detail_table() {
    let request = DocumentationCompletenessRequest::new("snap-completeness", 1);
    let report = build_documentation_completeness_report(&request, &fixture()).unwrap();

    assert_eq!(report.languages.len(), 1);
    assert_eq!(report.adapters.len(), 1);
    assert_eq!(report.unprocessed_files.len(), 1);
    assert_eq!(report.omitted.languages, 3);
    assert_eq!(report.omitted.adapters, 3);
    assert_eq!(report.omitted.unprocessed_files, 0);
    assert!(
        report
            .languages
            .iter()
            .all(|row| !row.language.contains('\\'))
    );
}

#[test]
fn completeness_report_fails_closed_on_identity_baseline_and_request_errors() {
    let snapshot = fixture();

    let mismatch = DocumentationCompletenessRequest::new("snap-other", 8);
    assert!(
        build_documentation_completeness_report(&mismatch, &snapshot)
            .unwrap_err()
            .contains("snapshot mismatch")
    );

    let zero_limit = DocumentationCompletenessRequest::new("snap-completeness", 0);
    assert!(
        build_documentation_completeness_report(&zero_limit, &snapshot)
            .unwrap_err()
            .contains("greater than zero")
    );

    let no_baseline = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-completeness".to_string())),
        entities: vec![content_entity(
            "function-only",
            "symbol://only",
            EntityKind::Function,
            "src/only.rs",
            Some("rust"),
        )],
        ..CanonicalSnapshot::default()
    };
    assert!(
        build_documentation_completeness_report(
            &DocumentationCompletenessRequest::new("snap-completeness", 8),
            &no_baseline,
        )
        .unwrap_err()
        .contains("baseline `file` entities")
    );
}

fn fixture() -> CanonicalSnapshot {
    let snapshot = SnapshotId("snap-completeness".to_string());
    let readme_file = file_entity("file-readme", "README.md", "README.md", "markdown");
    let rust_file = file_entity("file-rust", "src/lib.rs", "src/lib.rs", "rust");
    let api_file = file_entity(
        "file-api",
        "api\\openapi.yaml",
        "api/openapi.yaml",
        "yaml",
    );
    let template_file = file_entity(
        "file-template",
        "templates/page.twig",
        "templates/page.twig",
        "twig",
    );

    let doc = content_entity(
        "doc-readme",
        "doc://README.md",
        EntityKind::DocumentationPage,
        "README.md",
        Some("markdown"),
    );
    let function = content_entity(
        "function-main",
        "symbol://crate/main",
        EntityKind::Function,
        "src/lib.rs",
        Some("rust"),
    );
    let endpoint = content_entity(
        "endpoint-health",
        "api://GET:/health",
        EntityKind::ApiEndpoint,
        "api/openapi.yaml",
        Some("yaml"),
    );

    CanonicalSnapshot {
        snapshot: Some(snapshot.clone()),
        entities: vec![
            readme_file,
            rust_file,
            api_file,
            template_file,
            doc.clone(),
            function,
            endpoint.clone(),
        ],
        facts: vec![
            baseline_fact("baseline-readme", "file-readme", "README.md", &snapshot),
            baseline_fact("baseline-rust", "file-rust", "src/lib.rs", &snapshot),
            baseline_fact(
                "baseline-api",
                "file-api",
                "api/openapi.yaml",
                &snapshot,
            ),
            baseline_fact(
                "baseline-template",
                "file-template",
                "templates/page.twig",
                &snapshot,
            ),
            content_fact(
                "markdown-heading",
                FactKind::DocSectionFound,
                &doc.id.0,
                "README.md",
                "markdown",
                &snapshot,
            ),
            content_fact(
                "openapi-route",
                FactKind::RouteDeclared,
                &endpoint.id.0,
                "api/openapi.yaml",
                "openapi",
                &snapshot,
            ),
        ],
        relations: vec![Relation {
            id: RelationId("rel-documents".to_string()),
            kind: RelationKind::Documents,
            from: doc.id.clone(),
            to: endpoint.id.clone(),
            status: RelationStatus::Verified,
            confidence: 1.0,
            evidence: vec![evidence("README.md", Some("markdown-linker"))],
            ownership: vec![Ownership {
                source_file: "README.md".to_string(),
            }],
            snapshot: snapshot.clone(),
            payload: json!({}),
        }],
        diagnostics: vec![Diagnostic {
            id: DiagnosticId("diag-readme".to_string()),
            kind: DiagnosticKind::DocumentationReferenceUnresolved,
            severity: Severity::Medium,
            status: DiagnosticStatus::Open,
            title: "Unresolved reference".to_string(),
            message: "README references an unresolved target".to_string(),
            entities: vec![doc.id],
            evidence: vec![evidence("README.md", Some("checker-markdown"))],
            ownership: vec![Ownership {
                source_file: "README.md".to_string(),
            }],
            snapshot,
            suggested_fix: None,
            payload: json!({}),
        }],
        ..CanonicalSnapshot::default()
    }
}

fn file_entity(
    id: &str,
    source_path: &str,
    ownership_path: &str,
    language: &str,
) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(format!("file://{ownership_path}")),
        kind: EntityKind::File,
        name: ownership_path.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: source_path.to_string(),
            line_start: None,
            line_end: None,
        }),
        language: Some(LanguageCode(language.to_string())),
        aliases: Vec::new(),
        ownership: vec![Ownership {
            source_file: ownership_path.to_string(),
        }],
        payload: json!({}),
    }
}

fn content_entity(
    id: &str,
    stable_key: &str,
    kind: EntityKind,
    path: &str,
    language: Option<&str>,
) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind,
        name: id.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: path.to_string(),
            line_start: Some(1),
            line_end: Some(1),
        }),
        language: language.map(|language| LanguageCode(language.to_string())),
        aliases: Vec::new(),
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        payload: json!({}),
    }
}

fn baseline_fact(id: &str, subject: &str, path: &str, snapshot: &SnapshotId) -> Fact {
    Fact {
        id: FactId(id.to_string()),
        kind: FactKind::FileDiscovered,
        subject: EntityId(subject.to_string()),
        object: None,
        value: json!({"path": path}),
        evidence: vec![evidence(path, Some("file"))],
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        snapshot: snapshot.clone(),
        extractor: "file".to_string(),
        confidence: 1.0,
    }
}

fn content_fact(
    id: &str,
    kind: FactKind,
    subject: &str,
    path: &str,
    adapter: &str,
    snapshot: &SnapshotId,
) -> Fact {
    Fact {
        id: FactId(id.to_string()),
        kind,
        subject: EntityId(subject.to_string()),
        object: None,
        value: json!({}),
        evidence: vec![evidence(path, Some(adapter))],
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        snapshot: snapshot.clone(),
        extractor: adapter.to_string(),
        confidence: 1.0,
    }
}

fn evidence(path: &str, adapter: Option<&str>) -> Evidence {
    Evidence {
        source_file: Some(path.to_string()),
        line_start: Some(1),
        line_end: Some(1),
        extractor: adapter.map(str::to_string),
        commit_hash: None,
        confidence: 1.0,
        status: EvidenceStatus::Verified,
    }
}
