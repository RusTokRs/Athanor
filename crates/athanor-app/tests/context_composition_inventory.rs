const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const ACTIVE_CONTEXT_SOURCE: &str = include_str!("../src/context_composition.rs");
const LEGACY_CONTEXT_SOURCE: &str = include_str!("../src/context.rs");

#[test]
fn context_module_routes_to_the_composition_first_owner() {
    assert!(APP_LIB_SOURCE.contains("#[path = \"context_composition.rs\"]\npub mod context;"));
    assert!(!APP_LIB_SOURCE.contains("\npub mod context;\n"));

    assert!(ACTIVE_CONTEXT_SOURCE.contains("composition.init_store"));
    assert!(ACTIVE_CONTEXT_SOURCE.contains("composition.build_search_index"));
    assert!(ACTIVE_CONTEXT_SOURCE.contains("get_or_build_search_index_with_factory"));
    assert!(!ACTIVE_CONTEXT_SOURCE.contains("crate::store::init_store"));
    assert!(!ACTIVE_CONTEXT_SOURCE.contains("get_or_build_search_index("));
    assert!(!ACTIVE_CONTEXT_SOURCE.contains("Option<&RuntimeComposition>"));
    assert!(!ACTIVE_CONTEXT_SOURCE.contains("match composition"));
}

#[test]
fn quarantined_legacy_context_is_not_the_active_module() {
    assert!(LEGACY_CONTEXT_SOURCE.contains("context_project_inner"));
    assert!(LEGACY_CONTEXT_SOURCE.contains("get_or_build_search_index"));
    assert!(!APP_LIB_SOURCE.contains("#[path = \"context.rs\"]"));
}
