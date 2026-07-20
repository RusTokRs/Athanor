use std::collections::BTreeSet;

use athanor_app::{
    INDEX_GENERATION_CLEANUP_SCHEMA_V1, REPAIR_APPLY_SCHEMA_V2, REPAIR_CANONICAL_LATEST_SCHEMA_V1,
    REPAIR_CLEANUP_SCHEMA_V2, REPAIR_INSPECT_SCHEMA_V2, REPAIR_RECOVER_CANONICAL_SCHEMA_V1,
    REPAIR_RECOVER_INDEX_CLEANUP_SCHEMA_V1, REPAIR_RECOVER_INDEX_SCHEMA_V1,
    REPAIR_REGENERATE_SCHEMA_V1, VERSIONED_JSON_CONTRACTS, validate_contract_registry,
};

#[test]
fn repair_public_reports_are_registered_and_embedded_types_are_not() {
    validate_contract_registry(VERSIONED_JSON_CONTRACTS)
        .expect("JSON contract registry must remain valid");

    let contracts = VERSIONED_JSON_CONTRACTS
        .iter()
        .map(|contract| (contract.schema, contract.rust_type))
        .collect::<BTreeSet<_>>();
    for expected in [
        (REPAIR_INSPECT_SCHEMA_V2, "RepairInspectReport"),
        (REPAIR_CLEANUP_SCHEMA_V2, "RepairCleanupReport"),
        (REPAIR_REGENERATE_SCHEMA_V1, "RepairRegenerateReport"),
        (
            REPAIR_RECOVER_CANONICAL_SCHEMA_V1,
            "RepairRecoverCanonicalReport",
        ),
        (REPAIR_APPLY_SCHEMA_V2, "RepairApplyReport"),
        (
            INDEX_GENERATION_CLEANUP_SCHEMA_V1,
            "IndexGenerationCleanupReport",
        ),
        (REPAIR_RECOVER_INDEX_SCHEMA_V1, "RepairRecoverIndexReport"),
        (
            REPAIR_RECOVER_INDEX_CLEANUP_SCHEMA_V1,
            "RepairRecoverIndexCleanupReport",
        ),
        (
            REPAIR_CANONICAL_LATEST_SCHEMA_V1,
            "RepairCanonicalLatestReport",
        ),
    ] {
        assert!(
            contracts.contains(&expected),
            "missing repair owner {expected:?}"
        );
    }

    let registered_types = VERSIONED_JSON_CONTRACTS
        .iter()
        .map(|contract| contract.rust_type)
        .collect::<BTreeSet<_>>();
    for embedded in [
        "CanonicalRepairState",
        "GeneratedRepairState",
        "RepairIssue",
        "RepairCleanupRemoval",
        "RepairCleanupRetained",
        "IndexGenerationCleanupRow",
        "IndexCleanupTombstone",
    ] {
        assert!(
            !registered_types.contains(embedded),
            "embedded repair type `{embedded}` must not become a top-level contract owner"
        );
    }
}
