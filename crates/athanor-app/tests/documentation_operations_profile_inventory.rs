use athanor_app::{
    DOCUMENTATION_REFERENCE_LIMIT, DocumentationContextItemKind, DocumentationGenerationLimits,
    DocumentationGenerationRequest, DocumentationProfile, DocumentationValidationStatus,
    OPERATIONS_DOCUMENT_MEDIA_TYPE, OPERATIONS_DOCUMENT_PATH, build_documentation_operations_profile,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Entity, EntityId, EntityKind, Ownership, SnapshotId, SourceLocation, StableKey,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[test]
fn operations_profile_is_exact_bounded_cited_and_checksum_bound() {
    let snapshot = operations_snapshot();
    let profile = build_documentation_operations_profile(&request(16), &snapshot).unwrap();

    assert_eq!(profile.outline.profile, DocumentationProfile::Operations);
    assert_eq!(
        profile
            .outline
            .sections
            .iter()
            .map(|section| section.id.as_str())
            .collect::<Vec<_>>(),
        vec!["overview", "inventory"]
    );
    assert_eq!(profile.context.profile, DocumentationProfile::Operations);
    assert_eq!(count_entities(&profile), 10);
    assert_eq!(profile.context.omitted.entities, 0);
    assert_eq!(profile.context.omitted.facts, 0);
    assert_eq!(profile.context.omitted.relations, 0);
    assert_eq!(profile.context.omitted.diagnostics, 0);
    assert!(profile.context.items.iter().all(|item| {
        item.kind == DocumentationContextItemKind::Entity
            && item.stable_keys.len() == 1
            && !item.evidence.is_empty()
    }));
    assert_eq!(profile.draft.citations.len(), 10);
    assert_eq!(profile.validation_report.status, DocumentationValidationStatus::Valid);
    assert_eq!(profile.validation_report.profile, DocumentationProfile::Operations);
    assert_eq!(profile.validation_report.metrics.unsupported_relations, 0);
    assert_eq!(profile.document.path, OPERATIONS_DOCUMENT_PATH);
    assert_eq!(profile.document.media_type, OPERATIONS_DOCUMENT_MEDIA_TYPE);
    assert!(profile.document.content.contains("- Profile: `operations`"));
    assert!(profile.document.content.contains("## Operations Overview"));
    assert!(profile.document.content.contains("## Operations Inventory"));
    assert!(profile.document.content.contains("Slice 4A scope"));
    assert!(!profile.document.content.contains("package://workspace"));
    assert!(!profile.document.content.contains("dependency://serde"));
    assert_eq!(
        profile.document.sha256,
        sha256_hex(profile.document.content.as_bytes())
    );
    assert_eq!(
        serde_json::to_value(DocumentationProfile::Operations).unwrap(),
        json!("operations")
    );

    let env = profile
        .context
        .items
        .iter()
        .find(|item| item.stable_keys == ["env://DATABASE_URL".to_string()])
        .unwrap();
    assert!(env.evidence.iter().any(|location| {
        location.path == "config/runtime.env"
            && location.start_line == 3
            && location.end_line == 3
    }));
}

#[test]
fn operations_profile_round_robins_categories_and_ignores_input_order() {
    let snapshot = operations_snapshot();
    let profile = build_documentation_operations_profile(&request(6), &snapshot).unwrap();

    assert_eq!(count_entities(&profile), 6);
    assert_eq!(profile.context.omitted.entities, 4);
    assert_eq!(
        profile
            .context
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "operations-environment-0001",
            "operations-automation-0002",
            "operations-deployment-0003",
            "operations-data-0004",
            "operations-configuration-0005",
            "operations-runbooks-0006",
        ]
    );

    let mut reversed = snapshot.clone();
    reversed.entities.reverse();
    let repeated = build_documentation_operations_profile(&request(6), &reversed).unwrap();
    assert_eq!(profile.outline, repeated.outline);
    assert_eq!(profile.context, repeated.context);
    assert_eq!(profile.draft, repeated.draft);
    assert_eq!(profile.validation_report, repeated.validation_report);
    assert_eq!(profile.document, repeated.document);
}

#[test]
fn operations_profile_excludes_non_operational_and_non_portable_entities() {
    let mut snapshot = operations_snapshot();
    snapshot.entities.push(entity(
        "package",
        "package://workspace",
        EntityKind::Package,
        "Cargo.toml",
        json!({}),
    ));
    snapshot.entities.push(entity(
        "dependency",
        "dependency://serde",
        EntityKind::Dependency,
        "Cargo.toml",
        json!({}),
    ));
    snapshot.entities.push(Entity {
        id: EntityId("unsafe-env".to_string()),
        stable_key: StableKey("env://UNSAFE".to_string()),
        kind: EntityKind::EnvVar,
        name: "UNSAFE".to_string(),
        title: None,
        source: Some(SourceLocation {
            path: "/etc/environment".to_string(),
            line_start: Some(1),
            line_end: Some(1),
        }),
        language: None,
        aliases: Vec::new(),
        ownership: Vec::new(),
        payload: json!({}),
    });

    let profile = build_documentation_operations_profile(&request(32), &snapshot).unwrap();
    assert_eq!(count_entities(&profile), 10);
    let keys = profile
        .context
        .items
        .iter()
        .flat_map(|item| item.stable_keys.iter().map(String::as_str))
        .collect::<Vec<_>>();
    assert!(!keys.contains(&"package://workspace"));
    assert!(!keys.contains(&"dependency://serde"));
    assert!(!keys.contains(&"env://UNSAFE"));
}

