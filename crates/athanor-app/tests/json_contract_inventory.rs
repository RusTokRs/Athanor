use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use athanor_app::{VERSIONED_JSON_CONTRACTS, validate_contract_registry, validate_schema_id};

const INVENTORIED_SOURCE_FILES: &[&str] = &[
    "src/bench.rs",
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
    "src/index_runtime.rs",
    "src/pipeline.rs",
    "src/project_registry.rs",
    "src/rustok_architecture.rs",
    "src/rustok_json_contract.rs",
    "src/validate_changed.rs",
];

const KNOWN_UNREGISTERED_PUBLIC_SCHEMAS: &[&str] = &[];
const KNOWN_PERSISTED_SCHEMAS: &[&str] = &["athanor.project_registry_state.v1"];
const KNOWN_GENERATED_SCHEMAS: &[&str] = &["athanor.validation_result.v1"];
const KNOWN_EMBEDDED_SCHEMAS: &[&str] = &[
    "athanor.index_metrics.v1",
    "athanor.index_report_metrics.v1",
];

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
    let public_migration = schema_set(KNOWN_UNREGISTERED_PUBLIC_SCHEMAS);
    let persisted = schema_set(KNOWN_PERSISTED_SCHEMAS);
    let generated = schema_set(KNOWN_GENERATED_SCHEMAS);
    let embedded = schema_set(KNOWN_EMBEDDED_SCHEMAS);

    for (label, schemas) in [
        ("public migration", &public_migration),
        ("persisted", &persisted),
        ("generated", &generated),
        ("embedded", &embedded),
    ] {
        for schema in schemas {
            validate_schema_id(schema).expect("classified schema ids must remain canonical");
            assert!(
                !registered.contains(schema),
                "{label} schema `{schema}` must not be mixed into the public report registry"
            );
        }
    }

    let classified_sets = [&public_migration, &persisted, &generated, &embedded];
    for (index, left) in classified_sets.iter().enumerate() {
        for right in classified_sets.iter().skip(index + 1) {
            assert!(
                left.is_disjoint(right),
                "JSON schema classifications must remain mutually exclusive"
            );
        }
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut observed_public_migration = BTreeSet::new();
    let mut observed_persisted = BTreeSet::new();
    let mut observed_generated = BTreeSet::new();
    let mut observed_embedded = BTreeSet::new();

    for relative_path in INVENTORIED_SOURCE_FILES {
        let path = manifest_dir.join(relative_path);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

        for schema in extract_schema_ids(&source) {
            if public_migration.contains(schema.as_str()) {
                observed_public_migration.insert(schema.clone());
            }
            if persisted.contains(schema.as_str()) {
                observed_persisted.insert(schema.clone());
            }
            if generated.contains(schema.as_str()) {
                observed_generated.insert(schema.clone());
            }
            if embedded.contains(schema.as_str()) {
                observed_embedded.insert(schema.clone());
            }
            assert!(
                registered.contains(schema.as_str())
                    || public_migration.contains(schema.as_str())
                    || persisted.contains(schema.as_str())
                    || generated.contains(schema.as_str())
                    || embedded.contains(schema.as_str()),
                "schema `{schema}` in `{relative_path}` is neither registered nor explicitly classified"
            );
        }
    }

    assert_observed_matches(
        "public migration",
        observed_public_migration,
        public_migration,
    );
    assert_observed_matches("persisted", observed_persisted, persisted);
    assert_observed_matches("generated", observed_generated, generated);
    assert_observed_matches("embedded", observed_embedded, embedded);
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

fn schema_set(schemas: &[&'static str]) -> BTreeSet<&'static str> {
    schemas.iter().copied().collect()
}

fn assert_observed_matches(
    label: &str,
    observed: BTreeSet<String>,
    expected: BTreeSet<&str>,
) {
    assert_eq!(
        observed,
        expected.into_iter().map(str::to_string).collect(),
        "{label} schema classification and observed literals diverged"
    );
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
