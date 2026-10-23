use std::fs;

use crate::adapter_contract::{
    ADAPTER_MANIFEST_SCHEMA_V1, ADAPTER_TRUST_REGISTRY_SCHEMA_V2, ADAPTER_TRUST_REPORT_SCHEMA_V1,
};

use super::super::*;
use super::fixtures::{empty_output_command, empty_output_program, temp_root};

#[test]
fn external_process_plugins_require_opt_in_trust_and_allowlist() {
    let root = temp_root("runtime-process-policy");
    let manifest_dir = root.join(".athanor/adapters");
    fs::create_dir_all(&manifest_dir).unwrap();
    let manifest_path = manifest_dir.join("external.json");
    let manifest = AdapterPluginManifest {
        schema: ADAPTER_MANIFEST_SCHEMA_V1.to_string(),
        name: "external-policy".to_string(),
        version: None,
        adapters: vec![AdapterPluginEntry {
            id: "external.extractor.empty".to_string(),
            kind: AdapterPluginKind::Extractor,
            enabled: true,
            command: Some(empty_output_command()),
            supports_extensions: vec!["rs".to_string()],
        }],
    };
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let trust_path = root.join("state/adapter-trust.json");

    let error = RuntimeBuilder::new(&root)
        .adapter_trust_path(&trust_path)
        .with_discovered_plugins()
        .err()
        .expect("external process adapter should be rejected by default");
    assert!(
        error
            .to_string()
            .contains("external process adapters are disabled")
    );

    let error = RuntimeBuilder::new(&root)
        .adapter_trust_path(&trust_path)
        .allow_external_process(true)
        .allowed_external_process_programs([empty_output_program()])
        .with_discovered_plugins()
        .err()
        .expect("untrusted external process adapter should be rejected");
    assert!(error.to_string().contains("is not trusted"));

    let report = trust_adapter_plugin(AdapterTrustOptions {
        trust_path: trust_path.clone(),
        manifest_path,
    })
    .expect("trusting plugin should succeed");
    assert_eq!(report.schema, ADAPTER_TRUST_REPORT_SCHEMA_V1);

    let error = RuntimeBuilder::new(&root)
        .adapter_trust_path(&trust_path)
        .allow_external_process(true)
        .with_discovered_plugins()
        .err()
        .expect("external process adapter without allowlist should be rejected");
    assert!(error.to_string().contains("external_process_allowlist"));

    RuntimeBuilder::new(&root)
        .adapter_trust_path(&trust_path)
        .allow_external_process(true)
        .allowed_external_process_programs([empty_output_program()])
        .with_discovered_plugins()
        .expect("explicit opt-in, trust, and allowlist should allow external process adapters");

    let persisted: serde_json::Value =
        serde_json::from_slice(&fs::read(&trust_path).unwrap()).unwrap();
    assert_eq!(persisted["schema"], ADAPTER_TRUST_REGISTRY_SCHEMA_V2);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn trust_report_is_versioned_and_manifest_changes_invalidate_trust() {
    let root = temp_root("runtime-trust-hash");
    let manifest_dir = root.join(".athanor/adapters");
    fs::create_dir_all(&manifest_dir).unwrap();
    let manifest_path = manifest_dir.join("external.json");
    let mut manifest = AdapterPluginManifest {
        schema: ADAPTER_MANIFEST_SCHEMA_V1.to_string(),
        name: "external-trust-hash".to_string(),
        version: None,
        adapters: vec![AdapterPluginEntry {
            id: "external.extractor.empty".to_string(),
            kind: AdapterPluginKind::Extractor,
            enabled: true,
            command: Some(empty_output_command()),
            supports_extensions: vec!["rs".to_string()],
        }],
    };
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let trust_path = root.join("state/adapter-trust.json");
    trust_adapter_plugin(AdapterTrustOptions {
        trust_path: trust_path.clone(),
        manifest_path: manifest_path.clone(),
    })
    .unwrap();

    let trusted = list_adapter_plugin_trust(AdapterTrustListOptions {
        root: root.clone(),
        trust_path: trust_path.clone(),
    })
    .unwrap();
    assert_eq!(trusted.schema, ADAPTER_TRUST_REPORT_SCHEMA_V1);
    assert!(trusted.plugins[0].trusted);

    manifest.version = Some("0.2.0".to_string());
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let changed = list_adapter_plugin_trust(AdapterTrustListOptions {
        root: root.clone(),
        trust_path,
    })
    .unwrap();
    assert!(!changed.plugins[0].trusted);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sidecar_file_args_are_part_of_the_trust_record() {
    let root = temp_root("runtime-trust-sidecar");
    let manifest_dir = root.join(".athanor/adapters");
    fs::create_dir_all(&manifest_dir).unwrap();
    let payload = manifest_dir.join("payload.py");
    fs::write(&payload, b"print('one')\n").unwrap();
    let manifest_path = manifest_dir.join("external.json");
    let mut command = empty_output_command();
    command.args.push(payload.display().to_string());
    let manifest = AdapterPluginManifest {
        schema: ADAPTER_MANIFEST_SCHEMA_V1.to_string(),
        name: "sidecar-trust".to_string(),
        version: None,
        adapters: vec![AdapterPluginEntry {
            id: "external.extractor.empty".to_string(),
            kind: AdapterPluginKind::Extractor,
            enabled: true,
            command: Some(command),
            supports_extensions: vec!["rs".to_string()],
        }],
    };
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let trust_path = root.join("state/adapter-trust.json");
    trust_adapter_plugin(AdapterTrustOptions {
        trust_path: trust_path.clone(),
        manifest_path: manifest_path.clone(),
    })
    .unwrap();
    let trusted = list_adapter_plugin_trust(AdapterTrustListOptions {
        root: root.clone(),
        trust_path: trust_path.clone(),
    })
    .unwrap();
    assert!(trusted.plugins[0].trusted);
    assert!(
        trusted.plugins[0]
            .executable_hashes
            .iter()
            .any(|executable| executable.program.ends_with("payload.py"))
    );

    fs::write(&payload, b"print('two')\n").unwrap();
    let changed = list_adapter_plugin_trust(AdapterTrustListOptions {
        root: root.clone(),
        trust_path: trust_path.clone(),
    })
    .unwrap();
    assert!(!changed.plugins[0].trusted);

    fs::remove_file(&manifest_path).unwrap();
    untrust_adapter_plugin(AdapterTrustOptions {
        trust_path,
        manifest_path,
    })
    .expect("untrust must work after the manifest file is removed");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn launch_digests_must_match_the_trust_record() {
    let root = temp_root("runtime-trust-bind");
    let manifest_dir = root.join(".athanor/adapters");
    fs::create_dir_all(&manifest_dir).unwrap();
    let program = manifest_dir.join("adapter.bin");
    fs::copy(empty_output_program(), &program).unwrap();
    let manifest_path = manifest_dir.join("external.json");
    let manifest = AdapterPluginManifest {
        schema: ADAPTER_MANIFEST_SCHEMA_V1.to_string(),
        name: "bind-trust".to_string(),
        version: None,
        adapters: vec![AdapterPluginEntry {
            id: "external.extractor.empty".to_string(),
            kind: AdapterPluginKind::Extractor,
            enabled: true,
            command: Some(AdapterProcessCommand {
                program: "./adapter.bin".to_string(),
                args: Vec::new(),
            }),
            supports_extensions: vec!["rs".to_string()],
        }],
    };
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let plugin = super::super::plugin_discovery::read_manifest(manifest_path).unwrap();
    let trusted = super::super::trust::trusted_plugin_record(&plugin).unwrap();
    fs::write(&program, b"replaced-adapter-bytes").unwrap();
    let error = super::super::process_adapter_support::ProcessCommand::from_manifest_with_options(
        &manifest_dir,
        plugin.manifest.adapters[0].command.as_ref().unwrap(),
        false,
        super::super::process_adapter_support::ProcessLimits::default(),
        Some(&trusted.executable_hashes),
    )
    .expect_err("replaced payload must not bind to the previous trust record");
    assert!(
        error
            .to_string()
            .contains("does not match the trusted digest")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn every_launch_rechecks_payload_hash_when_size_is_unchanged() {
    let root = temp_root("runtime-trust-rehash");
    let manifest_dir = root.join(".athanor/adapters");
    fs::create_dir_all(&manifest_dir).unwrap();
    let program = manifest_dir.join("adapter.bin");
    fs::write(&program, b"trusted").unwrap();
    let command = AdapterProcessCommand {
        program: "./adapter.bin".to_string(),
        args: Vec::new(),
    };
    let command = super::super::process_adapter_support::ProcessCommand::from_manifest(
        &manifest_dir,
        &command,
    )
    .expect("initial payload should produce launch digests");
    command
        .verify_unchanged()
        .expect("unchanged payload should pass its first launch check");

    fs::write(&program, b"altered").unwrap();

    let error = command.verify_unchanged().expect_err(
        "a same-size payload replacement must not pass after an earlier successful launch",
    );
    assert!(error.to_string().contains("changed after runtime assembly"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_unknown_manifest_fields() {
    let content = serde_json::json!({
        "schema": ADAPTER_MANIFEST_SCHEMA_V1,
        "name": "unknown-field",
        "unexpected": true,
        "adapters": []
    })
    .to_string();
    let error = serde_json::from_str::<AdapterPluginManifest>(&content)
        .expect_err("unknown manifest field should fail deserialization");
    assert!(error.to_string().contains("unknown field"));
}
