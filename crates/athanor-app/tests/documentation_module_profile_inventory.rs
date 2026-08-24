use athanor_app::{
    MODULE_DOCUMENT_MEDIA_TYPE, MODULE_DOCUMENT_PATH, DocumentationContextItemKind,
    DocumentationGenerationLimits, DocumentationGenerationRequest, DocumentationProfile,
    DocumentationValidationStatus, build_documentation_architecture_profile,
    build_documentation_module_profile,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{EntityId, EntityKind};
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("fixtures/documentation_architecture_profile.v1.json");

#[test]
fn module_profile_is_exact_scoped_cited_and_checksum_bound() {
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
        ["overview", "modules", "relationships", "diagnostics"]
    );
    assert_eq!(profile.context.profile, DocumentationProfile::Module);
    assert_eq!(
        count_kind(&profile.context.items, DocumentationContextItemKind::Entity),
        1
    );
    assert_eq!(
        count_kind(&profile.context.items, DocumentationContextItemKind::Fact),
        1
    );
    assert_eq!(
        count_kind(
            &profile.context.items,
            DocumentationContextItemKind::Relation
        ),
        1
    );
    assert_eq!(
        count_kind(
            &profile.context.items,
            DocumentationContextItemKind::Diagnostic
        ),
        1
    );
    assert_eq!(profile.context.omitted.entities, 0);
    assert_eq!(profile.context.omitted.facts, 0);
    assert_eq!(profile.context.omitted.relations, 0);
    assert_eq!(profile.context.omitted.diagnostics, 0);
    assert_eq!(profile.draft.citations.len(), 4);
    assert_eq!(
        profile.validation_report.status,
        DocumentationValidationStatus::Valid
    );
    assert_eq!(profile.validation_report.metrics.unsupported_relations, 0);
    assert_eq!(profile.document.path, MODULE_DOCUMENT_PATH);
    assert_eq!(profile.document.media_type, MODULE_DOCUMENT_MEDIA_TYPE);

    for required in [
        "# Module Overview",
        "- Profile: `module`",
        "## Modules",
        "## Module Relationships",
        "## Module Diagnostics",
        "Fact symbol_defined describes `rust://module/service`.",
        "`rust://package/demo` contains `rust://module/service`.",
        "medium diagnostic uncovered_symbol: Service lacks direct coverage",
        "flowchart LR",
        "src/lib.rs:3-8",
        "Slice 2B scope: source-backed modules plus module-scoped facts, relations, and open diagnostics",
    ] {
        assert!(
            profile.document.content.contains(required),
            "module Markdown omits {required}"
        );
    }
    assert!(
        !profile.document.content.contains("file://README.md"),
        "unrelated documentation relation escaped the selected module scope"
    );
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
fn module_profile_matches_architecture_evidence_semantics_for_shared_items() {
    let snapshot = fixture_snapshot();
    let module = build_documentation_module_profile(&request(16), &snapshot).unwrap();
    let architecture = build_documentation_architecture_profile(
        &DocumentationGenerationRequest::new(
            "snap-architecture-0001",
            DocumentationProfile::Architecture,
            limits(16),
        ),
        &snapshot,
    )
    .unwrap();

    for kind in [
        DocumentationContextItemKind::Entity,
        DocumentationContextItemKind::Fact,
        DocumentationContextItemKind::Relation,
        DocumentationContextItemKind::Diagnostic,
    ] {
        let module_item = module
            .context
            .items
            .iter()
            .find(|item| item.kind == kind)
            .unwrap();
        let architecture_item = architecture
            .context
            .items
            .iter()
            .find(|item| {
                item.kind == kind
                    && item.stable_keys == module_item.stable_keys
                    && item.source_stable_key == module_item.source_stable_key
                    && item.target_stable_key == module_item.target_stable_key
            })
            .unwrap();
        assert_eq!(module_item.stable_keys, architecture_item.stable_keys);
        assert_eq!(module_item.evidence, architecture_item.evidence);
        assert_eq!(
            module_item.relation_direction,
            architecture_item.relation_direction
        );
    }
}

#[test]
fn module_profile_orders_modules_discloses_omissions_and_ignores_input_order() {
    let snapshot = multi_module_snapshot();
    let profile = build_documentation_module_profile(&request(2), &snapshot).unwrap();

    assert_eq!(profile.context.omitted.entities, 1);
    assert_eq!(profile.context.omitted.facts, 0);
    assert_eq!(profile.context.omitted.relations, 0);
    assert_eq!(profile.context.omitted.diagnostics, 0);
    assert_eq!(
        profile
            .context
            .items
            .iter()
            .filter(|item| item.kind == DocumentationContextItemKind::Entity)
            .map(|item| item.stable_keys[0].as_str())
            .collect::<Vec<_>>(),
        ["rust://module/alpha", "rust://module/service"]
    );
    assert!(profile.document.content.contains(
        "- Omitted module scope: modules 1, facts 0, relations 0, diagnostics 0"
    ));

    let mut reordered = snapshot.clone();
    reordered.entities.reverse();
    reordered.facts.reverse();
    reordered.relations.reverse();
    reordered.diagnostics.reverse();
    assert_eq!(
        profile,
        build_documentation_module_profile(&request(2), &reordered).unwrap()
    );
}

#[test]
fn module_profile_scopes_facts_relations_and_diagnostics_to_selected_modules() {
    let snapshot = fixture_snapshot();
    assert_eq!(snapshot.relations.len(), 2);
    let profile = build_documentation_module_profile(&request(16), &snapshot).unwrap();

    assert_eq!(
        count_kind(
            &profile.context.items,
            DocumentationContextItemKind::Relation
        ),
        1
    );
    let relation = profile
        .context
        .items
        .iter()
        .find(|item| item.kind == DocumentationContextItemKind::Relation)
        .unwrap();
    assert_eq!(
        relation.source_stable_key.as_deref(),
        Some("rust://package/demo")
    );
    assert_eq!(
        relation.target_stable_key.as_deref(),
        Some("rust://module/service")
    );
    assert_eq!(
        relation.stable_keys,
        [
            "rust://module/service".to_string(),
            "rust://package/demo".to_string()
        ]
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

fn count_kind(
    items: &[athanor_app::DocumentationContextItem],
    kind: DocumentationContextItemKind,
) -> usize {
    items.iter().filter(|item| item.kind == kind).count()
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
