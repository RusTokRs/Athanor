use athanor_app::{
    API_DOCUMENT_MEDIA_TYPE, API_DOCUMENT_PATH, DOCUMENTATION_REFERENCE_LIMIT,
    DocumentationContextItemKind, DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationProfile, DocumentationRelationDirection, DocumentationValidationStatus,
    build_documentation_api_profile,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, EvidenceStatus, Fact, FactId, FactKind, Ownership, Relation, RelationId, RelationKind,
    RelationStatus, Severity, SnapshotId, SourceLocation, StableKey,
};
use serde_json::json;
use sha2::{Digest, Sha256};

#[test]
fn api_profile_is_exact_scoped_cited_and_checksum_bound() {
    let snapshot = api_snapshot();
    let profile = build_documentation_api_profile(&request(8), &snapshot).unwrap();

    assert_eq!(profile.outline.profile, DocumentationProfile::Api);
    assert_eq!(
        profile
            .outline
            .sections
            .iter()
            .map(|section| section.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "overview",
            "endpoints",
            "schemas",
            "examples",
            "facts",
            "relationships",
            "diagnostics",
        ]
    );
    assert_eq!(profile.context.profile, DocumentationProfile::Api);
    assert_eq!(count_kind(&profile, DocumentationContextItemKind::Entity), 5);
    assert_eq!(count_kind(&profile, DocumentationContextItemKind::Fact), 2);
    assert_eq!(count_kind(&profile, DocumentationContextItemKind::Relation), 7);
    assert_eq!(count_kind(&profile, DocumentationContextItemKind::Diagnostic), 2);
    assert_eq!(profile.context.omitted.entities, 0);
    assert_eq!(profile.context.omitted.facts, 0);
    assert_eq!(profile.context.omitted.relations, 0);
    assert_eq!(profile.context.omitted.diagnostics, 0);
    assert_eq!(profile.draft.citations.len(), 16);
    assert_eq!(profile.validation_report.status, DocumentationValidationStatus::Valid);
    assert_eq!(profile.validation_report.profile, DocumentationProfile::Api);
    assert_eq!(profile.validation_report.metrics.unsupported_relations, 0);
    assert_eq!(profile.document.path, API_DOCUMENT_PATH);
    assert_eq!(profile.document.media_type, API_DOCUMENT_MEDIA_TYPE);
    assert!(profile.document.content.contains("- Profile: `api`"));
    assert!(profile.document.content.contains("## API Endpoints"));
    assert!(profile.document.content.contains("## API Schemas"));
    assert!(profile.document.content.contains("## API Examples"));
    assert!(profile.document.content.contains("## API Facts"));
    assert!(profile.document.content.contains("## API Relationships"));
    assert!(profile.document.content.contains("## API Diagnostics"));
    assert!(profile.document.content.contains("```mermaid"));
    assert!(profile.document.content.contains("Slice 3B scope"));
    assert!(!profile.document.content.contains("graphql_uses_fragment"));
    assert_eq!(
        profile.document.sha256,
        sha256_hex(profile.document.content.as_bytes())
    );
    assert_eq!(
        serde_json::to_value(DocumentationProfile::Api).unwrap(),
        json!("api")
    );
    assert!(profile.context.items.len() <= DOCUMENTATION_REFERENCE_LIMIT);

    let endpoint = profile
        .context
        .items
        .iter()
        .find(|item| item.stable_keys == ["api://GET:/health".to_string()])
        .unwrap();
    assert!(endpoint.id.starts_with("api-endpoint-"));
    assert!(endpoint.summary.contains("GET /health"));
    assert_eq!(endpoint.evidence[0].path, "api/openapi.yaml");
}

