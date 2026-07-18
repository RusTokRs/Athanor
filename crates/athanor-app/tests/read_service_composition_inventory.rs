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
fn remaining_snapshot_compatibility_is_bounded_to_change_map() {
    assert_eq!(
        SEARCH_FACADE_SOURCE
            .matches("pub async fn search_snapshot(")
            .count(),
        1
    );
    assert!(CHANGE_MAP_SOURCE.contains("use crate::search::search_snapshot;"));
    assert!(CHANGE_MAP_SOURCE.contains(
        "let search = search_snapshot(&root, &snapshot, task.to_string(), search_limit).await?;"
    ));
    assert!(!CHANGE_MAP_SOURCE.contains("search_snapshot_with_operation_context"));
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