#[test]
fn operations_profile_enforces_shared_reference_ceiling() {
    let entities = (0..300)
        .map(|index| {
            entity(
                &format!("env-{index:03}"),
                &format!("env://KEY_{index:03}"),
                EntityKind::EnvVar,
                "config/example.env",
                json!({}),
            )
        })
        .collect::<Vec<_>>();
    let snapshot = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-operations-large".to_string())),
        entities,
        ..CanonicalSnapshot::default()
    };
    let request = DocumentationGenerationRequest::new(
        "snap-operations-large",
        DocumentationProfile::Operations,
        DocumentationGenerationLimits {
            max_entities: 300,
            max_facts: 1,
            max_relations: 1,
            max_diagnostics: 1,
        },
    );
    let profile = build_documentation_operations_profile(&request, &snapshot).unwrap();

    assert_eq!(profile.context.items.len(), DOCUMENTATION_REFERENCE_LIMIT);
    assert_eq!(profile.draft.citations.len(), DOCUMENTATION_REFERENCE_LIMIT);
    assert_eq!(profile.context.omitted.entities, 44);
}

#[test]
fn operations_profile_fails_closed_for_wrong_identity_or_missing_surface() {
    let snapshot = operations_snapshot();
    let wrong_profile = DocumentationGenerationRequest::new(
        "snap-operations-0001",
        DocumentationProfile::Api,
        limits(16),
    );
    assert!(
        build_documentation_operations_profile(&wrong_profile, &snapshot)
            .unwrap_err()
            .to_string()
            .contains("requires profile `operations`")
    );

    let wrong_snapshot = DocumentationGenerationRequest::new(
        "snap-other",
        DocumentationProfile::Operations,
        limits(16),
    );
    assert!(
        build_documentation_operations_profile(&wrong_snapshot, &snapshot)
            .unwrap_err()
            .to_string()
            .contains("does not match canonical snapshot")
    );

    let empty = CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-operations-0001".to_string())),
        entities: vec![entity(
            "module",
            "module://core",
            EntityKind::Module,
            "src/lib.rs",
            json!({}),
        )],
        ..CanonicalSnapshot::default()
    };
    assert!(
        build_documentation_operations_profile(&request(16), &empty)
            .unwrap_err()
            .to_string()
            .contains("has no evidence-backed operational entity")
    );
}

fn operations_snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-operations-0001".to_string())),
        entities: vec![
            entity(
                "env-database",
                "env://DATABASE_URL",
                EntityKind::EnvVar,
                "config\\runtime.env",
                json!({"has_default": false}),
            ),
            entity(
                "script-deploy",
                "automation://script/deploy",
                EntityKind::Script,
                "scripts/deploy.sh",
                json!({}),
            ),
            entity(
                "command-migrate",
                "automation://command/migrate",
                EntityKind::ScriptCommand,
                "Makefile",
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
                "service-api",
                "deployment://service/api",
                EntityKind::DockerService,
                "deploy/docker-compose.yml",
                json!({}),
            ),
            entity(
                "migration-users",
                "data://migration/001-users",
                EntityKind::DbMigration,
                "migrations/001_users.sql",
                json!({}),
            ),
            entity(
                "table-users",
                "data://table/users",
                EntityKind::DbTable,
                "migrations/001_users.sql",
                json!({}),
            ),
            entity(
                "config-port",
                "configuration://server.port",
                EntityKind::Feature,
                "config/app.toml",
                json!({}),
            ),
            entity(
                "runbook-deploy",
                "runbook://deploy",
                EntityKind::Runbook,
                "docs/runbooks/deploy.md",
                json!({}),
            ),
            entity(
                "runbook-step-rollback",
                "runbook-step://rollback",
                EntityKind::OperationStep,
                "docs/runbooks/deploy.md",
                json!({}),
            ),
        ],
        ..CanonicalSnapshot::default()
    }
}

fn entity(id: &str, stable_key: &str, kind: EntityKind, path: &str, payload: Value) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: StableKey(stable_key.to_string()),
        kind,
        name: id.to_string(),
        title: None,
        source: Some(SourceLocation {
            path: path.to_string(),
            line_start: Some(3),
            line_end: None,
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
        "snap-operations-0001",
        DocumentationProfile::Operations,
        DocumentationGenerationLimits {
            max_entities,
            max_facts: 16,
            max_relations: 16,
            max_diagnostics: 8,
        },
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

fn count_entities(profile: &athanor_app::DocumentationOperationsProfile) -> usize {
    profile
        .context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Entity)
        .count()
}

fn sha256_hex(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}
