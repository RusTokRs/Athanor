use athanor_app::legacy_factory::{
    LegacyFactoryInstallError, LegacyFactoryUnavailableError,
};

const LEGACY_FACTORY_SOURCE: &str = include_str!("../src/legacy_factory.rs");
const ADAPTER_REGISTRY_SOURCE: &str = include_str!("../src/runtime/legacy_registry.rs");
const PROJECTION_SOURCE: &str = include_str!("../src/projection.rs");
const STORE_SOURCE: &str = include_str!("../src/store.rs");
const SEARCH_SOURCE: &str = include_str!("../src/search.rs");

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
fn remaining_legacy_factory_targets_stay_visible_until_migrated() {
    assert!(STORE_SOURCE.contains("let _ = STORE_FACTORY.set(factory)"));
    assert!(SEARCH_SOURCE.contains("let _ = SEARCH_INDEX_FACTORY.set(factory)"));
    assert!(SEARCH_SOURCE.contains("let _ = SEARCH_INDEX_OPERATION_FACTORY.set(factory)"));
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
