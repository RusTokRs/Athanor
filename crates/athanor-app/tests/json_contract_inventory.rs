use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use athanor_app::{
    ADAPTER_NON_PUBLIC_JSON_CONTRACTS, AUTOMATION_JSON_CONTRACTS, BoundaryLifecycle,
    NON_PUBLIC_JSON_CONTRACTS, VERSIONED_JSON_CONTRACTS, validate_adapter_contract_inventory,
    validate_automation_contract_inventory, validate_boundary_contract_inventory,
    validate_contract_registry, validate_schema_id,
};

#[test]
fn schema_registries_are_valid_unique_and_disjoint() {
    validate_contract_registry(VERSIONED_JSON_CONTRACTS)
        .expect("public JSON contract registry must remain valid");
    validate_boundary_contract_inventory(VERSIONED_JSON_CONTRACTS)
        .expect("non-public JSON boundary registry must remain valid");
    validate_adapter_contract_inventory(VERSIONED_JSON_CONTRACTS)
        .expect("adapter JSON boundary registry must remain valid");
    validate_automation_contract_inventory(
        VERSIONED_JSON_CONTRACTS,
        NON_PUBLIC_JSON_CONTRACTS,
        ADAPTER_NON_PUBLIC_JSON_CONTRACTS,
    )
    .expect("automation JSON contract registry must remain valid");

    assert_eq!(VERSIONED_JSON_CONTRACTS.len(), 60);
    assert_eq!(NON_PUBLIC_JSON_CONTRACTS.len(), 30);
    assert_eq!(ADAPTER_NON_PUBLIC_JSON_CONTRACTS.len(), 4);
    assert_eq!(AUTOMATION_JSON_CONTRACTS.len(), 1);

    let public = VERSIONED_JSON_CONTRACTS
        .iter()
        .map(|contract| contract.schema)
        .collect::<BTreeSet<_>>();
    let general = NON_PUBLIC_JSON_CONTRACTS
        .iter()
        .map(|contract| contract.schema)
        .collect::<BTreeSet<_>>();
    let adapter = ADAPTER_NON_PUBLIC_JSON_CONTRACTS
        .iter()
        .map(|contract| contract.schema)
        .collect::<BTreeSet<_>>();
    let automation = AUTOMATION_JSON_CONTRACTS
        .iter()
        .map(|contract| contract.schema)
        .collect::<BTreeSet<_>>();

    assert!(public.is_disjoint(&general));
    assert!(public.is_disjoint(&adapter));
    assert!(public.is_disjoint(&automation));
    assert!(general.is_disjoint(&adapter));
    assert!(general.is_disjoint(&automation));
    assert!(adapter.is_disjoint(&automation));

    for schema in public {
        validate_schema_id(schema).expect("public schema ids must remain canonical");
    }
    for descriptor in NON_PUBLIC_JSON_CONTRACTS {
        validate_schema_id(descriptor.schema).unwrap_or_else(|error| {
            panic!(
                "general boundary schema {} is not canonical: {error}",
                descriptor.schema
            )
        });
    }
    for descriptor in ADAPTER_NON_PUBLIC_JSON_CONTRACTS {
        if descriptor.lifecycle == BoundaryLifecycle::Current {
            validate_schema_id(descriptor.schema).unwrap_or_else(|error| {
                panic!(
                    "current adapter schema {} is not canonical: {error}",
                    descriptor.schema
                )
            });
        }
    }
    for descriptor in AUTOMATION_JSON_CONTRACTS {
        if descriptor.lifecycle == BoundaryLifecycle::Current {
            validate_schema_id(descriptor.schema).unwrap_or_else(|error| {
                panic!(
                    "current automation schema {} is not canonical: {error}",
                    descriptor.schema
                )
            });
        }
    }
}

#[test]
fn every_workspace_production_schema_literal_is_explicitly_classified() {
    let workspace = workspace_root();
    let sources = production_rust_sources(&workspace);
    assert!(
        !sources.is_empty(),
        "workspace source discovery returned no Rust files"
    );

    let classified = classified_schemas();
    let mut observed = BTreeMap::<String, BTreeSet<PathBuf>>::new();
    for path in sources {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for schema in extract_schema_literals(production_prefix(&source)) {
            observed
                .entry(schema)
                .or_default()
                .insert(path.strip_prefix(&workspace).unwrap_or(&path).to_path_buf());
        }
    }

    let unknown = observed
        .iter()
        .filter(|(schema, _)| !classified.contains(schema.as_str()))
        .map(|(schema, paths)| format!("{schema}: {paths:?}"))
        .collect::<Vec<_>>();
    assert!(
        unknown.is_empty(),
        "unclassified production JSON schema literals:\n{}",
        unknown.join("\n")
    );

    let missing = classified
        .iter()
        .filter(|schema| !observed.contains_key(**schema))
        .copied()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "classified schemas are no longer observable in production sources: {missing:?}"
    );
}

#[test]
fn qualified_feature_schema_is_versioned_without_changing_its_wire_id() {
    assert_eq!(
        validate_schema_id("athanor.index_state.v46-js-ts-precision-v1"),
        Ok(46)
    );

    for invalid in [
        "athanor.index_state.v46-js-ts-precision",
        "athanor.index_state.v46--v1",
        "athanor.index_state.v46-js-ts-precision-v0",
        "athanor.index_state.v0-js-ts-precision-v1",
    ] {
        assert!(
            validate_schema_id(invalid).is_err(),
            "invalid qualified schema was accepted: {invalid}"
        );
    }
}

fn classified_schemas() -> BTreeSet<&'static str> {
    VERSIONED_JSON_CONTRACTS
        .iter()
        .map(|contract| contract.schema)
        .chain(
            NON_PUBLIC_JSON_CONTRACTS
                .iter()
                .map(|contract| contract.schema),
        )
        .chain(
            ADAPTER_NON_PUBLIC_JSON_CONTRACTS
                .iter()
                .map(|contract| contract.schema),
        )
        .chain(
            AUTOMATION_JSON_CONTRACTS
                .iter()
                .map(|contract| contract.schema),
        )
        .collect()
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("athanor-app must be under <workspace>/crates")
        .to_path_buf()
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
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => panic!(
            "failed to read source directory {}: {error}",
            path.display()
        ),
    };
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to inspect source directory {}: {error}",
                path.display()
            )
        });
        let child = entry.path();
        let file_type = entry.file_type().unwrap_or_else(|error| {
            panic!("failed to inspect source path {}: {error}", child.display())
        });
        if file_type.is_dir() {
            if child.file_name().and_then(|name| name.to_str()) != Some("target") {
                collect_rust_sources(&child, sources);
            }
            continue;
        }
        if !file_type.is_file() || child.extension().and_then(OsStr::to_str) != Some("rs") {
            continue;
        }
        let stem = child
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
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

fn production_prefix(source: &str) -> &str {
    source
        .find("#[cfg(test)]\nmod tests")
        .map(|offset| &source[..offset])
        .unwrap_or(source)
}

fn extract_schema_literals(source: &str) -> BTreeSet<String> {
    const MARKER: &str = "\"athanor.";
    let mut schemas = BTreeSet::new();
    let mut offset = 0;

    while let Some(relative_start) = source[offset..].find(MARKER) {
        let start = offset + relative_start + 1;
        let tail = &source[start..];
        let Some(end) = tail.find('"') else {
            break;
        };
        let candidate = &tail[..end];
        if candidate.len() > "athanor.".len()
            && candidate.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '.' | '_' | '-')
            })
        {
            schemas.insert(candidate.to_string());
        }
        offset = start + end + 1;
    }

    schemas
}
