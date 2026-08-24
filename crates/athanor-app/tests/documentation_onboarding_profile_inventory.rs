use athanor_app::{
    DOCUMENTATION_REFERENCE_LIMIT, DocumentationContextItemKind, DocumentationGenerationLimits,
    DocumentationGenerationRequest, DocumentationProfile, DocumentationValidationStatus,
    ONBOARDING_DOCUMENT_MEDIA_TYPE, ONBOARDING_DOCUMENT_PATH, build_documentation_onboarding_profile,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Entity, EntityId, EntityKind, Ownership, SnapshotId, SourceLocation, StableKey,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[test]
fn onboarding_profile_is_exact_bounded_cited_and_checksum_bound() {
    let snapshot = onboarding_snapshot();
    let profile = build_documentation_onboarding_profile(&request(6), &snapshot).unwrap();

    assert_eq!(profile.context.profile, DocumentationProfile::Onboarding);
    assert_eq!(profile.context.items.len(), 6);
    assert!(
        profile
            .context
            .items
            .iter()
            .all(|item| item.kind == DocumentationContextItemKind::Entity)
    );
    assert_eq!(profile.context.omitted.entities, 1);
    assert_eq!(profile.context.omitted.facts, 0);
    assert_eq!(profile.context.omitted.relations, 0);
    assert_eq!(profile.context.omitted.diagnostics, 0);
    assert_eq!(profile.draft.citations.len(), 6);
    assert_eq!(profile.validation_report.status, DocumentationValidationStatus::Valid);
    assert_eq!(profile.validation_report.profile, DocumentationProfile::Onboarding);
    assert_eq!(profile.validation_report.metrics.unsupported_relations, 0);
    assert_eq!(profile.document.path, ONBOARDING_DOCUMENT_PATH);
    assert_eq!(profile.document.media_type, ONBOARDING_DOCUMENT_MEDIA_TYPE);
    assert_eq!(
        profile.document.sha256,
        sha256_hex(profile.document.content.as_bytes())
    );
    assert!(profile.document.content.contains("# Onboarding Documentation"));
    assert!(profile.document.content.contains("## Onboarding Overview"));
    assert!(profile.document.content.contains("## Getting Started Inventory"));
    assert!(profile.document.content.contains("- Profile: `onboarding`"));
    assert!(profile.document.content.contains("Slice 5A scope"));
    assert!(!profile.document.content.contains("module://core"));
    assert!(!profile.document.content.contains("automation://script/bootstrap"));
    assert_eq!(
        serde_json::to_value(DocumentationProfile::Onboarding).unwrap(),
        json!("onboarding")
    );

    let ids = profile
        .context
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "onboarding-guides-0001",
            "onboarding-sections-0002",
            "onboarding-packages-0003",
            "onboarding-commands-0004",
            "onboarding-environment-0005",
            "onboarding-verification-0006",
        ]
    );

    let keys = profile
        .context
        .items
        .iter()
        .map(|item| item.stable_keys[0].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            "doc://docs/README.md",
            "doc://docs/README.md#getting-started",
            "package://workspace",
            "automation://command/bootstrap",
            "env://DATABASE_URL",
            "automation://ci/quality",
        ]
    );

    let complete = build_documentation_onboarding_profile(&request(7), &snapshot).unwrap();
    assert_eq!(complete.context.omitted.entities, 0);
    assert!(
        complete
            .context
            .items
            .iter()
            .any(|item| item.stable_keys == ["test://smoke"])
    );

    let guide = &profile.context.items[0];
    assert!(guide.evidence.iter().any(|location| {
        location.path == "docs/README.md" && location.start_line == 1 && location.end_line == 1
    }));
    assert!(guide.evidence.iter().any(|location| {
        location.path == "docs/README.md" && location.start_line == 2 && location.end_line == 12
    }));
}

#[test]
fn onboarding_profile_ignores_input_order_and_discloses_omissions() {
    let snapshot = onboarding_snapshot();
    let request = request(6);
    let profile = build_documentation_onboarding_profile(&request, &snapshot).unwrap();

    let mut reversed = snapshot.clone();
    reversed.entities.reverse();
    let repeated = build_documentation_onboarding_profile(&request, &reversed).unwrap();

    assert_eq!(profile.outline, repeated.outline);
    assert_eq!(profile.context, repeated.context);
    assert_eq!(profile.draft, repeated.draft);
    assert_eq!(profile.validation_report, repeated.validation_report);
    assert_eq!(profile.document, repeated.document);
    assert!(profile.document.content.contains("Omitted onboarding entities: 1"));
}

