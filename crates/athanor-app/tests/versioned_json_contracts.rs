use std::path::PathBuf;

use athanor_app::{
    ApiOverview, DocsOverview, OperationsOverview, OVERVIEW_SCHEMA_V1, OverviewTotals,
    RepositoryOverview, SEARCH_SCHEMA_V1, SearchOmissions, SearchReport, VersionedJsonContract,
};
use serde_json::Value;

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
