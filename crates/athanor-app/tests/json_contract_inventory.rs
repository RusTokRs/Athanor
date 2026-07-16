use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use athanor_app::{VERSIONED_JSON_CONTRACTS, validate_contract_registry, validate_schema_id};

const AGENT_FACING_SOURCE_FILES: &[&str] = &[
    "src/overview.rs",
    "src/search.rs",
    "src/explain.rs",
    "src/impact.rs",
    "src/check.rs",
    "src/coverage.rs",
    "src/capabilities.rs",
    "src/change_map.rs",
    "src/context.rs",
    "src/context_report.rs",
    "src/graph.rs",
    "src/project_registry.rs",
    "src/rustok_json_contract.rs",
];

const KNOWN_UNREGISTERED_AGENT_FACING_SCHEMAS: &[&str] = &[];

const KNOWN_PERSISTED_SCHEMAS: &[&str] = &["athanor.project_registry_state.v1"];

const MIGRATED_SHARED_SCHEMA_BUILDERS: &[(&str, &str)] = &[
    ("src/search.rs", "athanor.search.v1"),
    ("src/impact.rs", "athanor.impact_analysis.v1"),
    ("src/check.rs", "athanor.diagnostic_check.v1"),
    ("src/check.rs", "athanor.affected_check.v1"),
    ("src/check.rs", "athanor.operations_docs_check.v1"),
    ("src/change_map.rs", "athanor.change_map.v1"),
];

#[test]
fn known_schema_literals_are_registered_or_explicitly_classified() {
    validate_contract_registry(VERSIONED_JSON_CONTRACTS)
        .expect("shared JSON contract registry must remain valid");

    let registered = VERSIONED_JSON_CONTRACTS
        .iter()
        .map(|contract| contract.schema)
        .collect::<BTreeSet<_>>();
    let tracked = KNOWN_UNREGISTERED_AGENT_FACING_SCHEMAS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let persisted = KNOWN_PERSISTED_SCHEMAS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    for schema in &tracked {
        validate_schema_id(schema).expect("tracked schema ids must remain canonical");
        assert!(
            !registered.contains(schema),
            "tracked schema `{schema}` is registered now; remove it from the migration allowlist"
        );
    }
    for schema in &persisted {
        validate_schema_id(schema).expect("persisted schema ids must remain canonical");
        assert!(
            !registered.contains(schema),
            "persisted schema `{schema}` must not be mixed into the public report registry"
        );
        assert!(
            !tracked.contains(schema),
            "persisted schema `{schema}` must not also be a migration exception"
        );
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut observed_tracked = BTreeSet::new();
    let mut observed_persisted = BTreeSet::new();

    for relative_path in AGENT_FACING_SOURCE_FILES {
        let path = manifest_dir.join(relative_path);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

        for schema in extract_schema_ids(&source) {
            if tracked.contains(schema.as_str()) {
                observed_tracked.insert(schema.clone());
            }
            if persisted.contains(schema.as_str()) {
                observed_persisted.insert(schema.clone());
            }
            assert!(
                registered.contains(schema.as_str())
                    || tracked.contains(schema.as_str())
                    || persisted.contains(schema.as_str()),
                "schema `{schema}` in `{relative_path}` is neither registered nor explicitly classified"
            );
        }
    }

    assert_eq!(
        observed_tracked,
        tracked.into_iter().map(str::to_string).collect(),
        "migration allowlist and observed agent-facing schema literals diverged"
    );
    assert_eq!(
        observed_persisted,
        persisted.into_iter().map(str::to_string).collect(),
        "persisted schema classification and observed literals diverged"
    );
}

#[test]
fn migrated_builders_use_shared_registry_constants() {
    let registered = VERSIONED_JSON_CONTRACTS
        .iter()
        .map(|contract| contract.schema)
        .collect::<BTreeSet<_>>();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    for (relative_path, schema) in MIGRATED_SHARED_SCHEMA_BUILDERS {
        assert!(
            registered.contains(schema),
            "migrated builder schema `{schema}` must remain registered"
        );
        let path = manifest_dir.join(relative_path);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let quoted_literal = format!("\"{schema}\"");
        assert!(
            !source.contains(&quoted_literal),
            "migrated builder `{relative_path}` embeds schema literal `{schema}` instead of using the shared registry constant"
        );
    }
}

fn extract_schema_ids(source: &str) -> BTreeSet<String> {
    let mut schemas = BTreeSet::new();
    let mut offset = 0;

    while let Some(relative_start) = source[offset..].find("athanor.") {
        let start = offset + relative_start;
        let tail = &source[start..];
        let end = tail
            .find(|character: char| {
                !(character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '.' | '_' | '-'))
            })
            .unwrap_or(tail.len());
        let candidate = &tail[..end];

        if validate_schema_id(candidate).is_ok() {
            schemas.insert(candidate.to_string());
        }

        offset = start + end.max(1);
    }

    schemas
}
