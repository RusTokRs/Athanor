use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    CONFIG_DOCTOR_SCHEMA_V1, CONFIG_VALIDATE_SCHEMA_V1, ConfigReportOptions,
    VersionedJsonContract, doctor_project_config, validate_project_config,
};
use serde_json::Value;

#[test]
fn config_reports_match_golden_fixture() {
    let root = PathBuf::from("project");
    let validation = validate_project_config(ConfigReportOptions { root: root.clone() })
        .expect("default validation report");
    let doctor = doctor_project_config(ConfigReportOptions { root })
        .expect("default doctor report");

    validation
        .validate_contract()
        .expect("valid config validation contract");
    doctor
        .validate_contract()
        .expect("valid config doctor contract");

    assert_eq!(validation.schema, CONFIG_VALIDATE_SCHEMA_V1);
    assert_eq!(doctor.schema, CONFIG_DOCTOR_SCHEMA_V1);

    let fixture = read_fixture("config_contracts.v1.json");
    assert_eq!(
        serde_json::to_value(validation).unwrap(),
        fixture["validation"]
    );
    assert_eq!(serde_json::to_value(doctor).unwrap(), fixture["doctor"]);
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
