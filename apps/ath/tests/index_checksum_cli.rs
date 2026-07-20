use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

#[test]
fn index_publishes_checksum_bound_pointer_and_repair_detects_tampering() {
    let root = temp_root("checksum");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn checksum_fixture() {}\n").unwrap();

    let output = run(&[
        "index",
        root.to_str().expect("UTF-8 fixture path"),
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let pointer_path = root.join(".athanor/state/index-current.json");
    let pointer: Value = serde_json::from_slice(&fs::read(&pointer_path).unwrap()).unwrap();
    assert_eq!(pointer["schema"], "athanor.index_current.v2");
    assert_digest(&pointer["read_model_manifest_sha256"]);
    assert_digest(&pointer["index_state_sha256"]);

    let read_model = root.join(
        pointer["read_model"]
            .as_str()
            .expect("read-model pointer path"),
    );
    let state = root.join(
        pointer["index_state"]
            .as_str()
            .expect("index-state pointer path"),
    );
    assert!(state.is_file());
    let manifest: Value =
        serde_json::from_slice(&fs::read(read_model.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["checksums"]["algorithm"], "sha256");
    assert_eq!(
        manifest["checksums"]["files"]
            .as_object()
            .expect("checksum file map")
            .len(),
        4
    );

    fs::write(read_model.join("entities.jsonl"), "tampered\n").unwrap();
    assert_checksum_issue(&root);

    fs::remove_dir_all(&root).unwrap();
    let root = indexed_root("missing-file");
    let pointer: Value =
        serde_json::from_slice(&fs::read(root.join(".athanor/state/index-current.json")).unwrap())
            .unwrap();
    let read_model = root.join(pointer["read_model"].as_str().unwrap());
    fs::remove_file(read_model.join("facts.jsonl")).unwrap();
    assert_checksum_issue(&root);

    fs::remove_dir_all(&root).unwrap();
    let root = indexed_root("state-tamper");
    let pointer: Value =
        serde_json::from_slice(&fs::read(root.join(".athanor/state/index-current.json")).unwrap())
            .unwrap();
    fs::write(root.join(pointer["index_state"].as_str().unwrap()), "{}\n").unwrap();
    assert_checksum_issue(&root);
    fs::remove_dir_all(root).unwrap();
}

fn indexed_root(label: &str) -> PathBuf {
    let root = temp_root(label);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn indexed_fixture() {}\n").unwrap();
    let output = run(&[
        "index",
        root.to_str().expect("UTF-8 fixture path"),
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    root
}

fn assert_checksum_issue(root: &Path) {
    let output = run(&[
        "repair",
        "inspect",
        root.to_str().expect("UTF-8 fixture path"),
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).expect("parse repair inspection");
    assert!(
        report["issues"]
            .as_array()
            .expect("repair issues")
            .iter()
            .any(|issue| issue["code"] == "index_current_checksum_mismatch"),
        "repair report did not contain checksum mismatch: {report}"
    );
}

fn assert_digest(value: &Value) {
    let digest = value.as_str().expect("SHA-256 digest string");
    assert!(digest.starts_with("sha256:"));
    assert_eq!(digest.len(), "sha256:".len() + 64);
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-cli-index-{label}-{nonce}"))
}
