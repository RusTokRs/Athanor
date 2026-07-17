use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    BoundaryLifecycle, NON_PUBLIC_JSON_CONTRACTS, PROCESS_ADAPTER_FRAMING,
    PROCESS_PROTOCOL_CONTRACTS, VERSIONED_JSON_CONTRACTS, validate_boundary_contract_inventory,
    validate_non_public_contract_value, validate_process_protocol_value,
};
use serde_json::Value;

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
    let schema_sources = [
        app_source(manifest_dir, "src/project_registry.rs"),
        app_source(manifest_dir, "src/daemon.rs"),
        app_source(manifest_dir, "src/index_current.rs"),
        app_source(manifest_dir, "src/index_state.rs"),
        app_source(manifest_dir, "src/index_publication_journal.rs"),
        app_source(manifest_dir, "src/repair_pointer.rs"),
        app_source(manifest_dir, "src/index_runtime.rs"),
        app_source(manifest_dir, "src/generation.rs"),
        app_source(manifest_dir, "src/api.rs"),
        app_source(manifest_dir, "src/read_model.rs"),
        app_source(manifest_dir, "src/docs.rs"),
        app_source(manifest_dir, "src/projection.rs"),
        app_source(manifest_dir, "src/pipeline.rs"),
        workspace_source(manifest_dir, "athanor-store-jsonl/src/lib.rs"),
        workspace_source(manifest_dir, "athanor-store-jsonl/src/store.rs"),
        workspace_source(manifest_dir, "athanor-store-jsonl/src/atomic_publication.rs"),
        workspace_source(manifest_dir, "athanor-projector-wiki/src/lib.rs"),
        workspace_source(manifest_dir, "athanor-projector-html/src/lib.rs"),
    ]
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

    let process_source = read_source(app_source(
        manifest_dir,
        "src/runtime/process_adapter.rs",
    ));
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

fn app_source(manifest_dir: &Path, relative: &str) -> PathBuf {
    manifest_dir.join(relative)
}

fn workspace_source(manifest_dir: &Path, relative: &str) -> PathBuf {
    manifest_dir
        .parent()
        .expect("athanor-app crate parent")
        .join(relative)
}

fn read_source(path: PathBuf) -> String {
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
