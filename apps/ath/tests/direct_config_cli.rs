use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

#[test]
fn config_help_is_served_by_direct_dispatcher() {
    for command in ["validate", "doctor"] {
        let output = run(&["config", command, "--help"]);
        assert!(
            output.status.success(),
            "config {command} help failed: {}",
            stderr(&output)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("--path"));
        assert!(stdout.contains("--json"));
    }
}

#[test]
fn config_validate_json_uses_versioned_flattened_report() {
    let root = temp_root("validate");
    let output = run(&[
        "config",
        "validate",
        "--path",
        root.to_str().expect("UTF-8 temp path"),
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let report: Value = serde_json::from_slice(&output.stdout).expect("config validate JSON");
    assert_eq!(report["schema"], "athanor.config_validate.v1");
    assert_eq!(report["root"], path_value(&root));
    assert!(report.get("config").is_none());
    assert!(report.get("storage").is_some());

    cleanup(&root);
}

#[test]
fn config_doctor_json_matches_typed_report_shape() {
    let root = temp_root("doctor");
    let output = run(&[
        "config",
        "doctor",
        "--path",
        root.to_str().expect("UTF-8 temp path"),
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let report: Value = serde_json::from_slice(&output.stdout).expect("config doctor JSON");
    assert_eq!(report["schema"], "athanor.config_doctor.v1");
    assert_eq!(report["root"], path_value(&root));
    assert!(report["config"].is_object());
    assert_eq!(report["checks"].as_array().map(Vec::len), Some(3));

    cleanup(&root);
}

#[test]
fn config_validate_rejects_unknown_fields_before_output() {
    let root = temp_root("invalid");
    fs::write(root.join("athanor.toml"), "unknown = true\n")
        .expect("write invalid config");
    let output = run(&[
        "config",
        "validate",
        "--path",
        root.to_str().expect("UTF-8 temp path"),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("unknown field"));

    cleanup(&root);
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "athanor-direct-config-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create temp project");
    root
}

fn path_value(path: &Path) -> Value {
    json!(path)
}

fn cleanup(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