#[test]
fn api_profile_keeps_only_supported_relations_and_open_scoped_diagnostics() {
    let profile = build_documentation_api_profile(&request(8), &api_snapshot()).unwrap();
    let relations = profile
        .context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Relation)
        .collect::<Vec<_>>();

    assert_eq!(relations.len(), 7);
    assert!(relations.iter().all(|item| {
        item.source_stable_key.is_some()
            && item.target_stable_key.is_some()
            && item.relation_direction == Some(DocumentationRelationDirection::Directed)
    }));
    for expected in [
        "implemented_by",
        "schema_for_request",
        "schema_for_response",
        "example_for",
        "documents",
        "documents_api",
        "documents_operation",
    ] {
        assert!(relations.iter().any(|item| item.summary.contains(expected)));
    }
    assert!(
        !relations
            .iter()
            .any(|item| item.summary.contains("graphql_uses_fragment"))
    );

    let diagnostics = profile
        .context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Diagnostic)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .any(|item| item.summary.contains("api_example_invalid"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|item| item.summary.contains("api_openapi_graphql_drift"))
    );
    assert!(
        diagnostics
            .iter()
            .all(|item| !item.summary.contains("resolved response drift"))
    );
}

#[test]
fn api_profile_orders_scoped_evidence_discloses_omissions_and_ignores_input_order() {
    let limits = DocumentationGenerationLimits {
        max_entities: 2,
        max_facts: 1,
        max_relations: 2,
        max_diagnostics: 1,
    };
    let request = DocumentationGenerationRequest::new(
        "snap-api-0001",
        DocumentationProfile::Api,
        limits,
    );
    let snapshot = api_snapshot();
    let profile = build_documentation_api_profile(&request, &snapshot).unwrap();

    assert_eq!(count_kind(&profile, DocumentationContextItemKind::Entity), 2);
    assert_eq!(count_kind(&profile, DocumentationContextItemKind::Fact), 1);
    assert_eq!(count_kind(&profile, DocumentationContextItemKind::Relation), 2);
    assert_eq!(count_kind(&profile, DocumentationContextItemKind::Diagnostic), 1);
    assert_eq!(profile.context.omitted.entities, 3);
    assert_eq!(profile.context.omitted.facts, 1);
    assert_eq!(profile.context.omitted.relations, 2);
    assert_eq!(profile.context.omitted.diagnostics, 0);
    assert_eq!(profile.validation_report.metrics.unsupported_relations, 2);
    assert!(profile.document.content.contains(
        "Omitted API scope: entities 3, facts 1, relations 2, diagnostics 0"
    ));
    assert!(profile.document.content.contains("Unsupported API relations: 2"));

    let selected_entities = profile
        .context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Entity)
        .map(|item| item.stable_keys[0].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        selected_entities,
        vec!["api://GET:/health", "api-schema://openapi.yaml#Health"]
    );

    let mut reversed = snapshot.clone();
    reversed.entities.reverse();
    reversed.facts.reverse();
    reversed.relations.reverse();
    reversed.diagnostics.reverse();
    let repeated = build_documentation_api_profile(&request, &reversed).unwrap();
    assert_eq!(profile.outline, repeated.outline);
    assert_eq!(profile.context, repeated.context);
    assert_eq!(profile.draft, repeated.draft);
    assert_eq!(profile.validation_report, repeated.validation_report);
    assert_eq!(profile.document, repeated.document);
}

#[test]
fn api_profile_fails_closed_for_wrong_identity_or_missing_api_surface() {
    let snapshot = api_snapshot();
    let wrong_profile = DocumentationGenerationRequest::new(
        "snap-api-0001",
        DocumentationProfile::Module,
        limits(8),
    );
    assert!(
        build_documentation_api_profile(&wrong_profile, &snapshot)
            .unwrap_err()
            .to_string()
            .contains("requires profile `api`")
    );

    let wrong_snapshot = DocumentationGenerationRequest::new(
        "snap-other",
        DocumentationProfile::Api,
        limits(8),
    );
    assert!(
        build_documentation_api_profile(&wrong_snapshot, &snapshot)
            .unwrap_err()
            .to_string()
            .contains("does not match canonical snapshot")
    );

    let empty = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-api-0001".to_string())),
        ..CanonicalSnapshot::default()
    };
    assert!(
        build_documentation_api_profile(&request(8), &empty)
            .unwrap_err()
            .to_string()
            .contains("has no evidence-backed API endpoint, schema, or example entity")
    );
}

