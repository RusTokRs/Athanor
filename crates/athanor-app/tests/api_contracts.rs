use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    API_CLEANUP_SCHEMA_V1, API_CONTRACT_DIFF_SCHEMA_V2, API_CONTRACT_LATEST_SCHEMA,
    API_CONTRACT_SNAPSHOT_SCHEMA, ApiCleanupArtifact, ApiCleanupArtifactKind, ApiCleanupReport,
    ApiContractChange, ApiContractChangeKind, ApiContractDiff, ApiContractItem, ApiContractLatest,
    ApiContractSnapshot, VersionedJsonContract, validate_contract_value,
};
use serde_json::{Value, json};

#[test]
fn public_and_generated_api_contracts_match_golden_fixture() {
    let snapshot = ApiContractSnapshot {
        schema: API_CONTRACT_SNAPSHOT_SCHEMA.to_string(),
        snapshot: "snap-api".to_string(),
        endpoints: vec![ApiContractItem {
            entity_id: None,
            stable_key: "api://GET:/catalog".to_string(),
            name: "List catalog".to_string(),
            source: None,
            ownership: Vec::new(),
            payload: json!({
                "method": "GET",
                "path": "/catalog"
            }),
        }],
        schemas: Vec::new(),
        examples: Vec::new(),
    };
    let latest = ApiContractLatest {
        schema: API_CONTRACT_LATEST_SCHEMA.to_string(),
        snapshot: "snap-api".to_string(),
        path: "snapshots/snap-api.json".to_string(),
    };
    let diff = ApiContractDiff {
        schema: API_CONTRACT_DIFF_SCHEMA_V2.to_string(),
        from: "snap-api-old".to_string(),
        to: "snap-api".to_string(),
        breaking_changes: 1,
        changes: vec![ApiContractChange {
            kind: ApiContractChangeKind::EndpointRemoved,
            stable_key: "api://DELETE:/catalog/{id}".to_string(),
            breaking: true,
            reasons: vec!["contract_item_removed".to_string()],
            entity_id: None,
            source: None,
            ownership: Vec::new(),
            before: Some(json!({
                "method": "DELETE",
                "path": "/catalog/{id}"
            })),
            after: None,
        }],
        diagnostics: Vec::new(),
        artifact: Some("diffs/snap-api-old--snap-api.json".to_string()),
        cleanup: None,
    };
    let cleanup = ApiCleanupReport {
        schema: API_CLEANUP_SCHEMA_V1.to_string(),
        root: PathBuf::from("project"),
        dry_run: true,
        keep_snapshots: 2,
        keep_diffs: 1,
        removed: vec![ApiCleanupArtifact {
            kind: ApiCleanupArtifactKind::Snapshot,
            id: "snap-api-old".to_string(),
            path: PathBuf::from("project/.athanor/api/snapshots/snap-api-old.json"),
        }],
        retained: vec![ApiCleanupArtifact {
            kind: ApiCleanupArtifactKind::Diff,
            id: "snap-api-old--snap-api".to_string(),
            path: PathBuf::from("project/.athanor/api/diffs/snap-api-old--snap-api.json"),
        }],
    };

    diff.validate_contract().expect("valid API diff contract");
    cleanup
        .validate_contract()
        .expect("valid API cleanup contract");

    let snapshot_value = serde_json::to_value(snapshot).unwrap();
    let latest_value = serde_json::to_value(latest).unwrap();
    validate_contract_value(API_CONTRACT_SNAPSHOT_SCHEMA, &snapshot_value)
        .expect("valid generated API snapshot");
    validate_contract_value(API_CONTRACT_LATEST_SCHEMA, &latest_value)
        .expect("valid generated API latest pointer");

    let fixture = read_fixture("api_contracts.v1.json");
    assert_eq!(snapshot_value, fixture["snapshot"]);
    assert_eq!(latest_value, fixture["latest"]);
    assert_eq!(serde_json::to_value(diff).unwrap(), fixture["diff"]);
    assert_eq!(serde_json::to_value(cleanup).unwrap(), fixture["cleanup"]);
}

fn read_fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    serde_json::from_str(
        &fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}
