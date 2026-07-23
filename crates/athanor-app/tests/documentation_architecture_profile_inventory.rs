use athanor_app::{
    ARCHITECTURE_DOCUMENT_MEDIA_TYPE, ARCHITECTURE_DOCUMENT_PATH, DocumentationContextItemKind,
    DocumentationGenerationLimits, DocumentationGenerationRequest, DocumentationProfile,
    DocumentationValidationStatus, build_documentation_architecture_profile,
};
use athanor_core::CanonicalSnapshot;
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("fixtures/documentation_architecture_profile.v1.json");

#[test]
fn architecture_profile_is_cited_valid_and_checksum_bound() {
    let snapshot = fixture_snapshot();
    let profile = build_documentation_architecture_profile(&request(full_limits()), &snapshot)
        .expect("deterministic architecture profile");

    assert_eq!(
        profile
            .outline
            .sections
            .iter()
            .map(|section| section.id.as_str())
            .collect::<Vec<_>>(),
        ["overview", "components", "relationships", "diagnostics"]
    );
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Entity),
        3
    );
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Fact),
        1
    );
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Relation),
        2
    );
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Diagnostic),
        1
    );
    assert_eq!(profile.context.omitted.entities, 0);
    assert_eq!(profile.context.omitted.facts, 0);
    assert_eq!(profile.context.omitted.relations, 0);
    assert_eq!(profile.context.omitted.diagnostics, 0);
    assert!(!profile.context.policy.provider_enabled);
    assert!(!profile.context.policy.network_enabled);
    assert!(!profile.context.policy.raw_file_access);
    assert!(!profile.context.policy.secrets_included);

    assert_eq!(
        profile.validation_report.status,
        DocumentationValidationStatus::Valid
    );
    assert_eq!(
        profile
            .validation_report
            .metrics
            .citation_coverage_basis_points,
        10_000
    );
    assert_eq!(
        profile
            .validation_report
            .metrics
            .citation_validity_basis_points,
        10_000
    );
    assert_eq!(
        profile
            .validation_report
            .metrics
            .diagram_validity_basis_points,
        10_000
    );
    assert!(
        profile
            .validation_report
            .metrics
            .deterministic_repeatability
    );

    assert_eq!(profile.document.path, ARCHITECTURE_DOCUMENT_PATH);
    assert_eq!(
        profile.document.media_type,
        ARCHITECTURE_DOCUMENT_MEDIA_TYPE
    );
    assert!(
        profile
            .document
            .content
            .starts_with("# Architecture Overview\n")
    );
    for required in [
        "- Snapshot: `snap-architecture-0001`",
        "## Components",
        "## Relationships",
        "```mermaid",
        "flowchart LR",
        "-->|contains|",
        "-->|documents|",
        "## Diagnostics",
        "## Evidence",
        "[^citation-entity-0001]",
        "src/lib.rs:3-8",
    ] {
        assert!(
            profile.document.content.contains(required),
            "architecture Markdown omits {required}"
        );
    }
    assert_eq!(
        profile.document.sha256,
        format!("{:x}", Sha256::digest(profile.document.content.as_bytes()))
    );
}

#[test]
fn architecture_profile_is_invariant_to_canonical_input_order() {
    let snapshot = fixture_snapshot();
    let mut reversed = snapshot.clone();
    reversed.entities.reverse();
    reversed.facts.reverse();
    reversed.relations.reverse();
    reversed.diagnostics.reverse();

    let request = request(full_limits());
    let original = build_documentation_architecture_profile(&request, &snapshot).unwrap();
    let reordered = build_documentation_architecture_profile(&request, &reversed).unwrap();
    assert_eq!(original, reordered);
}

#[test]
fn architecture_profile_enforces_limits_and_discloses_omissions() {
    let snapshot = fixture_snapshot();
    let profile = build_documentation_architecture_profile(
        &request(DocumentationGenerationLimits {
            max_entities: 1,
            max_facts: 1,
            max_relations: 1,
            max_diagnostics: 1,
        }),
        &snapshot,
    )
    .unwrap();

    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Entity),
        1
    );
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Fact),
        1
    );
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Relation),
        1
    );
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Diagnostic),
        1
    );
    assert_eq!(profile.context.omitted.entities, 2);
    assert_eq!(profile.context.omitted.relations, 1);
    assert!(
        profile
            .document
            .content
            .contains("- Omitted: entities 2, facts 0, relations 1, diagnostics 0")
    );
    assert_eq!(profile.validation_report.metrics.unsupported_relations, 1);
}

#[test]
fn architecture_profile_requires_exact_snapshot_and_evidence_backed_entities() {
    let mut snapshot = fixture_snapshot();
    snapshot.snapshot = None;
    assert!(build_documentation_architecture_profile(&request(full_limits()), &snapshot).is_err());

    let snapshot = fixture_snapshot();
    let mut wrong_request = request(full_limits());
    wrong_request.snapshot = "snap-other".to_string();
    assert!(build_documentation_architecture_profile(&wrong_request, &snapshot).is_err());

    let mut unsupported = fixture_snapshot();
    for entity in &mut unsupported.entities {
        entity.source = None;
        entity.ownership.clear();
    }
    assert!(
        build_documentation_architecture_profile(&request(full_limits()), &unsupported).is_err()
    );
}

fn fixture_snapshot() -> CanonicalSnapshot {
    serde_json::from_str(FIXTURE).expect("valid canonical architecture fixture")
}

fn request(limits: DocumentationGenerationLimits) -> DocumentationGenerationRequest {
    DocumentationGenerationRequest::new(
        "snap-architecture-0001",
        DocumentationProfile::Architecture,
        limits,
    )
}

fn full_limits() -> DocumentationGenerationLimits {
    DocumentationGenerationLimits {
        max_entities: 16,
        max_facts: 16,
        max_relations: 16,
        max_diagnostics: 8,
    }
}

fn count(context: &athanor_app::DocumentationContext, kind: DocumentationContextItemKind) -> usize {
    context
        .items
        .iter()
        .filter(|item| item.kind == kind)
        .count()
}
