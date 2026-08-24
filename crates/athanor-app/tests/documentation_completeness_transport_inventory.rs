use athanor_app::{
    DOCUMENTATION_COMPLETENESS_BASELINE_ADAPTER, DOCUMENTATION_COMPLETENESS_SCHEMA_V1,
    DocumentationCompletenessOmitted, DocumentationCompletenessReport,
    DocumentationCompletenessTotals, VersionedDocumentationCompletenessReport,
    VersionedJsonContract,
};
use serde_json::Value;

#[test]
fn completeness_transport_matches_registered_golden_contract() {
    let report = DocumentationCompletenessReport {
        snapshot: "snap_fixture".to_string(),
        baseline_adapter: DOCUMENTATION_COMPLETENESS_BASELINE_ADAPTER,
        limit: 50,
        totals: DocumentationCompletenessTotals::default(),
        languages: Vec::new(),
        adapters: Vec::new(),
        unprocessed_files: Vec::new(),
        omitted: DocumentationCompletenessOmitted::default(),
    };
    let transport = VersionedDocumentationCompletenessReport::from(&report);

    assert_eq!(transport.schema, DOCUMENTATION_COMPLETENESS_SCHEMA_V1);
    transport
        .validate_contract()
        .expect("completeness transport contract must validate");

    let actual = serde_json::to_value(&transport).expect("serialize completeness transport");
    let expected: Value = serde_json::from_str(include_str!(
        "fixtures/documentation_completeness.v1.json"
    ))
    .expect("golden completeness JSON must parse");
    assert_eq!(actual, expected);
}
