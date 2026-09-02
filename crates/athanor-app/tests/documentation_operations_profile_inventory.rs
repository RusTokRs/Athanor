use athanor_app::{
    DOCUMENTATION_REFERENCE_LIMIT, DocumentationContextItemKind, DocumentationGenerationLimits,
    DocumentationGenerationRequest, DocumentationProfile, DocumentationRelationDirection,
    DocumentationValidationStatus, OPERATIONS_DOCUMENT_MEDIA_TYPE, OPERATIONS_DOCUMENT_PATH,
    build_documentation_operations_profile,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, EntityKind,
    Evidence, EvidenceStatus, Fact, FactId, FactKind, Ownership, Relation, RelationId,
    RelationKind, RelationStatus, Severity, SnapshotId, SourceLocation, StableKey,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[test]
fn operations_profile_is_exact_scoped_cited_and_checksum_bound() {
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
        vec![
            "overview",
            "inventory",
            "facts",
            "relationships",
            "diagnostics"
        ]
    );
    assert_eq!(
        count_kind(&profile, DocumentationContextItemKind::Entity),
        10
    );
    assert_eq!(count_kind(&profile, DocumentationContextItemKind::Fact), 3);
    assert_eq!(
        count_kind(&profile, DocumentationContextItemKind::Relation),
        6
    );
    assert_eq!(
        count_kind(&profile, DocumentationContextItemKind::Diagnostic),
        3
    );
    assert_eq!(profile.context.omitted.entities, 0);
    assert_eq!(profile.context.omitted.facts, 0);
    assert_eq!(profile.context.omitted.relations, 0);
    assert_eq!(profile.context.omitted.diagnostics, 0);
    assert_eq!(profile.draft.citations.len(), 22);
    assert_eq!(
        profile.validation_report.status,
        DocumentationValidationStatus::Valid
    );
    assert_eq!(
        profile.validation_report.profile,
        DocumentationProfile::Operations
    );
    assert_eq!(profile.validation_report.metrics.unsupported_relations, 0);
    assert_eq!(profile.document.path, OPERATIONS_DOCUMENT_PATH);
    assert_eq!(profile.document.media_type, OPERATIONS_DOCUMENT_MEDIA_TYPE);
    assert!(profile.document.content.contains("- Profile: `operations`"));
    assert!(profile.document.content.contains("## Operations Inventory"));
    assert!(profile.document.content.contains("## Operations Facts"));
    assert!(
        profile
            .document
            .content
            .contains("## Operations Relationships")
    );
    assert!(
        profile
            .document
            .content
            .contains("## Operations Diagnostics")
    );
    assert!(profile.document.content.contains("```mermaid"));
    assert!(profile.document.content.contains("Slice 4B scope"));
    assert!(!profile.document.content.contains("depends_on_runtime"));
    assert_eq!(
        profile.document.sha256,
        sha256_hex(profile.document.content.as_bytes())
    );
    assert_eq!(
        serde_json::to_value(DocumentationProfile::Operations).unwrap(),
        json!("operations")
    );
    assert!(profile.context.items.len() <= DOCUMENTATION_REFERENCE_LIMIT);

    let env = profile
        .context
        .items
        .iter()
        .find(|item| item.stable_keys == ["env://DATABASE_URL".to_string()])
        .unwrap();
    assert!(env.evidence.iter().any(|location| {
        location.path == "config/runtime.env" && location.start_line == 3 && location.end_line == 3
    }));
}