fn api_snapshot() -> CanonicalSnapshot {
    let endpoint_health = entity(
        "endpoint-health",
        "api://GET:/health",
        EntityKind::ApiEndpoint,
        "api\\openapi.yaml",
        json!({"method": "GET", "path": "/health"}),
    );
    let endpoint_login = entity(
        "endpoint-login",
        "api://POST:/login",
        EntityKind::ApiEndpoint,
        "api/openapi.yaml",
        json!({"method": "POST", "path": "/login"}),
    );
    let schema_health = entity(
        "schema-health",
        "api-schema://openapi.yaml#Health",
        EntityKind::ApiSchema,
        "api/openapi.yaml",
        json!({}),
    );
    let schema_login = entity(
        "schema-login",
        "api-schema://openapi.yaml#LoginRequest",
        EntityKind::ApiSchema,
        "api/openapi.yaml",
        json!({}),
    );
    let example_health = entity(
        "example-health",
        "api-example://GET:/health#ok",
        EntityKind::ApiExample,
        "api/openapi.yaml",
        json!({}),
    );
    let handler = entity(
        "handler-health",
        "symbol://rust:health",
        EntityKind::Function,
        "src/health.rs",
        json!({}),
    );
    let api_file = entity(
        "file-openapi",
        "file://api/openapi.yaml",
        EntityKind::File,
        "api/openapi.yaml",
        json!({}),
    );
    let docs_page = entity(
        "docs-api",
        "doc://docs/api.md",
        EntityKind::DocumentationPage,
        "docs/api.md",
        json!({}),
    );
    let docs_section = entity(
        "docs-health",
        "doc://docs/api.md#health",
        EntityKind::DocumentationSection,
        "docs/api.md",
        json!({}),
    );

    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-api-0001".to_string())),
        entities: vec![
            endpoint_login.clone(),
            schema_health.clone(),
            example_health.clone(),
            handler.clone(),
            endpoint_health.clone(),
            docs_page.clone(),
            schema_login.clone(),
            api_file.clone(),
            docs_section.clone(),
        ],
        facts: vec![
            fact(
                "fact-route-health",
                FactKind::RouteDeclared,
                &endpoint_health,
                Some(&api_file),
                "api/openapi.yaml",
            ),
            fact(
                "fact-schema-health",
                FactKind::Other("api_schema_declared".to_string()),
                &schema_health,
                Some(&api_file),
                "api/openapi.yaml",
            ),
            fact(
                "fact-unrelated-file",
                FactKind::FileDiscovered,
                &api_file,
                None,
                "api/openapi.yaml",
            ),
        ],
        relations: vec![
            relation(
                "rel-implemented",
                RelationKind::ImplementedBy,
                &endpoint_health,
                &handler,
                "src/health.rs",
            ),
            relation(
                "rel-request-schema",
                RelationKind::SchemaForRequest,
                &endpoint_login,
                &schema_login,
                "api/openapi.yaml",
            ),
            relation(
                "rel-response-schema",
                RelationKind::SchemaForResponse,
                &endpoint_health,
                &schema_health,
                "api/openapi.yaml",
            ),
            relation(
                "rel-example",
                RelationKind::ExampleFor,
                &example_health,
                &endpoint_health,
                "api/openapi.yaml",
            ),
            relation(
                "rel-docs",
                RelationKind::Documents,
                &docs_section,
                &endpoint_login,
                "docs/api.md",
            ),
            relation(
                "rel-docs-api",
                RelationKind::DocumentsApi,
                &docs_page,
                &endpoint_login,
                "docs/api.md",
            ),
            relation(
                "rel-docs-operation",
                RelationKind::DocumentsOperation,
                &docs_section,
                &endpoint_health,
                "docs/api.md",
            ),
            relation(
                "rel-graphql-other",
                RelationKind::Other("graphql_uses_fragment".to_string()),
                &endpoint_health,
                &schema_health,
                "api/schema.graphql",
            ),
        ],
        diagnostics: vec![
            diagnostic(
                "diag-example",
                DiagnosticKind::ApiExampleInvalid,
                Severity::High,
                DiagnosticStatus::Open,
                "invalid example",
                vec![example_health.id.clone()],
                "api/openapi.yaml",
            ),
            diagnostic(
                "diag-drift",
                DiagnosticKind::Other("api_openapi_graphql_drift".to_string()),
                Severity::Medium,
                DiagnosticStatus::Open,
                "cross-protocol drift",
                vec![endpoint_health.id.clone(), endpoint_login.id.clone()],
                "api/schema.graphql",
            ),
            diagnostic(
                "diag-resolved",
                DiagnosticKind::ApiResponseSchemaMismatch,
                Severity::High,
                DiagnosticStatus::Resolved,
                "resolved response drift",
                vec![endpoint_health.id.clone()],
                "api/openapi.yaml",
            ),
            diagnostic(
                "diag-unrelated",
                DiagnosticKind::MissingDocumentation,
                Severity::Low,
                DiagnosticStatus::Open,
                "unrelated file diagnostic",
                vec![api_file.id.clone()],
                "api/openapi.yaml",
            ),
        ],
    }
}

