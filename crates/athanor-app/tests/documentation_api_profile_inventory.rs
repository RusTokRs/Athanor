use athanor_app::{
    API_DOCUMENT_MEDIA_TYPE, API_DOCUMENT_PATH, DOCUMENTATION_REFERENCE_LIMIT,
    DocumentationContextItemKind, DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationProfile, DocumentationValidationStatus, build_documentation_api_profile,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{Entity, EntityId, EntityKind, Ownership, SnapshotId, SourceLocation, StableKey};
use serde_json::json;
use sha2::{Digest, Sha256};

#[test]
fn api_profile_is_exact_cited_bounded_and_checksum_bound() {
    let snapshot = api_snapshot();
    let profile = build_documentation_api_profile(&request(4), &snapshot).unwrap();

    assert_eq!(profile.outline.profile, DocumentationProfile::Api);
    assert_eq!(
        profile
            .outline
            .sections
            .iter()
            .map(|section| section.id.as_str())
            .collect::<Vec<_>>(),
        vec!["overview", "endpoints", "schemas", "examples"]
    );
    assert_eq!(profile.context.profile, DocumentationProfile::Api);
    assert_eq!(profile.context.items.len(), 4);
    assert_eq!(profile.context.omitted.entities, 1);
    assert_eq!(profile.context.omitted.facts, 0);
    assert_eq!(profile.context.omitted.relations, 0);
    assert_eq!(profile.context.omitted.diagnostics, 0);
    assert!(
        profile
            .context
            .items
            .iter()
            .all(|item| item.kind == DocumentationContextItemKind::Entity)
    );
    assert_eq!(profile.draft.citations.len(), 4);
    assert_eq!(profile.validation_report.status, DocumentationValidationStatus::Valid);
    assert_eq!(profile.validation_report.profile, DocumentationProfile::Api);
    assert_eq!(profile.document.path, API_DOCUMENT_PATH);
    assert_eq!(profile.document.media_type, API_DOCUMENT_MEDIA_TYPE);
    assert!(profile.document.content.contains("- Profile: `api`"));
    assert!(profile.document.content.contains("## API Endpoints"));
    assert!(profile.document.content.contains("## API Schemas"));
    assert!(profile.document.content.contains("## API Examples"));
    assert!(profile.document.content.contains("Omitted API entities: 1"));
    assert!(profile.document.content.contains("Slice 3A scope"));
    assert_eq!(profile.document.sha256, sha256_hex(profile.document.content.as_bytes()));
    assert_eq!(
        serde_json::to_value(DocumentationProfile::Api).unwrap(),
        json!("api")
    );
    assert!(profile.context.items.len() <= DOCUMENTATION_REFERENCE_LIMIT);

    let endpoint = &profile.context.items[0];
    assert!(endpoint.id.starts_with("api-endpoint-"));
    assert!(endpoint.summary.contains("GET /health"));
    assert_eq!(endpoint.evidence[0].path, "api/openapi.yaml");
}

#[test]
fn api_profile_round_robins_kinds_and_ignores_canonical_input_order() {
    let snapshot = api_snapshot();
    let profile = build_documentation_api_profile(&request(4), &snapshot).unwrap();
    assert_eq!(
        profile
            .context
            .items
            .iter()
            .map(|item| item.stable_keys[0].as_str())
            .collect::<Vec<_>>(),
        vec![
            "api://GET:/health",
            "api-schema://openapi.yaml#Health",
            "api-example://GET:/health#ok",
            "api://POST:/login",
        ]
    );

    let mut reversed = snapshot.clone();
    reversed.entities.reverse();
    let repeated = build_documentation_api_profile(&request(4), &reversed).unwrap();
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
    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-api-0001".to_string())),
        entities: vec![
            entity(
                "endpoint-login",
                "api://POST:/login",
                EntityKind::ApiEndpoint,
                "api/openapi.yaml",
                json!({"method": "POST", "path": "/login"}),
            ),
            entity(
                "schema-health",
                "api-schema://openapi.yaml#Health",
                EntityKind::ApiSchema,
                "api/openapi.yaml",
                json!({}),
            ),
            entity(
                "example-health",
                "api-example://GET:/health#ok",
                EntityKind::ApiExample,
                "api/openapi.yaml",
                json!({}),
            ),
            entity(
                "endpoint-health",
                "api://GET:/health",
                EntityKind::ApiEndpoint,
                "api\\openapi.yaml",
                json!({"method": "GET", "path": "/health"}),
            ),
            entity(
                "schema-login",
                "api-schema://openapi.yaml#LoginRequest",
                EntityKind::ApiSchema,
                "api/openapi.yaml",
                json!({}),
            ),
        ],
        facts: Vec::new(),
        relations: Vec::new(),
        diagnostics: Vec::new(),
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

fn sha256_hex(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}
