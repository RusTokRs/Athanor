const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const SEARCH_FACADE_SOURCE: &str = include_str!("../src/search_facade.rs");
const CHANGE_MAP_FACADE_SOURCE: &str = include_str!("../src/change_map_facade.rs");
const CHANGE_MAP_CORE_SOURCE: &str = include_str!("../src/change_map.rs");
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
fn public_change_map_surface_requires_runtime_composition() {
    assert!(APP_LIB_SOURCE.contains(
        "#[path = \"change_map_facade.rs\"]\npub mod change_map;"
    ));
    assert!(CHANGE_MAP_FACADE_SOURCE.contains(
        "pub async fn change_map_project_with_composition("
    ));
    assert!(CHANGE_MAP_FACADE_SOURCE.contains("composition: &RuntimeComposition"));
    assert!(CHANGE_MAP_FACADE_SOURCE.contains(
        "core::change_map_project_with_composition(options, composition).await"
    ));

    assert!(!CHANGE_MAP_FACADE_SOURCE.contains("pub async fn change_map_project("));
    assert!(!CHANGE_MAP_FACADE_SOURCE.contains("crate::store::init_store"));
    assert!(!CHANGE_MAP_FACADE_SOURCE.contains("Option<&RuntimeComposition>"));
    assert!(!CHANGE_MAP_FACADE_SOURCE.contains("pub use core::*"));
}

#[test]
fn remaining_snapshot_compatibility_is_crate_private_and_bounded_to_legacy_change_map_core() {
    assert!(SEARCH_FACADE_SOURCE.contains("pub(crate) async fn search_snapshot("));
    assert!(!SEARCH_FACADE_SOURCE.contains("pub async fn search_snapshot("));
    assert_eq!(
        SEARCH_FACADE_SOURCE
            .matches("pub(crate) async fn search_snapshot(")
            .count(),
        1
    );
    assert!(CHANGE_MAP_CORE_SOURCE.contains("use crate::search::search_snapshot;"));
    assert!(CHANGE_MAP_CORE_SOURCE.contains(
        "let search = search_snapshot(&root, &snapshot, task.to_string(), search_limit).await?;"
    ));
    assert!(!CHANGE_MAP_CORE_SOURCE.contains("search_snapshot_with_operation_context"));
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
fn read_service_facades_remain_bounded() {
    for (name, source, max_lines) in [
        ("ChangeMap facade", CHANGE_MAP_FACADE_SOURCE, 40),
        ("Search facade", SEARCH_FACADE_SOURCE, 120),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
