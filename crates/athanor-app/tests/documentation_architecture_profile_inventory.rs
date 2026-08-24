use athanor_app::{
    ARCHITECTURE_DOCUMENT_MEDIA_TYPE, ARCHITECTURE_DOCUMENT_PATH, DOCUMENTATION_REFERENCE_LIMIT,
    DocumentationContextItemKind, DocumentationGenerationLimits, DocumentationGenerationRequest,
    DocumentationProfile, DocumentationValidationStatus, build_documentation_architecture_profile,
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
    assert!(profile.document.content.contains(
        "- Unsupported relations: 1 relations are outside the bounded context and are not represented by relationship claims or Mermaid edges."
    ));
    assert_eq!(profile.validation_report.metrics.unsupported_relations, 1);
}

#[test]
fn architecture_profile_caps_aggregate_citations_without_starving_available_kinds() {
    let snapshot = citation_budget_snapshot();
    let request = request(DocumentationGenerationLimits {
        max_entities: 100,
        max_facts: 100,
        max_relations: 100,
        max_diagnostics: 100,
    });
    let profile = build_documentation_architecture_profile(&request, &snapshot).unwrap();

    assert_eq!(profile.context.items.len(), DOCUMENTATION_REFERENCE_LIMIT);
    assert_eq!(profile.draft.citations.len(), DOCUMENTATION_REFERENCE_LIMIT);
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Entity),
        85
    );
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Fact),
        85
    );
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Relation),
        85
    );
    assert_eq!(
        count(&profile.context, DocumentationContextItemKind::Diagnostic),
        1
    );
    assert_eq!(profile.context.omitted.entities, 15);
    assert_eq!(profile.context.omitted.facts, 15);
    assert_eq!(profile.context.omitted.relations, 15);
    assert_eq!(profile.context.omitted.diagnostics, 0);
    assert_eq!(
        profile.validation_report.status,
        DocumentationValidationStatus::Valid
    );
    assert!(profile.document.content.contains(&format!(
        "- Citation/context budget: {DOCUMENTATION_REFERENCE_LIMIT} items"
    )));

    let mut reordered = snapshot.clone();
    reordered.entities.reverse();
    reordered.facts.reverse();
    reordered.relations.reverse();
    reordered.diagnostics.reverse();
    assert_eq!(
        profile,
        build_documentation_architecture_profile(&request, &reordered).unwrap()
    );
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

fn citation_budget_snapshot() -> CanonicalSnapshot {
    let mut snapshot = fixture_snapshot();
    let entity = snapshot.entities[1].clone();
    let fact = snapshot.facts[0].clone();
    let relation = snapshot.relations[0].clone();

    for index in snapshot.entities.len()..100 {
        let mut clone = entity.clone();
        clone.id.0 = format!("entity-budget-{index:03}");
        clone.stable_key.0 = format!("rust://module/budget-{index:03}");
        snapshot.entities.push(clone);
    }
    for index in snapshot.facts.len()..100 {
        let mut clone = fact.clone();
        clone.id.0 = format!("fact-budget-{index:03}");
        snapshot.facts.push(clone);
    }
    for index in snapshot.relations.len()..100 {
        let mut clone = relation.clone();
        clone.id.0 = format!("relation-budget-{index:03}");
        snapshot.relations.push(clone);
    }

    snapshot
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
