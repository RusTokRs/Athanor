use athanor_app::{
    MODULE_DOCUMENT_MEDIA_TYPE, MODULE_DOCUMENT_PATH, DocumentationGenerationLimits,
    DocumentationGenerationRequest, DocumentationProfile, DocumentationValidationStatus,
    build_documentation_module_profile,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{EntityId, EntityKind};
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("fixtures/documentation_architecture_profile.v1.json");

#[test]
fn module_profile_is_exact_cited_bounded_and_checksum_bound() {
    let snapshot = fixture_snapshot();
    let profile = build_documentation_module_profile(&request(16), &snapshot)
        .expect("deterministic module profile");

    assert_eq!(
        profile
            .outline
            .sections
            .iter()
            .map(|section| section.id.as_str())
            .collect::<Vec<_>>(),
        ["overview", "modules"]
    );
    assert_eq!(profile.context.profile, DocumentationProfile::Module);
    assert_eq!(profile.context.items.len(), 1);
    assert_eq!(profile.context.omitted.entities, 0);
    assert_eq!(profile.draft.citations.len(), 1);
    assert_eq!(
        profile.validation_report.status,
        DocumentationValidationStatus::Valid
    );
    assert_eq!(profile.document.path, MODULE_DOCUMENT_PATH);
    assert_eq!(profile.document.media_type, MODULE_DOCUMENT_MEDIA_TYPE);
    for required in [
        "# Module Overview",
        "- Profile: `module`",
        "## Modules",
        "rust://module/service",
        "src/lib.rs:1-24",
        "Slice 2A scope: canonical module entities only",
    ] {
        assert!(
            profile.document.content.contains(required),
            "module Markdown omits {required}"
        );
    }
    assert_eq!(
        profile.document.sha256,
        format!("{:x}", Sha256::digest(profile.document.content.as_bytes()))
    );
    assert_eq!(
        serde_json::to_value(DocumentationProfile::Module).unwrap(),
        serde_json::Value::String("module".to_string())
    );
}

#[test]
fn module_profile_orders_modules_and_discloses_limit_omissions() {
    let snapshot = multi_module_snapshot();
    let profile = build_documentation_module_profile(&request(2), &snapshot).unwrap();

    assert_eq!(profile.context.items.len(), 2);
    assert_eq!(profile.context.omitted.entities, 1);
    assert_eq!(
        profile
            .context
            .items
            .iter()
            .map(|item| item.stable_keys[0].as_str())
            .collect::<Vec<_>>(),
        ["rust://module/alpha", "rust://module/service"]
    );
    assert!(profile.document.content.contains("- Omitted modules: 1"));

    let mut reordered = snapshot.clone();
    reordered.entities.reverse();
    assert_eq!(
        profile,
        build_documentation_module_profile(&request(2), &reordered).unwrap()
    );
}

#[test]
fn module_profile_fails_closed_for_wrong_profile_snapshot_or_missing_modules() {
    let snapshot = fixture_snapshot();
    let architecture_request = DocumentationGenerationRequest::new(
        "snap-architecture-0001",
        DocumentationProfile::Architecture,
        limits(16),
    );
    assert!(build_documentation_module_profile(&architecture_request, &snapshot).is_err());

    let mut wrong_snapshot = request(16);
    wrong_snapshot.snapshot = "snap-other".to_string();
    assert!(build_documentation_module_profile(&wrong_snapshot, &snapshot).is_err());

    let mut no_modules = snapshot;
    no_modules
        .entities
        .retain(|entity| entity.kind != EntityKind::Module);
    assert!(build_documentation_module_profile(&request(16), &no_modules).is_err());
}

fn fixture_snapshot() -> CanonicalSnapshot {
    serde_json::from_str(FIXTURE).expect("valid canonical documentation fixture")
}

fn multi_module_snapshot() -> CanonicalSnapshot {
    let mut snapshot = fixture_snapshot();
    let template = snapshot
        .entities
        .iter()
        .find(|entity| entity.kind == EntityKind::Module)
        .unwrap()
        .clone();

    for (id, stable_key, name, line) in [
        ("entity-module-alpha", "rust://module/alpha", "alpha", 30),
        ("entity-module-zeta", "rust://module/zeta", "zeta", 40),
    ] {
        let mut module = template.clone();
        module.id = EntityId(id.to_string());
        module.stable_key.0 = stable_key.to_string();
        module.name = name.to_string();
        module.title = None;
        let source = module.source.as_mut().unwrap();
        source.line_start = Some(line);
        source.line_end = Some(line + 4);
        snapshot.entities.push(module);
    }
    snapshot
}

fn request(max_entities: usize) -> DocumentationGenerationRequest {
    DocumentationGenerationRequest::new(
        "snap-architecture-0001",
        DocumentationProfile::Module,
        limits(max_entities),
    )
}

fn limits(max_entities: usize) -> DocumentationGenerationLimits {
    DocumentationGenerationLimits {
        max_entities,
        max_facts: 16,
        max_relations: 16,
        max_diagnostics: 8,
    }
}
