use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    CanonicalRepairState, GeneratedRepairState, INDEX_GENERATION_CLEANUP_SCHEMA_V1,
    IndexGenerationCleanupReport, REPAIR_APPLY_SCHEMA_V2, REPAIR_CANONICAL_LATEST_SCHEMA_V1,
    REPAIR_CLEANUP_SCHEMA_V2, REPAIR_INSPECT_SCHEMA_V2, REPAIR_RECOVER_CANONICAL_SCHEMA_V1,
    REPAIR_RECOVER_INDEX_CLEANUP_SCHEMA_V1, REPAIR_RECOVER_INDEX_SCHEMA_V1,
    REPAIR_REGENERATE_SCHEMA_V1, RepairApplyReport, RepairCanonicalLatestReport,
    RepairCleanupReport, RepairInspectReport, RepairRecoverCanonicalReport,
    RepairRecoverIndexCleanupReport, RepairRecoverIndexReport, RepairRegenerateReport,
    RepairStatus, VersionedJsonContract,
};
use athanor_core::CanonicalLatestIdentity;
use athanor_domain::SnapshotId;
use serde_json::Value;

#[test]
fn repair_reports_match_golden_fixture() {
    let inspect = inspection();
    let cleanup = cleanup();
    let regenerate = regenerate();
    let recover_canonical = recover_canonical();
    let apply = RepairApplyReport {
        schema: REPAIR_APPLY_SCHEMA_V2.to_string(),
        root: PathBuf::from("project"),
        dry_run: true,
        canonical: recover_canonical.clone(),
        generated: regenerate.clone(),
        cleanup: cleanup.clone(),
        remaining_issues: Vec::new(),
    };
    let index_cleanup = IndexGenerationCleanupReport {
        schema: INDEX_GENERATION_CLEANUP_SCHEMA_V1.to_string(),
        root: PathBuf::from("project"),
        dry_run: true,
        confirmation_token: None,
        removed: Vec::new(),
        retained: Vec::new(),
        remaining_issues: Vec::new(),
        inspection: inspect.clone(),
    };
    let recover_index = RepairRecoverIndexReport {
        schema: REPAIR_RECOVER_INDEX_SCHEMA_V1.to_string(),
        root: PathBuf::from("project"),
        dry_run: true,
        needed: false,
        recovered: false,
        snapshot: Some("snap-current".to_string()),
        generation: Some("gen_snap-current".to_string()),
        remaining_issues: Vec::new(),
        inspection: inspect.clone(),
    };
    let recover_index_cleanup = RepairRecoverIndexCleanupReport {
        schema: REPAIR_RECOVER_INDEX_CLEANUP_SCHEMA_V1.to_string(),
        root: PathBuf::from("project"),
        dry_run: true,
        needed: false,
        recovered: false,
        tombstones: Vec::new(),
        remaining_issues: Vec::new(),
    };
    let canonical_latest = RepairCanonicalLatestReport {
        schema: REPAIR_CANONICAL_LATEST_SCHEMA_V1.to_string(),
        root: PathBuf::from("project"),
        dry_run: true,
        needed: false,
        repaired: false,
        target: CanonicalLatestIdentity::for_snapshot(SnapshotId("snap-current".to_string())),
        previous: None,
        previous_error: None,
        remaining_issues: Vec::new(),
    };

    inspect
        .validate_contract()
        .expect("valid repair inspect contract");
    cleanup
        .validate_contract()
        .expect("valid repair cleanup contract");
    regenerate
        .validate_contract()
        .expect("valid repair regenerate contract");
    recover_canonical
        .validate_contract()
        .expect("valid repair canonical recovery contract");
    apply
        .validate_contract()
        .expect("valid repair apply contract");
    index_cleanup
        .validate_contract()
        .expect("valid index cleanup contract");
    recover_index
        .validate_contract()
        .expect("valid index publication recovery contract");
    recover_index_cleanup
        .validate_contract()
        .expect("valid index cleanup recovery contract");
    canonical_latest
        .validate_contract()
        .expect("valid canonical latest repair contract");

    let fixture = read_fixture("repair_contracts.v1.json");
    for (key, value) in [
        ("inspect", serde_json::to_value(inspect).unwrap()),
        ("cleanup", serde_json::to_value(cleanup).unwrap()),
        ("regenerate", serde_json::to_value(regenerate).unwrap()),
        (
            "recover_canonical",
            serde_json::to_value(recover_canonical).unwrap(),
        ),
        ("apply", serde_json::to_value(apply).unwrap()),
        (
            "index_cleanup",
            serde_json::to_value(index_cleanup).unwrap(),
        ),
        (
            "recover_index",
            serde_json::to_value(recover_index).unwrap(),
        ),
        (
            "recover_index_cleanup",
            serde_json::to_value(recover_index_cleanup).unwrap(),
        ),
        (
            "canonical_latest",
            serde_json::to_value(canonical_latest).unwrap(),
        ),
    ] {
        assert_eq!(value, fixture[key], "repair fixture mismatch for {key}");
    }
}

fn inspection() -> RepairInspectReport {
    RepairInspectReport {
        schema: REPAIR_INSPECT_SCHEMA_V2.to_string(),
        root: PathBuf::from("project"),
        status: RepairStatus::Clean,
        issues: Vec::new(),
        canonical: CanonicalRepairState {
            latest_snapshot: Some("snap-current".to_string()),
            snapshot_count: 1,
            orphan_snapshots: Vec::new(),
        },
        generated: GeneratedRepairState {
            current_generation: Some("00000001".to_string()),
            generation_count: 1,
            orphan_generations: Vec::new(),
        },
    }
}

fn cleanup() -> RepairCleanupReport {
    RepairCleanupReport {
        schema: REPAIR_CLEANUP_SCHEMA_V2.to_string(),
        root: PathBuf::from("project"),
        dry_run: true,
        removed: Vec::new(),
        retained: Vec::new(),
        remaining_issues: Vec::new(),
        inspection: inspection(),
    }
}

fn regenerate() -> RepairRegenerateReport {
    RepairRegenerateReport {
        schema: REPAIR_REGENERATE_SCHEMA_V1.to_string(),
        root: PathBuf::from("project"),
        dry_run: true,
        needed: false,
        generated: None,
        remaining_issues: Vec::new(),
        inspection: inspection(),
    }
}

fn recover_canonical() -> RepairRecoverCanonicalReport {
    RepairRecoverCanonicalReport {
        schema: REPAIR_RECOVER_CANONICAL_SCHEMA_V1.to_string(),
        root: PathBuf::from("project"),
        dry_run: true,
        needed: false,
        selected_snapshot: Some("snap-current".to_string()),
        recovered_snapshot: None,
        remaining_issues: Vec::new(),
        inspection: inspection(),
    }
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
