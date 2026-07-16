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
    "src/graph.rs",
    "src/context.rs",
    "src/project_registry.rs",
];

const KNOWN_UNREGISTERED_AGENT_FACING_SCHEMAS: &[&str] = &[
    "athanor.context_pack.v1",
    "athanor.project_registry.v1",
    "athanor.rustok_ffa_audit.v1",
    "athanor.rustok_ffa_surface_graph.v1",
    "athanor.rustok_ffa_violations_graph.v1",
    "athanor.rustok_fba_audit.v1",
    "athanor.rustok_fba_module_graph.v1",
    "athanor.rustok_fba_port_graph.v1",
    "athanor.rustok_fba_dependencies_graph.v1",
    "athanor.rustok_fba_violations_graph.v1",
    "athanor.rustok_page_builder_audit.v1",
    "athanor.rustok_page_builder_provider_graph.v1",
    "athanor.rustok_page_builder_consumer_graph.v1",
    "athanor.rustok_page_builder_violations_graph.v1",
];

const MIGRATED_SHARED_SCHEMA_BUILDERS: &[(&str, &str)] = &[
    ("src/search.rs", "athanor.search.v1"),
    ("src/impact.rs", "athanor.impact_analysis.v1"),
];

#[test]
fn known_agent_facing_schema_literals_are_registered_or_explicitly_tracked() {
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

    for schema in &tracked {
        validate_schema_id(schema).expect("tracked schema ids must remain canonical");
        assert!(
            !registered.contains(schema),
            "tracked schema `{schema}` is registered now; remove it from the migration allowlist"
        );
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut observed_tracked = BTreeSet::new();

    for relative_path in AGENT_FACING_SOURCE_FILES {
        let path = manifest_dir.join(relative_path);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

        for schema in extract_schema_ids(&source) {
            if tracked.contains(schema.as_str()) {
                observed_tracked.insert(schema.clone());
            }
            assert!(
                registered.contains(schema.as_str()) || tracked.contains(schema.as_str()),
                "agent-facing schema `{schema}` in `{relative_path}` is neither registered nor explicitly tracked"
            );
        }
    }

    assert_eq!(
        observed_tracked,
        tracked.into_iter().map(str::to_string).collect(),
        "migration allowlist and observed agent-facing schema literals diverged"
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
