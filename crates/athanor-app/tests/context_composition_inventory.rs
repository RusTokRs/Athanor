use std::path::Path;

const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const ACTIVE_CONTEXT_SOURCE: &str = include_str!("../src/context_composition.rs");
const CONTEXT_OPERATION_SOURCE: &str = include_str!("../src/context_operation.rs");
const DERIVED_READ_SOURCE: &str = include_str!("../src/derived_read_operation.rs");
const SEARCH_OPERATION_SOURCE: &str = include_str!("../src/search_operation.rs");
const RUSTOK_ARCHITECTURE_SOURCE: &str = include_str!("../src/rustok_architecture.rs");
const RUSTOK_COMPOSITION_SOURCE: &str = include_str!("../src/rustok_composition_operation.rs");

#[test]
fn context_module_routes_to_the_composition_first_owner() {
    assert!(APP_LIB_SOURCE.contains("#[path = \"context_composition.rs\"]\npub mod context;"));
    assert_eq!(APP_LIB_SOURCE.matches("pub mod context;").count(), 1);

    assert!(ACTIVE_CONTEXT_SOURCE.contains("composition.init_store"));
    assert!(ACTIVE_CONTEXT_SOURCE.contains("composition.build_search_index"));
    assert!(ACTIVE_CONTEXT_SOURCE.contains("get_or_build_search_index_with_factory"));
    assert!(!ACTIVE_CONTEXT_SOURCE.contains("crate::store::init_store"));
    assert!(!ACTIVE_CONTEXT_SOURCE.contains("get_or_build_search_index("));
    assert!(!ACTIVE_CONTEXT_SOURCE.contains("Option<&RuntimeComposition>"));
    assert!(!ACTIVE_CONTEXT_SOURCE.contains("match composition"));
    assert!(!ACTIVE_CONTEXT_SOURCE.contains("pub async fn context_project("));
}

#[test]
fn operation_aware_context_core_requires_composition() {
    assert!(CONTEXT_OPERATION_SOURCE.contains("composition: &RuntimeComposition"));
    assert!(CONTEXT_OPERATION_SOURCE.contains("composition.init_store"));
    assert!(
        CONTEXT_OPERATION_SOURCE.contains("composition.build_search_index_with_operation_context")
    );
    assert!(!CONTEXT_OPERATION_SOURCE.contains("Option<&RuntimeComposition>"));
    assert!(!CONTEXT_OPERATION_SOURCE.contains("Option<RuntimeComposition>"));
    assert!(!CONTEXT_OPERATION_SOURCE.contains("crate::store::init_store"));
    assert!(!CONTEXT_OPERATION_SOURCE.contains("get_or_build_search_index_with_operation_context"));
    assert!(!CONTEXT_OPERATION_SOURCE.contains("match composition"));

    assert!(
        DERIVED_READ_SOURCE.contains(
            "context_project_with_operation_context_impl(options, composition, operation)"
        )
    );
    assert!(!DERIVED_READ_SOURCE.contains("pub async fn context_project_with_operation_context("));
    assert!(
        !DERIVED_READ_SOURCE.contains("pub async fn change_map_project_with_operation_context(")
    );
    assert!(
        !SEARCH_OPERATION_SOURCE.contains("pub async fn search_project_with_operation_context(")
    );
}

#[test]
fn rustok_context_execution_is_owned_by_the_composition_service() {
    assert!(RUSTOK_ARCHITECTURE_SOURCE.contains("pub fn build_rustok_architecture_context("));
    assert!(!RUSTOK_ARCHITECTURE_SOURCE.contains("pub async fn rustok_architecture_context("));
    assert!(!RUSTOK_ARCHITECTURE_SOURCE.contains("context_project("));
    assert!(!RUSTOK_ARCHITECTURE_SOURCE.contains("init_store"));

    assert!(RUSTOK_COMPOSITION_SOURCE.contains(
        "pub async fn rustok_architecture_context_with_composition_and_operation_context("
    ));
    assert!(
        RUSTOK_COMPOSITION_SOURCE
            .contains("context_project_with_composition_and_operation_context")
    );
    assert!(RUSTOK_COMPOSITION_SOURCE.contains("composition.init_store"));
}

#[test]
fn legacy_context_and_rustok_operation_owners_are_physically_removed() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(!source_root.join("context.rs").exists());
    assert!(!source_root.join("rustok_operation.rs").exists());
    assert!(!APP_LIB_SOURCE.contains("#[path = \"context.rs\"]"));
    assert!(!APP_LIB_SOURCE.contains("pub mod rustok_operation;"));
    assert!(!APP_LIB_SOURCE.contains("pub use crate::rustok_operation::*;"));
    assert!(!APP_LIB_SOURCE.contains("pub use rustok_operation::*;"));
}
