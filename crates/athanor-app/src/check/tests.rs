use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use athanor_domain::{
    Diagnostic, DiagnosticId, DiagnosticKind, DiagnosticStatus, Entity, EntityId, Severity,
    SnapshotId,
};
use serde_json::json;

use super::affected::{
    affected_documentation_drift, affected_entity_ids, diagnostic_touches_paths_or_entities,
    inspect_snapshot_manifest, repair_issue_to_artifact_status,
};
use super::diagnostics::build_check_report;
use super::model::{AffectedArtifactKind, AffectedArtifactStatus, DiagnosticScope};
use crate::config::{ApiConfig, ApiSourceOfTruth};
use crate::json_contract::DIAGNOSTIC_CHECK_SCHEMA_V1;
use crate::repair::RepairIssue;

#[test]
fn filters_open_api_diagnostics_and_counts_severity() {
    let diagnostics = vec![
        diagnostic(
            "diag_api_high",
            DiagnosticKind::ApiRequestSchemaMismatch,
            Severity::High,
            DiagnosticStatus::Open,
        ),
        diagnostic(
            "diag_api_medium",
            DiagnosticKind::ApiEndpointImplementedButNotDocumented,
            Severity::Medium,
            DiagnosticStatus::Open,
        ),
        diagnostic(
            "diag_docs",
            DiagnosticKind::DocumentationPageMissingTitle,
            Severity::Medium,
            DiagnosticStatus::Open,
        ),
        diagnostic(
            "diag_resolved",
            DiagnosticKind::ApiPathMismatch,
            Severity::Critical,
            DiagnosticStatus::Resolved,
        ),
    ];

    let report = build_check_report(
        "snap_test".to_string(),
        DiagnosticScope::Api,
        &diagnostics,
        &ApiConfig::default(),
    );

    assert_eq!(report.schema, DIAGNOSTIC_CHECK_SCHEMA_V1);
    assert_eq!(report.counts.total, 2);
    assert_eq!(report.counts.high, 1);
    assert_eq!(report.counts.medium, 1);
    assert_eq!(report.diagnostics[0].id.0, "diag_api_high");
}

