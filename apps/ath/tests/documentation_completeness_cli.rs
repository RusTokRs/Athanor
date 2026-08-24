use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn completeness_help_exposes_exact_snapshot_limit_and_json() {
    let output = run(&["docs", "completeness", "--help"]);
    assert!(output.status.success(), "help stderr: {}", stderr(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--snapshot"));
    assert!(stdout.contains("--limit"));
    assert!(stdout.contains("--json"));
}

#[test]
fn exact_snapshot_completeness_round_trip_through_binary() {
    let root = fixture_root("round-trip");
    let root_arg = root.to_str().expect("UTF-8 temp path");

    let indexed = run(&["index", root_arg, "--json"]);
    assert!(indexed.status.success(), "index stderr: {}", stderr(&indexed));
    let index: Value = serde_json::from_slice(&indexed.stdout).expect("index JSON");
    let snapshot = index["snapshot"].as_str().expect("snapshot id");

    let json = json_command(&[
        "docs",
        "completeness",
        root_arg,
        "--snapshot",
        snapshot,
        "--limit",
        "8",
        "--json",
    ]);
    assert_eq!(json["schema"], "athanor.documentation_completeness.v1");
    assert_eq!(json["snapshot"], snapshot);
    assert_eq!(json["baseline_adapter"], "file");
    assert_eq!(json["limit"], 8);
    assert!(json["totals"]["tracked_files"].as_u64().unwrap() >= 2);

    let text = run(&[
        "docs",
        "completeness",
        root_arg,
        "--snapshot",
        snapshot,
        "--limit",
        "8",
    ]);
    assert!(text.status.success(), "text stderr: {}", stderr(&text));
    let stdout = String::from_utf8_lossy(&text.stdout);
    assert!(stdout.contains("documentation completeness for"));
    assert!(stdout.contains(snapshot));
    assert!(stdout.contains("languages:"));
    assert!(stdout.contains("adapters:"));

    cleanup(&root);
}

#[test]
fn missing_exact_snapshot_fails_without_generated_documentation_side_effects() {
    let root = fixture_root("missing");
    let root_arg = root.to_str().expect("UTF-8 temp path");
    let output = run(&[
        "docs",
        "completeness",
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
        "athanor-completeness-cli-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("src")).expect("create fixture root");
    fs::write(
        root.join("README.md"),
        "# Example Project\n\n## Getting Started\n\nSee src/lib.rs.\n",
    )
    .expect("write README fixture");
    fs::write(root.join("src/lib.rs"), "pub fn example() {}\n").expect("write Rust fixture");
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

fn cleanup(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
