use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use athanor_app::{
    API_SNAPSHOT_SCHEMA_V1, DOCS_PROPOSE_FIX_SCHEMA_V1, VERSIONED_JSON_CONTRACTS,
    validate_contract_registry,
};

#[test]
fn remaining_application_wrapper_schemas_are_registered_and_owned() {
    validate_contract_registry(VERSIONED_JSON_CONTRACTS)
        .expect("extended JSON contract registry must remain valid");

    let contracts = VERSIONED_JSON_CONTRACTS
        .iter()
        .map(|contract| (contract.schema, contract.rust_type))
        .collect::<BTreeSet<_>>();

    assert!(contracts.contains(&(API_SNAPSHOT_SCHEMA_V1, "VersionedApiSnapshotReport")));
    assert!(contracts.contains(&(
        DOCS_PROPOSE_FIX_SCHEMA_V1,
        "VersionedDocsProposeFixReport"
    )));

    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/response_contract.rs"),
    )
    .expect("read response contract source");
    assert!(source.contains("\"athanor.api_snapshot.v1\""));
    assert!(source.contains("\"athanor.docs_propose_fix.v1\""));
}
