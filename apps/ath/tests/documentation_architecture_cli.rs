use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn architecture_documentation_help_exposes_exact_snapshot_and_inspection() {
    for args in [
        vec!["docs", "generate-architecture", "--help"],
        vec!["docs", "architecture", "current", "--help"],
        vec!["docs", "architecture", "manifest", "--help"],
        vec!["docs", "architecture", "validation", "--help"],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{} help failed: {}",
            args.join(" "),
            stderr(&output)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("--json"));
        if args[1] == "generate-architecture" {
            assert!(stdout.contains("--snapshot"));
            assert!(stdout.contains("--force"));
            assert!(stdout.contains("--max-entities"));
        }
    }
}

#[test]
fn exact_snapshot_generation_and_inspection_round_trip_through_binary() {
    let root = fixture_root("round-trip");
    let root_arg = root.to_str().expect("UTF-8 temp path");

    let indexed = run(&["index", root_arg, "--json"]);
    assert!(
        indexed.status.success(),
        "index stderr: {}",
        stderr(&indexed)
    );
    let index: Value = serde_json::from_slice(&indexed.stdout).expect("index JSON");
    let snapshot = index["snapshot"].as_str().expect("snapshot id");

    let generated = run(&[
        "docs",
        "generate-architecture",
        root_arg,
        "--snapshot",
        snapshot,
        "--json",
    ]);
    assert!(
        generated.status.success(),
        "generation stderr: {}",
        stderr(&generated)
    );
    let generation: Value =
        serde_json::from_slice(&generated.stdout).expect("generation JSON report");
    assert_eq!(generation["status"], "published");
    assert_eq!(generation["snapshot"], snapshot);
    assert!(path_from_json(&generation["document"]).is_file());
    assert!(path_from_json(&generation["manifest"]).is_file());
    assert!(path_from_json(&generation["validation_report"]).is_file());

    let current = json_command(&["docs", "architecture", "current", root_arg, "--json"]);
    assert_eq!(current["current"]["snapshot"], snapshot);
    assert_eq!(current["current"]["generation"], generation["generation"]);

    let manifest = json_command(&["docs", "architecture", "manifest", root_arg, "--json"]);
    assert_eq!(manifest["manifest"]["snapshot"], snapshot);
    assert_eq!(
        manifest["manifest"]["documents"].as_array().map(Vec::len),
        Some(2)
    );

    let validation = json_command(&["docs", "architecture", "validation", root_arg, "--json"]);
    assert_eq!(validation["report"]["snapshot"], snapshot);
    assert_eq!(validation["report"]["status"], "valid");

    let repeated = json_command(&[
        "docs",
        "generate-architecture",
        root_arg,
        "--snapshot",
        snapshot,
        "--json",
    ]);
    assert_eq!(repeated["status"], "up_to_date");
    assert_eq!(repeated["generation"], generation["generation"]);

    cleanup(&root);
}

#[test]
fn missing_exact_snapshot_fails_without_publication() {
    let root = fixture_root("missing");
    let root_arg = root.to_str().expect("UTF-8 temp path");
    let output = run(&[
        "docs",
        "generate-architecture",
        root_arg,
        "--snapshot",
        "snap-missing",
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("not committed or does not exist"));
    assert!(!root.join(".athanor/generated/documentation").exists());
    cleanup(&root);
}

fn fixture_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "athanor-documentation-cli-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("src")).expect("create fixture source directory");
    fs::write(
        root.join("src/lib.rs"),
        "pub mod service { pub fn execute() -> &'static str { \"ok\" } }\n",
    )
    .expect("write Rust fixture");
    fs::write(
        root.join("README.md"),
        "---\nid: doc://README.md\nkind: project_overview\nlanguage: en\nsource_language: en\nstatus: active\n---\n# Fixture Service\n\nThe service exposes `execute`.\n",
    )
    .expect("write documentation fixture");
    root
}

fn json_command(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{} failed: {}",
        args.join(" "),
        stderr(&output)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid JSON for {}: {error}", args.join(" ")))
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

fn path_from_json(value: &Value) -> PathBuf {
    PathBuf::from(value.as_str().expect("path string"))
}

fn cleanup(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
