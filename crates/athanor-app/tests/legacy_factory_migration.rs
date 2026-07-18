use athanor_app::legacy_factory::{
    LegacyFactoryInstallError, LegacyFactoryUnavailableError,
};

const LEGACY_FACTORY_SOURCE: &str = include_str!("../src/legacy_factory.rs");
const ADAPTER_REGISTRY_SOURCE: &str = include_str!("../src/runtime/legacy_registry.rs");
const PROJECTION_SOURCE: &str = include_str!("../src/projection.rs");
const STORE_FACADE_SOURCE: &str = include_str!("../src/store_facade.rs");
const SEARCH_FACADE_SOURCE: &str = include_str!("../src/search_facade.rs");
const RUNTIME_DEFAULTS_SOURCE: &str =
    include_str!("../../athanor-runtime-defaults/src/lib.rs");
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
fn process_global_state_is_confined_to_known_compatibility_owners() {
    assert!(LEGACY_FACTORY_SOURCE.contains("install_once"));
    assert!(ADAPTER_REGISTRY_SOURCE.contains("OnceLock"));
    assert!(PROJECTION_SOURCE.contains("OnceLock"));
    assert!(STORE_FACADE_SOURCE.contains("STORE_FACTORY_GUARD"));
    assert!(SEARCH_FACADE_SOURCE.contains("SEARCH_INDEX_FACTORY_GUARD"));

    assert!(!STORE_FACADE_SOURCE.contains("SCOPED_STORE_COMPOSITION"));
    assert!(!STORE_FACADE_SOURCE.contains("with_store_composition"));
    assert!(!STORE_FACADE_SOURCE.contains("require_legacy_store_factory"));
    assert!(!SEARCH_FACADE_SOURCE.contains("pub fn require_legacy"));
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
    assert!(RUNTIME_DEFAULTS_SOURCE.contains("Legacy process-global installation"));
    assert!(RUNTIME_DEFAULTS_SOURCE.contains("#[deprecated"));
    assert!(MIGRATION_DOC.contains("RuntimeComposition"));
    assert!(MIGRATION_DOC.contains("COMP-003B"));
}

#[test]
fn guarded_factory_failures_are_public_and_actionable() {
    let installed = LegacyFactoryInstallError::new("fixture");
    let unavailable = LegacyFactoryUnavailableError::new("fixture");
    assert_eq!(installed.factory(), "fixture");
    assert_eq!(unavailable.factory(), "fixture");
    assert!(installed.to_string().contains("RuntimeComposition"));
    assert!(unavailable.to_string().contains("RuntimeComposition"));
}
