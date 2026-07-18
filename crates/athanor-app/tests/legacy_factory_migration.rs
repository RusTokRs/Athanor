use athanor_app::legacy_factory::{
    LegacyFactoryInstallError, LegacyFactoryUnavailableError,
};

const LEGACY_FACTORY_SOURCE: &str = include_str!("../src/legacy_factory.rs");
const RUNTIME_ROOT_SOURCE: &str = include_str!("../src/runtime.rs");
const RUNTIME_LEGACY_API_SOURCE: &str = include_str!("../src/runtime/legacy_api.rs");
const ADAPTER_REGISTRY_SOURCE: &str = include_str!("../src/runtime/legacy_registry.rs");
const ADAPTER_DISABLED_SOURCE: &str =
    include_str!("../src/runtime/legacy_registry_disabled.rs");
const PROJECTION_SOURCE: &str = include_str!("../src/projection.rs");
const PROJECTION_GLOBAL_SOURCE: &str = include_str!("../src/projection_legacy_global.rs");
const STORE_FACADE_SOURCE: &str = include_str!("../src/store_facade.rs");
const STORE_CORE_SOURCE: &str = include_str!("../src/store.rs");
const STORE_KNOWLEDGE_SOURCE: &str = include_str!("../src/store/knowledge.rs");
const STORE_PUBLICATION_SOURCE: &str = include_str!("../src/store/publication.rs");
const STORE_CANONICAL_SOURCE: &str = include_str!("../src/store/canonical.rs");
const STORE_GLOBAL_SOURCE: &str = include_str!("../src/store_legacy_global.rs");
const SEARCH_FACADE_SOURCE: &str = include_str!("../src/search_facade.rs");
const SEARCH_CORE_SOURCE: &str = include_str!("../src/search.rs");
const SEARCH_MODEL_SOURCE: &str = include_str!("../src/search/model.rs");
const SEARCH_INDEX_SOURCE: &str = include_str!("../src/search/index.rs");
const SEARCH_GLOBAL_SOURCE: &str = include_str!("../src/search_legacy_global.rs");
const APP_CARGO: &str = include_str!("../Cargo.toml");
const RUNTIME_DEFAULTS_CARGO: &str =
    include_str!("../../athanor-runtime-defaults/Cargo.toml");
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
fn legacy_runtime_feature_is_explicit_forwarded_and_exercised() {
    assert!(APP_CARGO.contains("legacy-global-runtime = []"));
    assert!(RUNTIME_DEFAULTS_CARGO.contains(
        "legacy-global-runtime = [\"athanor-app/legacy-global-runtime\"]"
    ));
    assert!(CI_SOURCE.contains("- legacy-global-runtime"));
    assert!(CI_SOURCE.contains("- all-features"));
}

#[test]
fn process_global_storage_is_confined_to_feature_only_owners() {
    for source in [
        RUNTIME_ROOT_SOURCE,
        RUNTIME_LEGACY_API_SOURCE,
        ADAPTER_DISABLED_SOURCE,
        PROJECTION_SOURCE,
        STORE_FACADE_SOURCE,
        STORE_CORE_SOURCE,
        STORE_KNOWLEDGE_SOURCE,
        STORE_PUBLICATION_SOURCE,
        STORE_CANONICAL_SOURCE,
        SEARCH_FACADE_SOURCE,
        SEARCH_CORE_SOURCE,
        SEARCH_MODEL_SOURCE,
        SEARCH_INDEX_SOURCE,
    ] {
        assert!(!source.contains("OnceLock"));
    }

    for source in [
        ADAPTER_REGISTRY_SOURCE,
        PROJECTION_GLOBAL_SOURCE,
        STORE_GLOBAL_SOURCE,
        SEARCH_GLOBAL_SOURCE,
    ] {
        assert!(source.contains("OnceLock"));
    }

    assert!(RUNTIME_ROOT_SOURCE.contains("legacy_registry_disabled.rs"));
    assert!(PROJECTION_SOURCE.contains("projection_legacy_global.rs"));
    assert!(STORE_FACADE_SOURCE.contains("store_legacy_global.rs"));
    assert!(SEARCH_FACADE_SOURCE.contains("search_legacy_global.rs"));
}

#[test]
fn store_and_search_core_owners_have_no_installation_api() {
    for source in [
        STORE_CORE_SOURCE,
        STORE_KNOWLEDGE_SOURCE,
        STORE_PUBLICATION_SOURCE,
    ] {
        assert!(!source.contains("install_store_factory"));
        assert!(!source.contains("pub async fn init_store"));
    }
    assert!(STORE_CORE_SOURCE.contains("store/knowledge.rs"));
    assert!(STORE_CORE_SOURCE.contains("store/publication.rs"));
    assert!(STORE_CORE_SOURCE.contains("store/canonical.rs"));
    assert!(STORE_FACADE_SOURCE.contains("RuntimeComposition::init_store"));

    for source in [
        SEARCH_CORE_SOURCE,
        SEARCH_MODEL_SOURCE,
        SEARCH_INDEX_SOURCE,
    ] {
        assert!(!source.contains("install_search_index_factory"));
        assert!(!source.contains("install_search_index_operation_factory"));
    }
    assert!(SEARCH_CORE_SOURCE.contains("search/index.rs"));
    assert!(SEARCH_CORE_SOURCE.contains("search/model.rs"));
    assert!(SEARCH_INDEX_SOURCE.contains("RuntimeComposition::build_search_index"));
    assert!(SEARCH_FACADE_SOURCE.contains("legacy-global-runtime disabled or not installed"));
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
fn compatibility_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("runtime root", RUNTIME_ROOT_SOURCE, 70),
        ("runtime legacy API", RUNTIME_LEGACY_API_SOURCE, 90),
        ("projection facade", PROJECTION_SOURCE, 130),
        ("projection global", PROJECTION_GLOBAL_SOURCE, 90),
        ("store facade", STORE_FACADE_SOURCE, 90),
        ("store core", STORE_CORE_SOURCE, 220),
        ("store knowledge", STORE_KNOWLEDGE_SOURCE, 230),
        ("store publication", STORE_PUBLICATION_SOURCE, 90),
        ("store canonical", STORE_CANONICAL_SOURCE, 90),
        ("store global", STORE_GLOBAL_SOURCE, 80),
        ("search facade", SEARCH_FACADE_SOURCE, 300),
        ("search core", SEARCH_CORE_SOURCE, 360),
        ("search model", SEARCH_MODEL_SOURCE, 90),
        ("search index", SEARCH_INDEX_SOURCE, 230),
        ("search global", SEARCH_GLOBAL_SOURCE, 90),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}

#[test]
fn compatibility_bootstrap_is_deprecated_and_documented() {
    assert!(MIGRATION_DOC.contains("RuntimeComposition"));
    assert!(MIGRATION_DOC.contains("legacy-global-runtime"));
    assert!(MIGRATION_DOC.contains("COMP-003B1"));
    assert!(MIGRATION_DOC.contains("COMP-003B2"));
    assert!(MIGRATION_DOC.contains("COMP-003C"));
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
