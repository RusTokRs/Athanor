use std::path::PathBuf;

use athanor_app::{
    AFFECTED_CHECK_SCHEMA_V1, AffectedCheckReport, AffectedFileCounts, ApiOverview,
    DIAGNOSTIC_CHECK_SCHEMA_V1, DiagnosticCheckReport, DiagnosticCounts, DiagnosticScope,
    DocsOverview, ENTITY_EXPLANATION_SCHEMA_V1, IMPACT_ANALYSIS_SCHEMA_V1,
    OPERATIONS_DOCS_CHECK_SCHEMA_V1, OperationsDocsCheckReport, OperationsOverview,
    OVERVIEW_SCHEMA_V1, OverviewTotals, RepositoryOverview, SEARCH_SCHEMA_V1, SearchOmissions,
    SearchReport, VersionedJsonContract, explain_snapshot, impact_snapshot,
};
use athanor_core::CanonicalSnapshot;
use athanor_domain::{
    Entity, EntityId, EntityKind, Ownership, SnapshotId, SourceLocation, StableKey,
};
use serde_json::{Value, json};

fn assert_matches_fixture<T: VersionedJsonContract>(document: &T, fixture: &str) {
    document
        .validate_contract()
        .expect("fixture document must satisfy its registered JSON contract");
    let actual = serde_json::to_value(document).expect("fixture document must serialize");
    let expected: Value = serde_json::from_str(fixture).expect("golden fixture must be valid JSON");
    assert_eq!(actual, expected);
}

#[test]
fn repository_overview_v1_matches_golden_fixture() {
    let report = RepositoryOverview {
        schema: OVERVIEW_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        totals: OverviewTotals::default(),
        entity_kinds: Vec::new(),
        relation_kinds: Vec::new(),
        source_roots: Vec::new(),
        api: ApiOverview::default(),
        docs: DocsOverview::default(),
        operations: OperationsOverview::default(),
        module_structure: Vec::new(),
        integration_boundaries: Vec::new(),
        graph_hubs: Vec::new(),
        open_diagnostics: Vec::new(),
    };

    assert_matches_fixture(&report, include_str!("fixtures/overview.v1.json"));
}

#[test]
fn search_report_v1_matches_golden_fixture() {
    let report = SearchReport {
        schema: SEARCH_SCHEMA_V1.to_string(),
        root: PathBuf::from("repo"),
        snapshot: "snap_fixture".to_string(),
        query: "contract".to_string(),
        limit: 5,
        returned: 0,
        truncated: false,
        omitted: SearchOmissions {
            results_lower_bound: 0,
            reason: None,
        },
        results: Vec::new(),
    };

    assert_matches_fixture(&report, include_str!("fixtures/search.v1.json"));
}

#[test]
fn entity_explanation_v1_matches_golden_fixture() {
    let snapshot = contract_snapshot();
    let report = explain_snapshot(&snapshot, "api://GET:/contract")
        .expect("fixture entity must be explainable");

    assert_eq!(report.schema, ENTITY_EXPLANATION_SCHEMA_V1);
    assert_matches_fixture(
        &report,
        include_str!("fixtures/entity_explanation.v1.json"),
    );
}

#[test]
fn impact_analysis_v1_matches_golden_fixture() {
    let snapshot = contract_snapshot();
    let report = impact_snapshot(&snapshot, snapshot.entities.clone(), 1);

    assert_eq!(report.schema, IMPACT_ANALYSIS_SCHEMA_V1);
    assert_matches_fixture(&report, include_str!("fixtures/impact_analysis.v1.json"));
}

#[test]
fn diagnostic_check_v1_matches_golden_fixture() {
    let report = diagnostic_report(DiagnosticScope::Api);

    assert_matches_fixture(&report, include_str!("fixtures/diagnostic_check.v1.json"));
}

#[test]
fn affected_check_v1_matches_golden_fixture() {
    let report = AffectedCheckReport {
        schema: AFFECTED_CHECK_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        affected_files: AffectedFileCounts {
            changed: 1,
            unchanged: 2,
            removed: 0,
        },
        stale_artifacts: Vec::new(),
        documentation_drift: Vec::new(),
        counts: DiagnosticCounts::default(),
        diagnostics: Vec::new(),
    };

    assert_matches_fixture(&report, include_str!("fixtures/affected_check.v1.json"));
}

#[test]
fn operations_docs_check_v1_matches_golden_fixture() {
    let report = OperationsDocsCheckReport {
        schema: OPERATIONS_DOCS_CHECK_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        counts: DiagnosticCounts::default(),
        env: diagnostic_report(DiagnosticScope::Env),
        scripts: diagnostic_report(DiagnosticScope::Scripts),
        deployment: diagnostic_report(DiagnosticScope::Deployment),
        runbooks: diagnostic_report(DiagnosticScope::Runbooks),
    };

    assert_matches_fixture(
        &report,
        include_str!("fixtures/operations_docs_check.v1.json"),
    );
}

fn diagnostic_report(scope: DiagnosticScope) -> DiagnosticCheckReport {
    DiagnosticCheckReport {
        schema: DIAGNOSTIC_CHECK_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        scope,
        counts: DiagnosticCounts::default(),
        diagnostics: Vec::new(),
    }
}

fn contract_snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot: Some(SnapshotId("snap_fixture".to_string())),
        entities: vec![Entity {
            id: EntityId("ent_contract".to_string()),
            stable_key: StableKey("api://GET:/contract".to_string()),
            kind: EntityKind::ApiEndpoint,
            name: "GET /contract".to_string(),
            title: Some("Contract endpoint".to_string()),
            source: Some(SourceLocation {
                path: "openapi.yaml".to_string(),
                line_start: Some(10),
                line_end: Some(20),
            }),
            language: None,
            aliases: vec!["contract".to_string()],
            ownership: vec![Ownership {
                source_file: "openapi.yaml".to_string(),
            }],
            payload: json!({ "method": "GET" }),
        }],
        ..CanonicalSnapshot::default()
    }
}
