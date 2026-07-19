use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    BoundaryLifecycle, NON_PUBLIC_JSON_CONTRACTS, PROCESS_ADAPTER_FRAMING,
    PROCESS_PROTOCOL_CONTRACTS, VERSIONED_JSON_CONTRACTS, validate_boundary_contract_inventory,
    validate_non_public_contract_value, validate_process_protocol_value,
};
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("fixtures/boundary_contracts.v1.json");

#[test]
fn non_public_json_boundaries_are_disjoint_and_fixture_protected() {
    validate_boundary_contract_inventory(VERSIONED_JSON_CONTRACTS)
        .expect("process/persistence boundary registry must remain valid");
    assert_eq!(NON_PUBLIC_JSON_CONTRACTS.len(), 30);

    let fixture: Value = serde_json::from_str(FIXTURE).expect("valid boundary fixture");
    let documents = fixture["documents"]
        .as_object()
        .expect("boundary documents object");
    let mut current = 0;
    let mut legacy_input = 0;
    let mut historical = 0;

    for descriptor in NON_PUBLIC_JSON_CONTRACTS {
        match descriptor.lifecycle {
            BoundaryLifecycle::Current => {
                current += 1;
                let value = documents.get(descriptor.schema).unwrap_or_else(|| {
                    panic!("missing current boundary fixture {}", descriptor.schema)
                });
                validate_non_public_contract_value(descriptor, value).unwrap_or_else(|error| {
                    panic!("invalid boundary fixture {}: {error}", descriptor.schema)
                });
            }
            BoundaryLifecycle::LegacyInput => {
                legacy_input += 1;
                assert!(
                    !documents.contains_key(descriptor.schema),
                    "legacy input {} must not be emitted as a current fixture",
                    descriptor.schema
                );
            }
            BoundaryLifecycle::Historical => {
                historical += 1;
                assert!(
                    !documents.contains_key(descriptor.schema),
                    "historical schema {} must not be emitted as a current fixture",
                    descriptor.schema
                );
            }
        }
    }

    assert_eq!(current, 24);
    assert_eq!(legacy_input, 5);
    assert_eq!(historical, 1);
    assert_eq!(documents.len(), current);
}

#[test]
fn empty_index_state_may_omit_derived_generation() {
    for schema in [
        "athanor.index_state.v46",
        "athanor.index_state.v46-js-ts-precision-v1",
    ] {
        let descriptor = NON_PUBLIC_JSON_CONTRACTS
            .iter()
            .find(|descriptor| descriptor.schema == schema)
            .expect("index-state boundary descriptor");
        validate_non_public_contract_value(
            descriptor,
            &json!({ "schema": schema, "snapshot": null, "files": {} }),
        )
        .expect("empty current index state may omit derived generation");
    }
}

#[test]
fn process_protocols_are_schema_less_typed_and_fixture_protected() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("valid boundary fixture");
    let protocols = fixture["process_protocols"]
        .as_object()
        .expect("process protocol fixture object");

    assert_eq!(PROCESS_PROTOCOL_CONTRACTS.len(), 4);
    assert_eq!(protocols.len(), PROCESS_PROTOCOL_CONTRACTS.len());
    for descriptor in PROCESS_PROTOCOL_CONTRACTS {
        assert_eq!(descriptor.framing, PROCESS_ADAPTER_FRAMING);
        let fixture = protocols
            .get(descriptor.name)
            .unwrap_or_else(|| panic!("missing process protocol fixture {}", descriptor.name));
        validate_process_protocol_value(descriptor, &fixture["request"], &fixture["response"])
            .unwrap_or_else(|error| panic!("invalid process fixture {}: {error}", descriptor.name));
        assert!(
            fixture["request"].get("schema").is_none(),
            "schema-less process request {} gained an Athanor schema",
            descriptor.name
        );
        assert!(
            fixture["response"].get("schema").is_none(),
            "schema-less process response {} gained an Athanor schema",
            descriptor.name
        );
    }
}

#[test]
fn inventory_schemas_and_process_framing_are_observable_in_runtime_sources() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("athanor-app must be under <workspace>/crates");
    let schema_sources = production_rust_sources(workspace)
        .into_iter()
        .map(read_source)
        .collect::<Vec<_>>()
        .join("\n");

    for descriptor in NON_PUBLIC_JSON_CONTRACTS {
        let literal = format!("\"{}\"", descriptor.schema);
        assert!(
            schema_sources.contains(&literal),
            "boundary schema {} is not observable in runtime source",
            descriptor.schema
        );
    }

    let endpoint_reader = read_source(manifest_dir.join("src/daemon_endpoint.rs"));
    assert!(endpoint_reader.contains("DAEMON_ENDPOINT_SCHEMA_V2"));
    assert!(
        !endpoint_reader.contains("DAEMON_ENDPOINT_SCHEMA_V1"),
        "historical daemon endpoint v1 unexpectedly became accepted input"
    );

    let process_source = read_source(manifest_dir.join("src/runtime/process_adapter.rs"));
    for descriptor in PROCESS_PROTOCOL_CONTRACTS {
        assert!(
            process_source.contains(descriptor.request_type),
            "process request type {} is not used by runtime",
            descriptor.request_type
        );
        let response_type = descriptor
            .response_type
            .trim_start_matches("Vec<")
            .trim_end_matches('>');
        assert!(
            process_source.contains(response_type),
            "process response type {} is not used by runtime",
            descriptor.response_type
        );
    }
    assert!(process_source.contains("input.push(b'\\n')"));
    assert!(process_source.contains("serde_json::from_slice(&output.stdout)"));
}

fn production_rust_sources(workspace: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    for root in [workspace.join("crates"), workspace.join("apps")] {
        collect_rust_sources(&root, &mut sources);
    }
    sources.sort();
    sources
}

fn collect_rust_sources(path: &Path, sources: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    for entry in entries {
        let entry = entry
            .unwrap_or_else(|error| panic!("failed to inspect {}: {error}", path.display()));
        let child = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!("failed to inspect {}: {error}", child.display())
        });
        if file_type.is_dir() {
            if child.file_name().and_then(OsStr::to_str) != Some("target") {
                collect_rust_sources(&child, sources);
            }
            continue;
        }
        if !file_type.is_file() || child.extension().and_then(OsStr::to_str) != Some("rs") {
            continue;
        }
        let stem = child.file_stem().and_then(OsStr::to_str).unwrap_or_default();
        if stem == "tests" || stem.ends_with("_test") || stem.ends_with("_tests") {
            continue;
        }
        if child
            .components()
            .any(|component| component.as_os_str() == OsStr::new("src"))
        {
            sources.push(child);
        }
    }
}

fn read_source(path: PathBuf) -> String {
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
