use athanor_app::legacy_factory::{
    LegacyFactoryInstallError, LegacyFactoryUnavailableError,
};

const LEGACY_FACTORY_SOURCE: &str = include_str!("../src/legacy_factory.rs");
const ADAPTER_REGISTRY_SOURCE: &str = include_str!("../src/runtime/legacy_registry.rs");
const PROJECTION_SOURCE: &str = include_str!("../src/projection.rs");
const STORE_FACADE_SOURCE: &str = include_str!("../src/store_facade.rs");
const SEARCH_FACADE_SOURCE: &str = include_str!("../src/search_facade.rs");
const GRAPH_OPERATION_SOURCE: &str = include_str!("../src/graph_operation.rs");
const RUSTOK_COMPOSITION_SOURCE: &str = include_str!("../src/rustok_composition_operation.rs");
const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const VALIDATE_CHANGED_SOURCE: &str = include_str!("../src/validate_changed.rs");
const CLI_ENTRY_SOURCE: &str = include_str!("../../../apps/ath/src/entry.rs");
const DIRECT_SEARCH_SOURCE: &str = include_str!("../../../apps/ath/src/direct_search_cli.rs");
const DIRECT_CONTEXT_SOURCE: &str = include_str!("../../../apps/ath/src/direct_context_cli.rs");
const DIRECT_CHECK_SOURCE: &str = include_str!("../../../apps/ath/src/direct_check_cli.rs");
const DIRECT_GRAPH_SOURCE: &str = include_str!("../../../apps/ath/src/direct_graph_cli.rs");
const DIRECT_RUSTOK_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_rustok_composed_cli.rs");
const DIRECT_GENERATION_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_generation_cli.rs");
const DIRECT_VALIDATE_CHANGED_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_validate_changed_cli.rs");

#[test]
fn migrated_legacy_factories_fail_explicitly() {
    assert!(LEGACY_FACTORY_SOURCE.contains("LegacyFactoryInstallError"));
    assert!(LEGACY_FACTORY_SOURCE.contains("LegacyFactoryUnavailableError"));
    assert!(ADAPTER_REGISTRY_SOURCE.contains("try_install_default_adapter_registry"));
    assert!(ADAPTER_REGISTRY_SOURCE.contains("try_default_adapter_registry"));
    assert!(!ADAPTER_REGISTRY_SOURCE.contains("unwrap_or_else(AdapterRegistry::empty)"));
    assert!(PROJECTION_SOURCE.contains("try_install_wiki_projector_factory"));
    assert!(PROJECTION_SOURCE.contains("try_install_html_projector_factory"));
    assert!(!PROJECTION_SOURCE.contains("let _ = WIKI_PROJECTOR_FACTORY.set"));
    assert!(!PROJECTION_SOURCE.contains("let _ = HTML_PROJECTOR_FACTORY.set"));
}

#[test]
fn store_and_search_globals_are_quarantined_behind_guarded_facades() {
    assert!(STORE_FACADE_SOURCE.contains("try_install_store_factory"));
    assert!(STORE_FACADE_SOURCE.contains("require_installed(&STORE_FACTORY_GUARD"));
    assert!(!STORE_FACADE_SOURCE.contains("let _ = STORE_FACTORY"));

    assert!(SEARCH_FACADE_SOURCE.contains("try_install_search_index_factory"));
    assert!(SEARCH_FACADE_SOURCE.contains("try_install_search_index_operation_factory"));
    assert!(SEARCH_FACADE_SOURCE.contains("require_any_search_factory"));
    assert!(!SEARCH_FACADE_SOURCE.contains("let _ = SEARCH_INDEX_FACTORY"));
    assert!(!SEARCH_FACADE_SOURCE.contains("let _ = SEARCH_INDEX_OPERATION_FACTORY"));

    assert!(APP_LIB_SOURCE.contains("#[path = \"store_facade.rs\"]"));
    assert!(APP_LIB_SOURCE.contains("#[path = \"search_facade.rs\"]"));
}

