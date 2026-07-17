use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

#[test]
fn plugin_help_preserves_existing_command_family() {
    for args in [
        vec!["plugins", "--help"],
        vec!["plugins", "list", "--help"],
        vec!["plugins", "trust", "--help"],
        vec!["plugins", "untrust", "--help"],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{} help failed: {}",
            args.join(" "),
            stderr(&output)
        );
    }
}

#[test]
fn plugin_commands_emit_public_report_and_persist_registry_schemas() {
    let root = temp_root("plugin-contracts");
    let manifest_dir = root.join(".athanor/adapters");
    fs::create_dir_all(&manifest_dir).unwrap();
    let manifest = manifest_dir.join("legacy.json");
    fs::write(
        &manifest,
        serde_json::to_vec_pretty(&json!({
            "schema": "athanor.adapter_manifest",
            "name": "legacy-cli-fixture",
            "adapters": []
        }))
        .unwrap(),
    )
    .unwrap();
    let trust_store = root.join("state/adapter-trust.json");

    let list = run_json(&[
        "plugins",
        "list",
        path(&root),
        "--trust-store",
        path(&trust_store),
        "--json",
    ]);
    assert_eq!(list["schema"], "athanor.adapter_trust_report.v1");
    assert_eq!(list["plugins"].as_array().map(Vec::len), Some(1));
    assert_eq!(list["plugins"][0]["trusted"], false);

    let trusted = run_json(&[
        "plugins",
        "trust",
        path(&manifest),
        "--trust-store",
        path(&trust_store),
        "--json",
    ]);
    assert_eq!(trusted["schema"], "athanor.adapter_trust_report.v1");
    assert_eq!(trusted["plugins"][0]["trusted"], true);

    let persisted: Value =
        serde_json::from_slice(&fs::read(&trust_store).unwrap()).expect("valid trust registry");
    assert_eq!(persisted["schema"], "athanor.adapter_trust_registry.v2");
    assert!(persisted["trusted_plugins"].is_array());

    let untrusted = run_json(&[
        "plugins",
        "untrust",
        path(&manifest),
        "--trust-store",
        path(&trust_store),
        "--json",
    ]);
    assert_eq!(untrusted["schema"], "athanor.adapter_trust_report.v1");
    assert_eq!(untrusted["plugins"][0]["trusted"], false);

    fs::remove_dir_all(root).unwrap();
}

fn run_json(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{} failed: {}",
        args.join(" "),
        stderr(&output)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{} returned invalid JSON: {error}", args.join(" ")))
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ath"))
        .args(args)
        .output()
        .expect("run ath CLI")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn path(path: &Path) -> &str {
    path.to_str().expect("UTF-8 fixture path")
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-cli-{label}-{nonce}"))
}