#[test]
fn operations_profile_keeps_supported_relations_and_only_open_scoped_diagnostics() {
    let profile =
        build_documentation_operations_profile(&request(16), &operations_snapshot()).unwrap();
    let relations = profile
        .context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Relation)
        .collect::<Vec<_>>();

    assert_eq!(relations.len(), 6);
    assert!(relations.iter().all(|item| {
        item.source_stable_key.is_some()
            && item.target_stable_key.is_some()
            && item.relation_direction == Some(DocumentationRelationDirection::Directed)
    }));
    for expected in [
        "defines",
        "contains",
        "documents",
        "documents_operation",
        "uses_env",
        "queries_table",
    ] {
        assert!(relations.iter().any(|item| item.summary.contains(expected)));
    }
    assert!(
        !relations
            .iter()
            .any(|item| item.summary.contains("depends_on_runtime"))
    );

    let diagnostics = profile
        .context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Diagnostic)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 3);
    assert!(
        diagnostics
            .iter()
            .any(|item| item.summary.contains("missing_env_var"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|item| item.summary.contains("broken_script_reference"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|item| item.summary.contains("other:deployment_drift"))
    );
    assert!(
        diagnostics
            .iter()
            .all(|item| !item.summary.contains("resolved env drift"))
    );
    assert!(
        diagnostics
            .iter()
            .all(|item| !item.summary.contains("unrelated package finding"))
    );
}

#[test]
fn operations_profile_discloses_limits_and_ignores_input_order() {
    let limits = DocumentationGenerationLimits {
        max_entities: 16,
        max_facts: 1,
        max_relations: 2,
        max_diagnostics: 1,
    };
    let request = DocumentationGenerationRequest::new(
        "snap-operations-0001",
        DocumentationProfile::Operations,
        limits,
    );
    let snapshot = operations_snapshot();
    let profile = build_documentation_operations_profile(&request, &snapshot).unwrap();

    assert_eq!(
        count_kind(&profile, DocumentationContextItemKind::Entity),
        10
    );
    assert_eq!(count_kind(&profile, DocumentationContextItemKind::Fact), 1);
    assert_eq!(
        count_kind(&profile, DocumentationContextItemKind::Relation),
        2
    );
    assert_eq!(
        count_kind(&profile, DocumentationContextItemKind::Diagnostic),
        1
    );
    assert_eq!(profile.context.omitted.entities, 0);
    assert_eq!(profile.context.omitted.facts, 2);
    assert_eq!(profile.context.omitted.relations, 4);
    assert_eq!(profile.context.omitted.diagnostics, 2);
    assert_eq!(profile.validation_report.metrics.unsupported_relations, 4);
    assert!(
        profile
            .document
            .content
            .contains("Omitted operations scope: entities 0, facts 2, relations 4, diagnostics 2")
    );
    assert!(
        profile
            .document
            .content
            .contains("Unrepresented supported operations relations: 4")
    );

    let mut reversed = snapshot.clone();
    reversed.entities.reverse();
    reversed.facts.reverse();
    reversed.relations.reverse();
    reversed.diagnostics.reverse();
    let repeated = build_documentation_operations_profile(&request, &reversed).unwrap();
    assert_eq!(profile.outline, repeated.outline);
    assert_eq!(profile.context, repeated.context);
    assert_eq!(profile.draft, repeated.draft);
    assert_eq!(profile.validation_report, repeated.validation_report);
    assert_eq!(profile.document, repeated.document);
}

#[test]
fn operations_profile_round_robins_categories_and_excludes_non_operational_entities() {
    let snapshot = operations_snapshot();
    let profile = build_documentation_operations_profile(&request(6), &snapshot).unwrap();
    let entity_items = profile
        .context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Entity)
        .collect::<Vec<_>>();

    assert_eq!(entity_items.len(), 6);
    assert_eq!(profile.context.omitted.entities, 4);
    assert_eq!(
        entity_items
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
    let keys = entity_items
        .iter()
        .flat_map(|item| item.stable_keys.iter().map(String::as_str))
        .collect::<Vec<_>>();
    assert!(!keys.contains(&"package://workspace"));
    assert!(!keys.contains(&"dependency://serde"));
}

#[test]
fn operations_profile_excludes_non_portable_operational_entities() {
    let mut snapshot = operations_snapshot();
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
    let entity_keys = profile
        .context
        .items
        .iter()
        .filter(|item| item.kind == DocumentationContextItemKind::Entity)
        .flat_map(|item| item.stable_keys.iter().map(String::as_str))
        .collect::<Vec<_>>();
    assert!(!entity_keys.contains(&"env://UNSAFE"));
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
    let mut entities = vec![
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
        entity(
            "file-runtime",
            "file://config/runtime.env",
            EntityKind::File,
            "config/runtime.env",
            json!({}),
        ),
        entity(
            "file-deploy",
            "file://scripts/deploy.sh",
            EntityKind::File,
            "scripts/deploy.sh",
            json!({}),
        ),
        entity(
            "doc-runbook",
            "doc://docs/runbooks/deploy.md",
            EntityKind::DocumentationPage,
            "docs/runbooks/deploy.md",
            json!({}),
        ),
        entity(
            "package-workspace",
            "package://workspace",
            EntityKind::Package,
            "Cargo.toml",
            json!({}),
        ),
        entity(
            "dependency-serde",
            "dependency://serde",
            EntityKind::Dependency,
            "Cargo.toml",
            json!({}),
        ),
    ];
    entities.sort_by(|left, right| left.id.0.cmp(&right.id.0));

    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap-operations-0001".to_string())),
        entities,
        facts: vec![
            fact(
                "fact-env",
                FactKind::EnvVarUsed,
                "env-database",
                Some("file-runtime"),
                "config/runtime.env",
            ),
            fact(
                "fact-script",
                FactKind::SymbolDefined,
                "script-deploy",
                Some("file-deploy"),
                "scripts/deploy.sh",
            ),
            fact(
                "fact-migration",
                FactKind::MigrationCreatesTable,
                "migration-users",
                Some("table-users"),
                "migrations/001_users.sql",
            ),
            fact(
                "fact-unrelated",
                FactKind::SymbolDefined,
                "package-workspace",
                Some("dependency-serde"),
                "Cargo.toml",
            ),
        ],
        relations: vec![
            relation(
                "rel-defines",
                RelationKind::Defines,
                "file-deploy",
                "script-deploy",
                "scripts/deploy.sh",
            ),
            relation(
                "rel-contains",
                RelationKind::Contains,
                "service-api",
                "command-migrate",
                "deploy/docker-compose.yml",
            ),
            relation(
                "rel-documents",
                RelationKind::Documents,
                "doc-runbook",
                "runbook-deploy",
                "docs/runbooks/deploy.md",
            ),
            relation(
                "rel-documents-operation",
                RelationKind::DocumentsOperation,
                "doc-runbook",
                "runbook-step-rollback",
                "docs/runbooks/deploy.md",
            ),
            relation(
                "rel-uses-env",
                RelationKind::UsesEnv,
                "service-api",
                "env-database",
                "deploy/docker-compose.yml",
            ),
            relation(
                "rel-queries-table",
                RelationKind::QueriesTable,
                "script-deploy",
                "table-users",
                "scripts/deploy.sh",
            ),
            relation(
                "rel-unsupported",
                RelationKind::Other("depends_on_runtime".to_string()),
                "service-api",
                "config-port",
                "deploy/docker-compose.yml",
            ),
        ],
        diagnostics: vec![
            diagnostic(
                "diag-env",
                DiagnosticKind::MissingEnvVar,
                DiagnosticStatus::Open,
                Severity::High,
                "missing database env",
                &["env-database"],
                "config/runtime.env",
            ),
            diagnostic(
                "diag-script",
                DiagnosticKind::BrokenScriptReference,
                DiagnosticStatus::Open,
                Severity::Medium,
                "broken deploy reference",
                &["script-deploy"],
                "scripts/deploy.sh",
            ),
            diagnostic(
                "diag-deploy",
                DiagnosticKind::Other("deployment_drift".to_string()),
                DiagnosticStatus::Open,
                Severity::Critical,
                "deployment drift",
                &["service-api"],
                "deploy/docker-compose.yml",
            ),
            diagnostic(
                "diag-resolved",
                DiagnosticKind::MissingEnvVar,
                DiagnosticStatus::Resolved,
                Severity::High,
                "resolved env drift",
                &["env-database"],
                "config/runtime.env",
            ),
            diagnostic(
                "diag-unrelated",
                DiagnosticKind::Other("package_drift".to_string()),
                DiagnosticStatus::Open,
                Severity::Low,
                "unrelated package finding",
                &["package-workspace"],
                "Cargo.toml",
            ),
        ],
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

fn fact(id: &str, kind: FactKind, subject: &str, object: Option<&str>, path: &str) -> Fact {
    Fact {
        id: FactId(id.to_string()),
        kind,
        subject: EntityId(subject.to_string()),
        object: object.map(|id| EntityId(id.to_string())),
        value: json!({}),
        evidence: vec![evidence(path, 5)],
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        snapshot: SnapshotId("snap-operations-0001".to_string()),
        extractor: "operations-test".to_string(),
        confidence: 1.0,
    }
}

fn relation(id: &str, kind: RelationKind, from: &str, to: &str, path: &str) -> Relation {
    Relation {
        id: RelationId(id.to_string()),
        kind,
        from: EntityId(from.to_string()),
        to: EntityId(to.to_string()),
        status: RelationStatus::Verified,
        confidence: 1.0,
        evidence: vec![evidence(path, 7)],
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        snapshot: SnapshotId("snap-operations-0001".to_string()),
        payload: json!({}),
    }
}

fn diagnostic(
    id: &str,
    kind: DiagnosticKind,
    status: DiagnosticStatus,
    severity: Severity,
    title: &str,
    entities: &[&str],
    path: &str,
) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId(id.to_string()),
        kind,
        severity,
        status,
        title: title.to_string(),
        message: title.to_string(),
        entities: entities
            .iter()
            .map(|id| EntityId((*id).to_string()))
            .collect(),
        evidence: vec![evidence(path, 9)],
        ownership: vec![Ownership {
            source_file: path.to_string(),
        }],
        snapshot: SnapshotId("snap-operations-0001".to_string()),
        suggested_fix: None,
        payload: json!({}),
    }
}

fn evidence(path: &str, line: u32) -> Evidence {
    Evidence {
        source_file: Some(path.to_string()),
        line_start: Some(line),
        line_end: Some(line),
        extractor: Some("operations-test".to_string()),
        commit_hash: None,
        confidence: 1.0,
        status: EvidenceStatus::Verified,
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

fn count_kind(
    profile: &athanor_app::DocumentationOperationsProfile,
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
