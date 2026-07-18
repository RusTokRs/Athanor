use athanor_app::legacy_factory::{
    LegacyFactoryInstallError, LegacyFactoryUnavailableError,
};

const LEGACY_FACTORY_SOURCE: &str = include_str!("../src/legacy_factory.rs");
const RUNTIME_ROOT_SOURCE: &str = include_str!("../src/runtime.rs");
const ADAPTER_REGISTRY_SOURCE: &str = include_str!("../src/runtime/legacy_registry.rs");
const ADAPTER_DISABLED_SOURCE: &str =
    include_str!("../src/runtime/legacy_registry_disabled.rs");
const PROJECTION_SOURCE: &str = include_str!("../src/projection.rs");
const PROJECTION_GLOBAL_SOURCE: &str = include_str!("../src/projection_legacy_global.rs");
const STORE_FACADE_SOURCE: &str = include_str!("../src/store_facade.rs");
const SEARCH_FACADE_SOURCE: &str = include_str!("../src/search_facade.rs");
const APP_CARGO: &str = include_str!("../Cargo.toml");
const RUNTIME_DEFAULTS_CARGO: &str = include_str!("../../athanor-runtime-defaults/Cargo.toml");
const CI_SOURCE: &str = include_str!("../../../.github/workflows/ci.yml");
const APPLICATION_COMPOSITION_SOURCE: &str =
    include_str!("../src/application_report_composition.rs");
const INDEX_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/index_cli.rs");
const SEARCH_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/direct_search_cli.rs");
const GENERATION_CLI_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_generation_cli.rs");
const MCP_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/mcp_cli.rs");
const MIGRATION_DOC: &str =
    include_str!("../../../docs/development/legacy-runtime-compatibility.md");

#[test]
fn legacy_runtime_feature_is_explicit_and_forwarded() {
    assert!(APP_CARGO.contains("legacy-global-runtime = []"));
    assert!(RUNTIME_DEFAULTS_CARGO.contains(
        "legacy-global-runtime = [\"athanor-app/legacy-global-runtime\"]"
    ));
    assert!(CI_SOURCE.contains("- legacy-global-runtime"));
    assert!(CI_SOURCE.contains("- all-features"));
}

#[test]
fn adapter_and_projector_globals_are_feature_or_test_only() {
    assert!(RUNTIME_ROOT_SOURCE.contains(
        "#[cfg(any(feature = \"legacy-global-runtime\", test))]"
    ));
    assert!(RUNTIME_ROOT_SOURCE.contains("legacy_registry_disabled.rs"));
    assert!(ADAPTER_REGISTRY_SOURCE.contains("OnceLock"));
    assert!(!ADAPTER_DISABLED_SOURCE.contains("OnceLock"));
    assert!(ADAPTER_DISABLED_SOURCE.contains("legacy-global-runtime feature is disabled"));

    assert!(!PROJECTION_SOURCE.contains("OnceLock"));
    assert!(PROJECTION_SOURCE.contains("projection_legacy_global.rs"));
    assert!(PROJECTION_GLOBAL_SOURCE.contains("OnceLock"));
    assert!(PROJECTION_SOURCE.contains("RuntimeComposition::project_wiki"));
    assert!(PROJECTION_SOURCE.contains("RuntimeComposition::project_html"));
}

#[test]
fn store_and_search_remain_the_explicit_comp_003b2_boundary() {
    assert!(STORE_FACADE_SOURCE.contains("STORE_FACTORY_GUARD"));
    assert!(SEARCH_FACADE_SOURCE.contains("SEARCH_INDEX_FACTORY_GUARD"));
    assert!(!STORE_FACADE_SOURCE.contains("SCOPED_STORE_COMPOSITION"));
    assert!(!STORE_FACADE_SOURCE.contains("with_store_composition"));
    assert!(!STORE_FACADE_SOURCE.contains("require_legacy_store_factory"));
    assert!(!SEARCH_FACADE_SOURCE.contains("pub fn require_legacy"));
    assert!(MIGRATION_DOC.contains("COMP-003B2"));
}

#[test]
fn active_entrypoints_use_explicit_runtime_composition() {
    for source in [
        INDEX_CLI_SOURCE,
        SEARCH_CLI_SOURCE,
        GENERATION_CLI_SOURCE,
        MCP_CLI_SOURCE,
    ] {
        assert!(source.contains("athanor_runtime_defaults::production()"));
        assert!(!source.contains("athanor_runtime_defaults::install()"));
    }
    assert!(INDEX_CLI_SOURCE.contains("index_project_with_composition"));
    assert!(SEARCH_CLI_SOURCE.contains("search_project_with_composition"));
    assert!(GENERATION_CLI_SOURCE.contains("generate_project"));
}

#[test]
fn application_report_services_have_no_store_scope_bridge() {
    assert!(APPLICATION_COMPOSITION_SOURCE.contains("api_direct::snapshot"));
    assert!(APPLICATION_COMPOSITION_SOURCE.contains("docs_direct::check"));
    assert!(!APPLICATION_COMPOSITION_SOURCE.contains("with_store_composition"));
    assert!(!APPLICATION_COMPOSITION_SOURCE.contains("crate::store::init_store"));
}

#[test]
fn compatibility_bootstrap_is_deprecated_and_documented() {
    assert!(MIGRATION_DOC.contains("RuntimeComposition"));
    assert!(MIGRATION_DOC.contains("legacy-global-runtime"));
    assert!(MIGRATION_DOC.contains("COMP-003B1"));
    assert!(MIGRATION_DOC.contains("COMP-003B2"));
}

#[test]
fn guarded_factory_failures_are_public_and_actionable() {
    assert!(LEGACY_FACTORY_SOURCE.contains("LegacyFactoryInstallError"));
    let installed = LegacyFactoryInstallError::new("fixture");
    let unavailable = LegacyFactoryUnavailableError::new("fixture");
    assert_eq!(installed.factory(), "fixture");
    assert_eq!(unavailable.factory(), "fixture");
    assert!(installed.to_string().contains("RuntimeComposition"));
    assert!(unavailable.to_string().contains("RuntimeComposition"));
}
