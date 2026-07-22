const RUNTIME_ROOT: &str = include_str!("../src/runtime.rs");
const RUNTIME_MODEL: &str = include_str!("../src/runtime/model.rs");
const RUNTIME_REGISTRY: &str = include_str!("../src/runtime/registry.rs");
const RUNTIME_BUILDER: &str = include_str!("../src/runtime/builder.rs");
const RUNTIME_TRUST: &str = include_str!("../src/runtime/trust.rs");
const RUNTIME_TEST_ROOT: &str = include_str!("../src/runtime/tests.rs");
const RUNTIME_PROCESS_FIXTURES: &str = include_str!("../src/runtime/tests/fixtures.rs");
const PROCESS_SCOPE: &str = include_str!("../src/process_execution_scope.rs");
const ADAPTER_CONTRACT: &str = include_str!("../src/adapter_contract.rs");

#[test]
fn runtime_owner_is_conventional_and_bounded() {
    for module in ["model", "registry", "builder", "trust"] {
        assert!(RUNTIME_ROOT.contains(&format!("mod {module};")));
    }
    assert!(!RUNTIME_ROOT.contains("include!("));
    assert!(RUNTIME_TEST_ROOT.contains("mod process;"));

    for (name, source, max_lines) in [
        ("runtime root", RUNTIME_ROOT, 100),
        ("runtime model", RUNTIME_MODEL, 140),
        ("runtime registry", RUNTIME_REGISTRY, 360),
        ("runtime builder", RUNTIME_BUILDER, 220),
        ("runtime trust", RUNTIME_TRUST, 160),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}

#[test]
fn runtime_trust_api_emits_only_the_public_report_schema() {
    assert!(RUNTIME_TRUST.contains("ADAPTER_TRUST_REPORT_SCHEMA_V1"));
    assert!(!RUNTIME_TRUST.contains("ADAPTER_TRUST_SCHEMA"));
    assert!(!ADAPTER_CONTRACT.contains("impl From<AdapterTrustReport>"));
    assert!(ADAPTER_CONTRACT.contains("list_adapter_plugin_trust(options)"));
}

#[test]
fn legacy_adapter_runtime_aliases_are_explicitly_deprecated() {
    assert_eq!(RUNTIME_ROOT.matches("#[deprecated").count(), 3);
    for replacement in [
        "use adapter_contract::VersionedAdapterTrustReport",
        "ADAPTER_MANIFEST_SCHEMA_V1 for current output",
        "ADAPTER_TRUST_REGISTRY_SCHEMA_V2 for current persistence",
    ] {
        assert!(
            RUNTIME_ROOT.contains(replacement),
            "legacy runtime alias does not name its current replacement: {replacement}"
        );
    }
}

#[test]
fn process_context_and_test_commands_have_one_clear_environment_owner() {
    assert!(!PROCESS_SCOPE.contains("crate::runtime::with_process_cancellation"));
    assert!(!RUNTIME_ROOT.contains("PROCESS_CANCELLATION"));

    let literals = RUNTIME_PROCESS_FIXTURES
        .split("\n    ProcessCommand {")
        .skip(1)
        .collect::<Vec<_>>();
    assert!(!literals.is_empty());
    for literal in literals {
        let body = literal.split("}\n").next().unwrap_or(literal);
        assert_eq!(
            body.matches("clear_environment:").count(),
            1,
            "ProcessCommand test fixture must set clear_environment exactly once"
        );
    }
}
