use std::fs;
use std::path::PathBuf;

use athanor_app::{
    ADAPTER_MANIFEST_SCHEMA_LEGACY, ADAPTER_MANIFEST_SCHEMA_V1,
    ADAPTER_NON_PUBLIC_JSON_CONTRACTS, ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2,
    ADAPTER_TRUST_REGISTRY_SCHEMA_V2, ADAPTER_TRUST_REPORT_SCHEMA_V1, AdapterTrustListOptions,
    AdapterTrustOptions, BoundaryLifecycle, VERSIONED_JSON_CONTRACTS,
    discover_adapter_plugins, list_adapter_plugin_trust_versioned, trust_adapter_plugin_versioned,
    validate_adapter_contract_inventory, validate_adapter_non_public_contract_value,
    validate_contract_value,
};
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("fixtures/adapter_contracts.v1.json");
const CONTRACT_SOURCE: &str = include_str!("../src/adapter_contract.rs");
const DISCOVERY_SOURCE: &str = include_str!("../src/runtime/plugin_discovery.rs");
const TRUST_REGISTRY_SOURCE: &str = include_str!("../src/runtime/plugin_trust_registry.rs");

#[test]
fn adapter_contract_owners_are_disjoint_and_fixture_protected() {
    validate_adapter_contract_inventory(VERSIONED_JSON_CONTRACTS)
        .expect("adapter contract registry must remain valid");
    assert_eq!(ADAPTER_NON_PUBLIC_JSON_CONTRACTS.len(), 4);

    let fixture: Value = serde_json::from_str(FIXTURE).expect("valid adapter contract fixture");
    let mut current = 0;
    let mut legacy_input = 0;

    for descriptor in ADAPTER_NON_PUBLIC_JSON_CONTRACTS {
        match descriptor.lifecycle {
            BoundaryLifecycle::Current => {
                current += 1;
                let value = match descriptor.schema {
                    ADAPTER_MANIFEST_SCHEMA_V1 => &fixture["current_manifest"],
                    ADAPTER_TRUST_REGISTRY_SCHEMA_V2 => &fixture["current_trust_registry"],
                    other => panic!("unexpected current adapter boundary {other}"),
                };
                validate_adapter_non_public_contract_value(descriptor, value)
                    .expect("valid current adapter boundary fixture");
            }
            BoundaryLifecycle::LegacyInput => legacy_input += 1,
            BoundaryLifecycle::Historical => {
                panic!("adapter migration has no historical-only boundary")
            }
        }
    }

    assert_eq!(current, 2);
    assert_eq!(legacy_input, 2);
    validate_contract_value(
        ADAPTER_TRUST_REPORT_SCHEMA_V1,
        &fixture["current_trust_report"],
    )
    .expect("valid current public trust report fixture");
}

#[test]
fn legacy_manifest_and_trust_registry_migrate_to_current_owners() {
    let root = temp_root("adapter-contract-migration");
    let manifest_dir = root.join(".athanor/adapters");
    fs::create_dir_all(&manifest_dir).unwrap();
    let manifest_path = manifest_dir.join("legacy.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&json!({
            "schema": ADAPTER_MANIFEST_SCHEMA_LEGACY,
            "name": "legacy-fixture",
            "adapters": []
        }))
        .unwrap(),
    )
    .unwrap();

    let plugins = discover_adapter_plugins(&root).expect("legacy manifest remains readable");
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].manifest.schema, ADAPTER_MANIFEST_SCHEMA_V1);

    let trust_path = root.join("state/adapter-trust.json");
    fs::create_dir_all(trust_path.parent().unwrap()).unwrap();
    fs::write(
        &trust_path,
        serde_json::to_vec_pretty(&json!({
            "schema": ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2,
            "trusted_plugins": []
        }))
        .unwrap(),
    )
    .unwrap();

    let report = list_adapter_plugin_trust_versioned(AdapterTrustListOptions {
        root: root.clone(),
        trust_path: trust_path.clone(),
    })
    .expect("legacy trust registry remains readable");
    assert_eq!(report.schema, ADAPTER_TRUST_REPORT_SCHEMA_V1);

    let report = trust_adapter_plugin_versioned(AdapterTrustOptions {
        trust_path: trust_path.clone(),
        manifest_path,
    })
    .expect("trust operation writes current registry and report schemas");
    assert_eq!(report.schema, ADAPTER_TRUST_REPORT_SCHEMA_V1);

    let persisted: Value =
        serde_json::from_slice(&fs::read(&trust_path).unwrap()).expect("valid persisted trust JSON");
    assert_eq!(persisted["schema"], ADAPTER_TRUST_REGISTRY_SCHEMA_V2);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn adapter_schema_lifecycle_is_observable_in_runtime_sources() {
    for schema in [
        ADAPTER_MANIFEST_SCHEMA_V1,
        ADAPTER_MANIFEST_SCHEMA_LEGACY,
        ADAPTER_TRUST_REGISTRY_SCHEMA_V2,
        ADAPTER_TRUST_REGISTRY_SCHEMA_LEGACY_V2,
        ADAPTER_TRUST_REPORT_SCHEMA_V1,
    ] {
        assert!(
            CONTRACT_SOURCE.contains(&format!("\"{schema}\"")),
            "adapter schema {schema} is not declared in contract source"
        );
    }
    assert!(DISCOVERY_SOURCE.contains("normalize_adapter_manifest_schema"));
    assert!(TRUST_REGISTRY_SOURCE.contains("normalize_adapter_trust_registry_schema"));
    assert!(TRUST_REGISTRY_SOURCE.contains("ADAPTER_TRUST_REGISTRY_SCHEMA_V2"));
    assert!(!TRUST_REGISTRY_SOURCE.contains("ADAPTER_TRUST_SCHEMA"));
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    std::env::temp_dir().join(format!("athanor-{label}-{nonce}"))
}