#[test]
fn onboarding_profile_honors_shared_reference_ceiling() {
    let entities = (0..300)
        .map(|index| {
            entity(
                &format!("env-{index:03}"),
                &format!("env://VAR_{index:03}"),
                EntityKind::EnvVar,
                &format!("config/env-{index:03}.txt"),
                json!({}),
            )
        })
        .collect::<Vec<_>>();
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-onboarding-limit".to_string())),
        entities,
        ..CanonicalSnapshot::default()
    };
    let request = DocumentationGenerationRequest::new(
        "snap-onboarding-limit",
        DocumentationProfile::Onboarding,
        DocumentationGenerationLimits {
            max_entities: 1_000,
            max_facts: 1,
            max_relations: 1,
            max_diagnostics: 1,
        },
    );
    let profile = build_documentation_onboarding_profile(&request, &snapshot).unwrap();

    assert_eq!(profile.context.items.len(), DOCUMENTATION_REFERENCE_LIMIT);
    assert_eq!(profile.draft.citations.len(), DOCUMENTATION_REFERENCE_LIMIT);
    assert_eq!(profile.context.omitted.entities, 44);
}

#[test]
fn onboarding_profile_fails_closed_for_wrong_identity_or_missing_surface() {
    let snapshot = onboarding_snapshot();
    let wrong_profile = DocumentationGenerationRequest::new(
        "snap-onboarding-0001",
        DocumentationProfile::Operations,
        limits(8),
    );
    assert!(
        build_documentation_onboarding_profile(&wrong_profile, &snapshot)
            .unwrap_err()
            .to_string()
            .contains("requires profile `onboarding`")
    );

    let wrong_snapshot = DocumentationGenerationRequest::new(
        "snap-other",
        DocumentationProfile::Onboarding,
        limits(8),
    );
    assert!(
        build_documentation_onboarding_profile(&wrong_snapshot, &snapshot)
            .unwrap_err()
            .to_string()
            .contains("does not match canonical snapshot")
    );

    let empty = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-onboarding-empty".to_string())),
        entities: vec![entity(
            "module-core",
            "module://core",
            EntityKind::Module,
            "src/lib.rs",
            json!({}),
        )],
        ..CanonicalSnapshot::default()
    };
    let empty_request = DocumentationGenerationRequest::new(
        "snap-onboarding-empty",
        DocumentationProfile::Onboarding,
        limits(8),
    );
    assert!(
        build_documentation_onboarding_profile(&empty_request, &empty)
            .unwrap_err()
            .to_string()
            .contains("no evidence-backed onboarding entity")
    );
}

fn onboarding_snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-onboarding-0001".to_string())),
        entities: vec![
            entity_with_title(
                "guide-docs",
                "doc://docs/README.md",
                EntityKind::DocumentationPage,
                "docs\\README.md",
                Some("Documentation Map"),
                Some((2, 12)),
                json!({"documentation_kind": "project_overview"}),
            ),
            entity(
                "section-getting-started",
                "doc://docs/README.md#getting-started",
                EntityKind::DocumentationSection,
                "docs/README.md",
                json!({"level": 2}),
            ),
            entity(
                "package-workspace",
                "package://workspace",
                EntityKind::Package,
                "Cargo.toml",
                json!({}),
            ),
            entity(
                "command-bootstrap",
                "automation://command/bootstrap",
                EntityKind::ScriptCommand,
                "Makefile",
                json!({}),
            ),
            entity(
                "env-database",
                "env://DATABASE_URL",
                EntityKind::EnvVar,
                "config/example.env",
                json!({}),
            ),
            entity(
                "test-smoke",
                "test://smoke",
                EntityKind::TestCase,
                "tests/smoke.rs",
                json!({}),
            ),
            entity(
                "ci-quality",
                "automation://ci/quality",
                EntityKind::CiJob,
                ".github/workflows/ci.yml",
                json!({}),
            ),
            entity(
                "module-core",
                "module://core",
                EntityKind::Module,
                "src/lib.rs",
                json!({}),
            ),
            entity(
                "script-bootstrap",
                "automation://script/bootstrap",
                EntityKind::Script,
                "scripts/bootstrap.sh",
                json!({}),
            ),
        ],
        ..CanonicalSnapshot::default()
    }
}

fn entity(id: &str, stable_key: &str, kind: EntityKind, path: &str, payload: Value) -> Entity {
    entity_with_title(id, stable_key, kind, path, None, Some((1, 1)), payload)
}

fn entity_with_title(
    id: &str,
    stable_key: &str,
    kind: EntityKind,
    path: &str,
    title: Option<&str>,
    lines: Option<(u32, u32)>,
    payload: Value,
) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind,
        name: stable_key.to_string(),
        title: title.map(str::to_string),
        source: Some(SourceLocation {
            path: path.to_string(),
            line_start: lines.map(|lines| lines.0),
            line_end: lines.map(|lines| lines.1),
        }),
        language: None,
        aliases: Vec::new(),
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        payload,
    }
}

fn request(max_entities: usize) -> DocumentationGenerationRequest {
    DocumentationGenerationRequest::new(
        "snap-onboarding-0001",
        DocumentationProfile::Onboarding,
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

fn sha256_hex(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}
