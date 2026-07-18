const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const SEARCH_FACADE_SOURCE: &str = include_str!("../src/search_facade.rs");
const CHANGE_MAP_SOURCE: &str = include_str!("../src/change_map.rs");
const EXPLAIN_SOURCE: &str = include_str!("../src/explain.rs");

#[test]
fn operation_aware_search_compatibility_apis_are_removed() {
    assert!(SEARCH_FACADE_SOURCE.contains(
        "pub async fn search_snapshot_with_composition_and_operation_context("
    ));
    assert!(SEARCH_FACADE_SOURCE.contains(
        "get_or_build_search_index_with_factory_and_operation"
    ));

    assert!(!SEARCH_FACADE_SOURCE.contains(
        "pub async fn search_snapshot_with_operation_context("
    ));
    assert!(!SEARCH_FACADE_SOURCE.contains(
        "pub fn get_or_build_search_index_with_operation_context("
    ));
    assert!(!SEARCH_FACADE_SOURCE.contains("use std::sync::Arc;"));
}

#[test]
fn change_map_owner_requires_runtime_composition() {
    assert!(APP_LIB_SOURCE.contains("pub mod change_map;"));
    assert!(!APP_LIB_SOURCE.contains("change_map_facade.rs"));

    assert!(CHANGE_MAP_SOURCE.contains(
        "pub async fn change_map_project_with_composition("
    ));
    assert!(CHANGE_MAP_SOURCE.contains("composition: &RuntimeComposition"));
    assert!(CHANGE_MAP_SOURCE.contains("composition.init_store(&root, &config)"));
    assert!(CHANGE_MAP_SOURCE.contains("use crate::search::search_snapshot_with_composition;"));
    assert!(CHANGE_MAP_SOURCE.contains(
        "let search = search_snapshot_with_composition("
    ));

    assert!(!CHANGE_MAP_SOURCE.contains("pub async fn change_map_project("));
    assert!(!CHANGE_MAP_SOURCE.contains("Option<&RuntimeComposition>"));
    assert!(!CHANGE_MAP_SOURCE.contains("crate::store::init_store"));
    assert!(!CHANGE_MAP_SOURCE.contains("use crate::search::search_snapshot;"));
    assert!(!CHANGE_MAP_SOURCE.contains("match composition"));
}

#[test]
fn no_composition_snapshot_search_helpers_are_removed() {
    assert!(!SEARCH_FACADE_SOURCE.contains("pub async fn search_snapshot("));
    assert!(!SEARCH_FACADE_SOURCE.contains("pub(crate) async fn search_snapshot("));
    assert!(!SEARCH_FACADE_SOURCE.contains("crate::test_runtime::composition()"));
    assert!(!SEARCH_FACADE_SOURCE.contains(
        "explicit RuntimeComposition is required for snapshot search"
    ));
    assert!(!SEARCH_FACADE_SOURCE.contains("use anyhow::bail;"));
}

#[test]
fn explain_owner_requires_explicit_composition() {
    assert!(EXPLAIN_SOURCE.contains("pub async fn explain_project_with_composition("));
    assert!(EXPLAIN_SOURCE.contains("composition: &RuntimeComposition"));
    assert!(EXPLAIN_SOURCE.contains("composition.init_store(&root, &config)"));

    assert!(!EXPLAIN_SOURCE.contains("pub async fn explain_project("));
    assert!(!EXPLAIN_SOURCE.contains("Option<&RuntimeComposition>"));
    assert!(!EXPLAIN_SOURCE.contains("crate::store::init_store"));
    assert!(!EXPLAIN_SOURCE.contains("match composition"));
}

#[test]
fn search_facade_remains_bounded() {
    let lines = SEARCH_FACADE_SOURCE.lines().count();
    assert!(lines <= 100, "Search facade grew to {lines} lines");
}
