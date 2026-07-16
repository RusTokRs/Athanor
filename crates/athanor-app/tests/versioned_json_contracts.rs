use std::path::PathBuf;

use athanor_app::{
    AFFECTED_CHECK_SCHEMA_V1, AffectedCheckReport, AffectedFileCounts, ApiOverview,
    CAPABILITIES_SCHEMA_V1, CHANGE_MAP_SCHEMA_V1, COVERAGE_SCHEMA_V1, CapabilitiesLimits,
    CapabilitiesOmitted, CapabilitiesReport, CapabilitiesTotals, ChangeMapCompleteness,
    ChangeMapCounts, ChangeMapLimits, ChangeMapQuery, ChangeMapReport, CoverageFilters,
    CoverageLimits, CoverageOmitted, CoverageReport, CoverageTotals, DIAGNOSTIC_CHECK_SCHEMA_V1,
    DiagnosticCheckReport, DiagnosticCounts, DiagnosticScope, DocsOverview,
    ENTITY_EXPLANATION_SCHEMA_V1, GRAPH_CYCLES_SCHEMA_V1, GRAPH_EXPORT_SCHEMA_V1,
    GRAPH_HUBS_SCHEMA_V1, GRAPH_PAGERANK_SCHEMA_V1, GRAPH_PATH_SCHEMA_V1,
    GRAPH_RELATED_SCHEMA_V1, GraphCycles, GraphExport, GraphHubs, GraphNode, GraphOmitted,
    GraphPageRank, GraphPath, GraphRelated, GraphRelatedNode, IMPACT_ANALYSIS_SCHEMA_V1,
    OPERATIONS_DOCS_CHECK_SCHEMA_V1, OperationsDocsCheckReport, OperationsOverview,
    OVERVIEW_SCHEMA_V1, OverviewTotals, PROJECT_RESOLUTION_SCHEMA_V1, ProjectRegistration,
    ProjectResolutionReport, RepositoryOverview, SEARCH_SCHEMA_V1, SearchOmissions, SearchReport,
    VersionedJsonContract, explain_snapshot, impact_snapshot,
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

fn assert_matches_value<T: VersionedJsonContract>(document: &T, expected: &Value) {
    document
        .validate_contract()
        .expect("fixture document must satisfy its registered JSON contract");
    let actual = serde_json::to_value(document).expect("fixture document must serialize");
    assert_eq!(&actual, expected);
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

#[test]
fn coverage_v1_matches_golden_fixture() {
    let report = CoverageReport {
        schema: COVERAGE_SCHEMA_V1,
        snapshot: "snap_fixture".to_string(),
        root: PathBuf::from("repo"),
        filters: CoverageFilters {
            adapter: None,
            file: None,
        },
        limits: CoverageLimits { limit: 50 },
        totals: CoverageTotals::default(),
        files: Vec::new(),
        adapters: Vec::new(),
        diagnostics: Vec::new(),
        omitted: CoverageOmitted::default(),
    };

    assert_matches_fixture(&report, include_str!("fixtures/coverage.v1.json"));
}

#[test]
fn capabilities_v1_matches_golden_fixture() {
    let report = CapabilitiesReport {
        schema: CAPABILITIES_SCHEMA_V1,
        snapshot: "snap_fixture".to_string(),
        root: PathBuf::from("repo"),
        baseline_adapter: "file",
        limits: CapabilitiesLimits {
            limit: 50,
            confidence_threshold: 1.0,
        },
        totals: CapabilitiesTotals::default(),
        languages: Vec::new(),
        adapters: Vec::new(),
        low_confidence_facts: Vec::new(),
        unprocessed_files: Vec::new(),
        omitted: CapabilitiesOmitted::default(),
    };

    assert_matches_fixture(&report, include_str!("fixtures/capabilities.v1.json"));
}

#[test]
fn second_wave_contracts_match_golden_fixture() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/json_contract_second_wave.v1.json"
    ))
    .expect("second-wave golden fixture must be valid JSON");

    let change_map = ChangeMapReport {
        schema: CHANGE_MAP_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        query: ChangeMapQuery {
            task: Some("contract".to_string()),
            target: None,
            diff: false,
            changed_files: Vec::new(),
        },
        limits: ChangeMapLimits {
            max_entities: 10,
            max_files: 10,
            max_diagnostics: 10,
            max_depth: 2,
        },
        returned: ChangeMapCounts::default(),
        omitted: ChangeMapCounts::default(),
        items: Vec::new(),
        files: Vec::new(),
        diagnostics: Vec::new(),
        completeness: ChangeMapCompleteness {
            candidate_limit_reached: false,
            candidate_limit: 5_000,
            note: "fixture".to_string(),
            suggested_command: "ath coverage --json".to_string(),
        },
    };
    assert_matches_value(&change_map, &fixture["change_map"]);

    let export = GraphExport {
        schema: GRAPH_EXPORT_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        nodes: Vec::new(),
        edges: Vec::new(),
        omitted: GraphOmitted {
            nodes: 0,
            edges: 0,
            reason: "limit".to_string(),
        },
    };
    assert_matches_value(&export, &fixture["graph_export"]);

    let related = GraphRelated {
        schema: GRAPH_RELATED_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        root: GraphRelatedNode {
            entity: graph_node("ent_root", "module://root", "root"),
            distance: 0,
        },
        nodes: Vec::new(),
        edges: Vec::new(),
        truncated: false,
    };
    assert_matches_value(&related, &fixture["graph_related"]);

    let path = GraphPath {
        schema: GRAPH_PATH_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        from: graph_node("ent_from", "module://from", "from"),
        to: graph_node("ent_to", "module://to", "to"),
        found: false,
        hops: None,
        nodes: Vec::new(),
        edges: Vec::new(),
        visited: 2,
        truncated: false,
    };
    assert_matches_value(&path, &fixture["graph_path"]);

    let hubs = GraphHubs {
        schema: GRAPH_HUBS_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        kind: None,
        hubs: Vec::new(),
        omitted: 0,
    };
    assert_matches_value(&hubs, &fixture["graph_hubs"]);

    let pagerank = GraphPageRank {
        schema: GRAPH_PAGERANK_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        kind: None,
        damping: 0.85,
        iterations: 0,
        converged: true,
        entity_count: 0,
        relation_count: 0,
        ranks: Vec::new(),
        omitted: 0,
    };
    assert_matches_value(&pagerank, &fixture["graph_pagerank"]);

    let cycles = GraphCycles {
        schema: GRAPH_CYCLES_SCHEMA_V1.to_string(),
        snapshot: "snap_fixture".to_string(),
        cycles: Vec::new(),
        start_entities: 0,
        omitted_start_entities: 0,
        truncated: false,
    };
    assert_matches_value(&cycles, &fixture["graph_cycles"]);

    let resolution = ProjectResolutionReport {
        schema: PROJECT_RESOLUTION_SCHEMA_V1.to_string(),
        registry_path: PathBuf::from("registry.json"),
        project: ProjectRegistration {
            project_id: "fixture".to_string(),
            root: PathBuf::from("project"),
        },
    };
    assert_matches_value(&resolution, &fixture["project_resolution"]);
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

fn graph_node(id: &str, stable_key: &str, name: &str) -> GraphNode {
    GraphNode {
        id: id.to_string(),
        stable_key: stable_key.to_string(),
        kind: "module".to_string(),
        name: name.to_string(),
        source: None,
        degree: 0,
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
