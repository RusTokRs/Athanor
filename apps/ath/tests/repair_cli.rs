use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

#[test]
fn repair_help_lists_transactional_commands() {
    let output = run(&["repair", "--help"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("index-retention"));
    assert!(stdout.contains("recover-index"));
    assert!(stdout.contains("recover-index-cleanup"));

    let output = run(&["repair", "index-retention", "--help"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("--confirmation-token")
    );
}

#[test]
fn index_retention_dry_run_emits_json_report() {
    let root = temp_root("retention-json");
    let output = run(&[
        "repair",
        "index-retention",
        root.to_str().expect("UTF-8 fixture path"),
        "--dry-run",
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse retention JSON");
    assert_eq!(
        report["schema"],
        serde_json::Value::String("athanor.index_generation_cleanup.v1".to_string())
    );
    assert_eq!(report["dry_run"], serde_json::Value::Bool(true));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recover_index_dry_run_emits_json_without_mutation() {
    let root = temp_root("recover-json");
    let output = run(&[
        "repair",
        "recover-index",
        root.to_str().expect("UTF-8 fixture path"),
        "--dry-run",
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse recovery JSON");
    assert_eq!(
        report["schema"],
        serde_json::Value::String("athanor.repair_recover_index.v1".to_string())
    );
    assert_eq!(report["needed"], serde_json::Value::Bool(false));
    assert!(!root.join(".athanor/state/index-publication.lock").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recover_index_cleanup_dry_run_emits_json_without_locking() {
    let root = temp_root("cleanup-recover-json");
    let output = run(&[
        "repair",
        "recover-index-cleanup",
        root.to_str().expect("UTF-8 fixture path"),
        "--dry-run",
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse cleanup recovery JSON");
    assert_eq!(
        report["schema"],
        serde_json::Value::String("athanor.repair_recover_index_cleanup.v1".to_string())
    );
    assert_eq!(report["needed"], serde_json::Value::Bool(false));
    assert!(!root.join(".athanor/state/index-publication.lock").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_retention_argument_returns_failure_exit_code() {
    let output = run(&["repair", "index-retention", "--keep", "many"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("non-negative integer"));
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("athanor-cli-{label}-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}
