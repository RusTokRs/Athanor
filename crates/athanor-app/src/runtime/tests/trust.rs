use std::fs;

use crate::adapter_contract::{
    ADAPTER_MANIFEST_SCHEMA_V1, ADAPTER_TRUST_REGISTRY_SCHEMA_V2,
    ADAPTER_TRUST_REPORT_SCHEMA_V1,
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
