use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

#[test]
fn bridge_recovery_upgrades_pre_checksum_generation_and_is_idempotent() {
    let root = indexed_root("migration");
    let pointer_path = root.join(".athanor/state/index-current.json");
    let pointer: Value = serde_json::from_slice(&fs::read(&pointer_path).unwrap()).unwrap();
    let snapshot = pointer["snapshot"].as_str().unwrap().to_string();
    let generation = pointer["generation"].as_str().unwrap().to_string();
    let read_model = root.join(pointer["read_model"].as_str().unwrap());

    let mut manifest: Value =
        serde_json::from_slice(&fs::read(read_model.join("manifest.json")).unwrap()).unwrap();
    manifest
        .as_object_mut()
        .expect("manifest object")
        .remove("checksums");
    fs::write(
        read_model.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    fs::write(
        &pointer_path,
        serde_json::to_vec_pretty(&json!({
            "schema": "athanor.index_current.v1",
            "snapshot": snapshot,
            "generation": generation,
            "read_model": pointer["read_model"],
            "index_state": pointer["index_state"]
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        root.join(".athanor/state/index-current-publication.json"),
        serde_json::to_vec_pretty(&json!({
            "schema": "athanor.index_current_publication.v1",
            "snapshot": snapshot,
            "generation": generation
        }))
        .unwrap(),
    )
    .unwrap();

    let output = run(&[
        "repair",
        "recover-index",
        root.to_str().expect("UTF-8 fixture path"),
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["recovered"], true);

    let repaired: Value = serde_json::from_slice(&fs::read(&pointer_path).unwrap()).unwrap();
    assert_eq!(repaired["schema"], "athanor.index_current.v2");
    assert_digest(&repaired["read_model_manifest_sha256"]);
    assert_digest(&repaired["index_state_sha256"]);
    let manifest: Value =
        serde_json::from_slice(&fs::read(read_model.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["checksums"]["algorithm"], "sha256");
    assert!(
        !root
            .join(".athanor/state/index-current-publication.json")
            .exists()
    );

    let second = run(&[
        "repair",
        "recover-index",
        root.to_str().expect("UTF-8 fixture path"),
        "--json",
    ]);
    assert!(second.status.success(), "stderr: {}", stderr(&second));
    let second_report: Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(second_report["needed"], false);
    assert_eq!(second_report["recovered"], false);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn extra_selected_read_model_file_is_rejected() {
    let root = indexed_root("extra-file");
    let pointer: Value = serde_json::from_slice(
        &fs::read(root.join(".athanor/state/index-current.json")).unwrap(),
    )
    .unwrap();
    let read_model = root.join(pointer["read_model"].as_str().unwrap());
    fs::write(read_model.join("substituted.jsonl"), "{}\n").unwrap();

    let output = run(&[
        "repair",
        "inspect",
        root.to_str().expect("UTF-8 fixture path"),
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "index_current_checksum_mismatch")
    );
    fs::remove_dir_all(root).unwrap();
}

fn indexed_root(label: &str) -> PathBuf {
    let root = temp_root(label);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn checksum_recovery() {}\n").unwrap();
    let output = run(&[
        "index",
        root.to_str().expect("UTF-8 fixture path"),
        "--json",
    ]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    root
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
    std::env::temp_dir().join(format!("athanor-cli-checksum-{label}-{nonce}"))
}

#[allow(dead_code)]
fn _assert_path(_: &Path) {}