#[test]
fn reports_stale_snapshot_manifests_for_affected_artifacts() {
    let root = test_root("affected-artifacts");
    write_json(
        &root.join(".athanor/generated/current/wiki/manifest.json"),
        r#"{"schema":"athanor.wiki_manifest.v1","snapshot":"snap_old"}"#,
    );
    write_json(
        &root.join(".athanor/generated/current/html/manifest.json"),
        r#"{"schema":"athanor.html_report_manifest.v1","snapshot":"snap_old"}"#,
    );
    write_json(
        &root.join(".athanor/api/latest.json"),
        r#"{"schema":"athanor.api_contract_latest.v1","snapshot":"snap_old"}"#,
    );
    fs::create_dir_all(root.join(".athanor/api/diffs")).unwrap();

    let mut statuses = Vec::new();
    let wiki_stale = inspect_snapshot_manifest(
        &root,
        ".athanor/generated/current/wiki/manifest.json",
        "snap_new",
        AffectedArtifactKind::Wiki,
        "wiki",
        &mut statuses,
    )
    .unwrap();
    let html_stale = inspect_snapshot_manifest(
        &root,
        ".athanor/generated/current/html/manifest.json",
        "snap_new",
        AffectedArtifactKind::HtmlReport,
        "html",
        &mut statuses,
    )
    .unwrap();
    let api_stale = inspect_snapshot_manifest(
        &root,
        ".athanor/api/latest.json",
        "snap_new",
        AffectedArtifactKind::ApiContract,
        "api snapshot",
        &mut statuses,
    )
    .unwrap();
    if api_stale && root.join(".athanor/api/diffs").is_dir() {
        statuses.push(AffectedArtifactStatus {
            kind: AffectedArtifactKind::ApiDiff,
            path: PathBuf::from(".athanor/api/diffs"),
            message: "API diff artifacts may not include the latest API contract snapshot"
                .to_string(),
            suggested_command: "api diff".to_string(),
        });
    }

    assert!(wiki_stale && html_stale && api_stale);
    assert_eq!(
        statuses
            .iter()
            .map(|status| status.kind)
            .collect::<Vec<_>>(),
        vec![
            AffectedArtifactKind::Wiki,
            AffectedArtifactKind::HtmlReport,
            AffectedArtifactKind::ApiContract,
            AffectedArtifactKind::ApiDiff,
        ]
    );
    assert!(statuses[0].message.contains("snap_old"));
    assert!(statuses[0].message.contains("snap_new"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn maps_repair_issues_to_affected_artifact_statuses() {
    let generated = repair_issue_to_artifact_status(&RepairIssue {
        code: "stale_current_generation_snapshot".to_string(),
        path: PathBuf::from(".athanor/generated/current.json"),
        message: "generated is stale".to_string(),
    })
    .unwrap();
    let orphan = repair_issue_to_artifact_status(&RepairIssue {
        code: "orphan_generated_generation".to_string(),
        path: PathBuf::from(".athanor/generated/generations/00000001"),
        message: "orphan generation".to_string(),
    })
    .unwrap();
    let unrelated = repair_issue_to_artifact_status(&RepairIssue {
        code: "missing_canonical_latest".to_string(),
        path: PathBuf::from(".athanor/store/canonical/jsonl/latest.json"),
        message: "canonical latest missing".to_string(),
    });

    assert_eq!(generated.kind, AffectedArtifactKind::GeneratedCurrent);
    assert_eq!(orphan.kind, AffectedArtifactKind::GeneratedGeneration);
    assert!(generated.suggested_command.contains("repair regenerate"));
    assert!(orphan.suggested_command.contains("repair cleanup"));
    assert!(unrelated.is_none());
}

#[test]
fn filters_documentation_and_environment_diagnostics() {
    let mut env_docs = diagnostic(
        "diag_config",
        DiagnosticKind::MissingDocumentation,
        Severity::Medium,
        DiagnosticStatus::Open,
    );
    env_docs.payload = json!({"scope": "env"});
    let mut script = diagnostic(
        "diag_script",
        DiagnosticKind::MissingDocumentation,
        Severity::Medium,
        DiagnosticStatus::Open,
    );
    script.payload = json!({"scope": "scripts"});
    let diagnostics = vec![
        diagnostic(
            "diag_docs",
            DiagnosticKind::EmptyDocumentationPage,
            Severity::High,
            DiagnosticStatus::Open,
        ),
        diagnostic(
            "diag_docs_reference",
            DiagnosticKind::DocumentationReferenceUnresolved,
            Severity::Medium,
            DiagnosticStatus::Open,
        ),
        diagnostic(
            "diag_env",
            DiagnosticKind::MissingEnvVar,
            Severity::Medium,
            DiagnosticStatus::Open,
        ),
        env_docs,
        script,
    ];

    let docs = build_check_report(
        "snap_test".to_string(),
        DiagnosticScope::Docs,
        &diagnostics,
        &ApiConfig::default(),
    );
    let env = build_check_report(
        "snap_test".to_string(),
        DiagnosticScope::Env,
        &diagnostics,
        &ApiConfig::default(),
    );

    assert_eq!(docs.counts.total, 2);
    assert_eq!(docs.diagnostics[0].id.0, "diag_docs");
    assert_eq!(
        env.diagnostics
            .iter()
            .map(|diagnostic| diagnostic.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["diag_config", "diag_env"]
    );
}

#[test]
fn filters_affected_diagnostics_by_changed_files_and_entities() {
    let changed = entity("ent_changed", "docs/changed.md");
    let unrelated = entity("ent_unrelated", "docs/unrelated.md");
    let affected_paths = BTreeSet::from(["docs/changed.md".to_string()]);
    let affected_entity_ids =
        affected_entity_ids(&[changed.clone(), unrelated.clone()], &affected_paths);

    let mut attached = diagnostic(
        "diag_attached",
        DiagnosticKind::MissingDocumentation,
        Severity::Medium,
        DiagnosticStatus::Open,
    );
    attached.entities = vec![changed.id.clone()];
    let mut evidence = diagnostic(
        "diag_evidence",
        DiagnosticKind::MissingDocumentation,
        Severity::Medium,
        DiagnosticStatus::Open,
    );
    evidence.evidence = vec![athanor_domain::Evidence {
        source_file: Some("docs/changed.md".to_string()),
        line_start: Some(1),
        line_end: Some(1),
        extractor: Some("test".to_string()),
        commit_hash: None,
        confidence: 1.0,
        status: athanor_domain::EvidenceStatus::Missing,
    }];
    let mut other = diagnostic(
        "diag_other",
        DiagnosticKind::MissingDocumentation,
        Severity::Medium,
        DiagnosticStatus::Open,
    );
    other.entities = vec![unrelated.id.clone()];

    assert!(diagnostic_touches_paths_or_entities(
        &attached,
        &affected_paths,
        &affected_entity_ids
    ));
    assert!(diagnostic_touches_paths_or_entities(
        &evidence,
        &affected_paths,
        &affected_entity_ids
    ));
    assert!(!diagnostic_touches_paths_or_entities(
        &other,
        &affected_paths,
        &affected_entity_ids
    ));
}

#[test]
fn reports_documentation_drift_only_for_affected_documents() {
    let changed = editable_doc("ent_changed_doc", "docs/changed.md", Some("snap_old"));
    let unchanged = editable_doc("ent_unchanged_doc", "docs/unchanged.md", Some("snap_old"));
    let current = editable_doc("ent_current_doc", "docs/current.md", Some("snap_current"));
    let affected_paths =
        BTreeSet::from(["docs/changed.md".to_string(), "docs/current.md".to_string()]);

    let drift = affected_documentation_drift(
        "snap_current".to_string(),
        &[changed, unchanged, current],
        &crate::config::DocsConfig::default(),
        &affected_paths,
    );

    assert_eq!(drift.len(), 1);
    assert_eq!(drift[0].path, "docs/changed.md");
    assert_eq!(drift[0].verified_snapshot.as_deref(), Some("snap_old"));
}

#[test]
fn payload_scopes_select_only_their_operations_diagnostics() {
    for (scope, value) in [
        (DiagnosticScope::Scripts, "scripts"),
        (DiagnosticScope::Deployment, "deployment"),
        (DiagnosticScope::Runbooks, "runbooks"),
    ] {
        let kind = if scope == DiagnosticScope::Runbooks {
            DiagnosticKind::StaleDocumentation
        } else {
            DiagnosticKind::MissingDocumentation
        };
        let mut scoped = diagnostic(
            "diag_scoped",
            kind.clone(),
            Severity::Medium,
            DiagnosticStatus::Open,
        );
        scoped.payload = json!({"scope": value});
        let unscoped = diagnostic(
            "diag_unscoped",
            kind,
            Severity::Medium,
            DiagnosticStatus::Open,
        );

        let report = build_check_report(
            "snap_test".to_string(),
            scope,
            &[unscoped, scoped],
            &ApiConfig::default(),
        );
        assert_eq!(report.counts.total, 1);
        assert_eq!(report.diagnostics[0].id.0, "diag_scoped");
    }
}

#[test]
fn api_policy_gates_filter_configured_diagnostics() {
    let missing_docs = diagnostic(
        "diag_missing_docs",
        DiagnosticKind::ApiEndpointImplementedButNotDocumented,
        Severity::Medium,
        DiagnosticStatus::Open,
    );
    let missing_impl = diagnostic(
        "diag_missing_impl",
        DiagnosticKind::ApiEndpointDocumentedButNotImplemented,
        Severity::High,
        DiagnosticStatus::Open,
    );

    let mut openapi = ApiConfig {
        source_of_truth: ApiSourceOfTruth::OpenapiFirst,
        fail_on_missing_docs: false,
        ..Default::default()
    };
    let report = build_check_report(
        "snap_test".to_string(),
        DiagnosticScope::Api,
        &[missing_docs.clone(), missing_impl.clone()],
        &openapi,
    );
    assert_eq!(report.counts.total, 1);
    assert_eq!(report.diagnostics[0].id.0, "diag_missing_impl");

    openapi.fail_on_missing_docs = true;
    assert_eq!(
        build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &[missing_docs, missing_impl.clone()],
            &openapi,
        )
        .counts
        .total,
        2
    );

    let code_first = ApiConfig {
        source_of_truth: ApiSourceOfTruth::CodeFirst,
        ..Default::default()
    };
    assert_eq!(
        build_check_report(
            "snap_test".to_string(),
            DiagnosticScope::Api,
            &[missing_impl],
            &code_first,
        )
        .counts
        .total,
        0
    );

    for (kind, config) in [
        (
            DiagnosticKind::ApiRequestSchemaMismatch,
            ApiConfig {
                fail_on_openapi_mismatch: false,
                ..Default::default()
            },
        ),
        (
            DiagnosticKind::ApiStatusCodeUndocumented,
            ApiConfig {
                fail_on_undocumented_status_code: false,
                ..Default::default()
            },
        ),
    ] {
        assert_eq!(
            build_check_report(
                "snap_test".to_string(),
                DiagnosticScope::Api,
                &[diagnostic(
                    "diag_gated",
                    kind,
                    Severity::High,
                    DiagnosticStatus::Open,
                )],
                &config,
            )
            .counts
            .total,
            0
        );
    }
}

fn diagnostic(
    id: &str,
    kind: DiagnosticKind,
    severity: Severity,
    status: DiagnosticStatus,
) -> Diagnostic {
    Diagnostic {
        id: DiagnosticId(id.to_string()),
        kind,
        severity,
        status,
        title: id.to_string(),
        message: id.to_string(),
        entities: vec![EntityId("ent_test".to_string())],
        evidence: Vec::new(),
        ownership: Vec::new(),
        snapshot: SnapshotId("snap_test".to_string()),
        suggested_fix: None,
        payload: json!({}),
    }
}

fn entity(id: &str, path: &str) -> Entity {
    Entity {
        id: EntityId(id.to_string()),
        stable_key: athanor_domain::StableKey(format!("doc://{path}")),
        kind: athanor_domain::EntityKind::DocumentationPage,
        name: path.to_string(),
        title: None,
        source: Some(athanor_domain::SourceLocation {
            path: path.to_string(),
            line_start: Some(1),
            line_end: Some(1),
        }),
        language: None,
        aliases: Vec::new(),
        ownership: vec![athanor_domain::Ownership {
            source_file: path.to_string(),
        }],
        payload: json!({}),
    }
}

fn editable_doc(id: &str, path: &str, verified_snapshot: Option<&str>) -> Entity {
    let mut entity = entity(id, path);
    entity.payload = json!({
        "documentation_layer": "editable",
        "last_verified_snapshot": verified_snapshot,
    });
    entity
}

fn write_json(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn test_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = env::temp_dir().join(format!("athanor-{name}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    root
}