fn entity(
    id: &str,
    stable_key: &str,
    kind: EntityKind,
    path: &str,
    payload: serde_json::Value,
) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind,
        name: stable_key.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: path.to_string(),
            line_start: Some(7),
            line_end: Some(9),
        }),
        language: None,
        aliases: Vec::new(),
        ownership: vec![Ownership {
            source_file: path.replace('\\', "/"),
        }],
        payload,
    }
}

fn fact(
    id: &str,
    kind: FactKind,
    subject: &Entity,
    object: Option<&Entity>,
    path: &str,
) -> Fact {
    Fact {
        id: FactId(id.to_string()),
        kind,
        subject: subject.id.clone(),
        object: object.map(|entity| entity.id.clone()),
        value: json!({}),
        evidence: vec![evidence(path, 11)],
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        snapshot: SnapshotId("snap-api-0001".to_string()),
        extractor: "fixture".to_string(),
        confidence: 1.0,
    }
}

fn relation(id: &str, kind: RelationKind, from: &Entity, to: &Entity, path: &str) -> Relation {
    Relation {
        id: RelationId(id.to_string()),
        kind,
        from: from.id.clone(),
        to: to.id.clone(),
        status: RelationStatus::Verified,
        confidence: 1.0,
        evidence: vec![evidence(path, 17)],
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        snapshot: SnapshotId("snap-api-0001".to_string()),
        payload: json!({}),
    }
}

fn diagnostic(
    id: &str,
    kind: DiagnosticKind,
    severity: Severity,
    status: DiagnosticStatus,
    title: &str,
    entities: Vec<EntityId>,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId(id.to_string()),
        kind,
        severity,
        status,
        title: title.to_string(),
        message: title.to_string(),
        entities,
        evidence: vec![evidence(path, 23)],
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        snapshot: SnapshotId("snap-api-0001".to_string()),
        suggested_fix: None,
        payload: json!({}),
    }
}

fn evidence(path: &str, line: u32) -> Evidence {
    Evidence {
        source_file: Some(path.to_string()),
        line_start: Some(line),
        line_end: Some(line + 1),
        extractor: Some("fixture".to_string()),
        commit_hash: None,
        confidence: 1.0,
        status: EvidenceStatus::Verified,
    }
}

fn request(max_entities: usize) -> DocumentationGenerationRequest {
    DocumentationGenerationRequest::new(
        "snap-api-0001",
        DocumentationProfile::Api,
        limits(max_entities),
    )
}

fn limits(max_entities: usize) -> DocumentationGenerationLimits {
    DocumentationGenerationLimits {
        max_entities,
        max_facts: 8,
        max_relations: 8,
        max_diagnostics: 8,
    }
}

fn count_kind(
    profile: &athanor_app::DocumentationApiProfile,
    kind: DocumentationContextItemKind,
) -> usize {
    profile
        .context
        .items
        .iter()
        .filter(|item| item.kind == kind)
        .count()
}

fn sha256_hex(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}
