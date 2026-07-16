use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    PROJECT_REGISTRY_REPORT_SCHEMA_V1, ProjectRegistration, ProjectRegistryReport,
    VersionedJsonContract,
};
use serde_json::Value;

#[test]
fn project_registry_report_matches_golden_fixture() {
    let report = ProjectRegistryReport {
        schema: PROJECT_REGISTRY_REPORT_SCHEMA_V1.to_string(),
        registry_path: PathBuf::from("state/projects.json"),
        projects: vec![
            ProjectRegistration {
                project_id: "alpha".to_string(),
                root: PathBuf::from("/workspace/alpha"),
            },
            ProjectRegistration {
                project_id: "beta".to_string(),
                root: PathBuf::from("/workspace/beta"),
            },
        ],
    };

    report
        .validate_contract()
        .expect("project registry report contract must remain valid");
    assert_eq!(
        serde_json::to_value(&report).expect("serialize project registry report"),
        read_fixture("project_registry_report.v1.json")
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