#[test]
fn changed_validation_has_an_explicit_composition_path() {
    assert!(VALIDATE_CHANGED_SOURCE.contains("validate_changed_with_composition"));
    assert!(VALIDATE_CHANGED_SOURCE.contains("RuntimeBuilder::from_composition"));
    assert!(CLI_ENTRY_SOURCE.contains("direct_validate_changed_cli"));
    assert!(CLI_ENTRY_SOURCE.contains("Athanor direct changed validation runtime"));
    assert!(DIRECT_VALIDATE_CHANGED_SOURCE.contains("athanor_runtime_defaults::production()"));
    assert!(!DIRECT_VALIDATE_CHANGED_SOURCE.contains("athanor_runtime_defaults::install()"));
}

#[test]
fn graph_operations_have_an_explicit_composition_path() {
    assert!(GRAPH_OPERATION_SOURCE.contains("export_graph_with_composition_and_operation_context"));
    assert!(GRAPH_OPERATION_SOURCE.contains("related_graph_with_composition_and_operation_context"));
    assert!(GRAPH_OPERATION_SOURCE.contains("shortest_graph_path_with_composition_and_operation_context"));
    assert!(GRAPH_OPERATION_SOURCE.contains("graph_hubs_with_composition_and_operation_context"));
    assert!(GRAPH_OPERATION_SOURCE.contains("graph_pagerank_with_composition_and_operation_context"));
    assert!(GRAPH_OPERATION_SOURCE.contains("graph_cycles_with_composition_and_operation_context"));
}

#[test]
fn rustok_operations_have_an_explicit_composition_path() {
    for operation in [
        "rustok_architecture_context_with_composition_and_operation_context",
        "rustok_ffa_audit_with_composition_and_operation_context",
        "rustok_fba_audit_with_composition_and_operation_context",
        "rustok_page_builder_audit_with_composition_and_operation_context",
        "graph_ffa_surface_with_composition_and_operation_context",
        "graph_ffa_violations_with_composition_and_operation_context",
        "graph_fba_module_with_composition_and_operation_context",
        "graph_fba_port_with_composition_and_operation_context",
        "graph_fba_dependencies_with_composition_and_operation_context",
        "graph_fba_violations_with_composition_and_operation_context",
        "graph_page_builder_provider_with_composition_and_operation_context",
        "graph_page_builder_consumer_with_composition_and_operation_context",
        "graph_page_builder_violations_with_composition_and_operation_context",
    ] {
        assert!(RUSTOK_COMPOSITION_SOURCE.contains(operation));
    }
    assert!(RUSTOK_COMPOSITION_SOURCE.contains("composition.init_store"));
    assert!(RUSTOK_COMPOSITION_SOURCE.contains(
        "context_project_with_composition_and_operation_context"
    ));
    assert!(!RUSTOK_COMPOSITION_SOURCE.contains("crate::store::init_store"));
}

#[test]
fn focused_composition_reads_do_not_install_global_runtime() {
    for source in [
        DIRECT_SEARCH_SOURCE,
        DIRECT_CONTEXT_SOURCE,
        DIRECT_CHECK_SOURCE,
        DIRECT_GRAPH_SOURCE,
        DIRECT_RUSTOK_SOURCE,
        DIRECT_GENERATION_SOURCE,
    ] {
        assert!(source.contains("athanor_runtime_defaults::production()"));
        assert!(!source.contains("athanor_runtime_defaults::install()"));
    }
    assert!(CLI_ENTRY_SOURCE.contains("direct_rustok_composed_cli"));
    assert!(!CLI_ENTRY_SOURCE.contains("mod direct_rustok_cli;"));
}

#[test]
fn legacy_factory_errors_are_public_and_actionable() {
    let installed = LegacyFactoryInstallError::new("fixture");
    let unavailable = LegacyFactoryUnavailableError::new("fixture");
    assert_eq!(installed.factory(), "fixture");
    assert_eq!(unavailable.factory(), "fixture");
    assert!(installed.to_string().contains("RuntimeComposition"));
    assert!(unavailable.to_string().contains("RuntimeComposition"));
}
