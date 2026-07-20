use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    API_SNAPSHOT_SCHEMA_V1, ApiSnapshotReport, DOCS_PATCH_SCHEMA, DOCS_PROPOSE_FIX_SCHEMA_V1,
    DocsFrontmatterChange, DocsPatchOperation, DocsPatchProposal, DocsProposeFixReport,
    VersionedApiSnapshotReport, VersionedDocsProposeFixReport, VersionedJsonContract,
};
use serde_json::{Value, json};

#[test]
fn remaining_application_wrappers_match_golden_fixture() {
    let api = VersionedApiSnapshotReport::from(ApiSnapshotReport {
        snapshot: "snap-api".to_string(),
        path: PathBuf::from("project/.athanor/api/snapshots/snap-api.json"),
        created: true,
        endpoints: 3,
        schemas: 2,
        examples: 1,
        cleanup: None,
    });
    let docs = VersionedDocsProposeFixReport::from(DocsProposeFixReport {
        proposal: DocsPatchProposal {
            schema: DOCS_PATCH_SCHEMA.to_string(),
            id: "docs-patch-snap-docs".to_string(),
            snapshot: "snap-docs".to_string(),
            operations: vec![DocsPatchOperation {
                path: "docs/catalog.md".to_string(),
                stable_key: "doc://docs/catalog.md".to_string(),
                create: false,
                content: None,
                changes: vec![DocsFrontmatterChange {
                    field: "last_verified_snapshot".to_string(),
                    old_value: Some(json!("snap-old")),
                    new_value: json!("snap-docs"),
                    reason: "verified_against_older_snapshot".to_string(),
                }],
            }],
        },
        path: PathBuf::from("project/.athanor/patches/docs/docs-patch-snap-docs.json"),
    });

    api.validate_contract()
        .expect("valid versioned API snapshot report");
    docs.validate_contract()
        .expect("valid versioned docs propose-fix report");
    assert_eq!(api.schema, API_SNAPSHOT_SCHEMA_V1);
    assert_eq!(docs.schema, DOCS_PROPOSE_FIX_SCHEMA_V1);

    let fixture = read_fixture("remaining_application_contracts.v1.json");
    assert_eq!(serde_json::to_value(api).unwrap(), fixture["api_snapshot"]);
    assert_eq!(
        serde_json::to_value(docs).unwrap(),
        fixture["docs_propose_fix"]
    );
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
