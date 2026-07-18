const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const SEARCH_FACADE_SOURCE: &str = include_str!("../src/search_facade.rs");
const CHANGE_MAP_ROOT_SOURCE: &str = include_str!("../src/change_map.rs");
const CHANGE_MAP_MODEL_SOURCE: &str = include_str!("../src/change_map/model.rs");
const CHANGE_MAP_EXECUTION_SOURCE: &str = include_str!("../src/change_map/execution.rs");
const CHANGE_MAP_RANKING_SOURCE: &str = include_str!("../src/change_map/ranking.rs");
const CHANGE_MAP_EVIDENCE_SOURCE: &str = include_str!("../src/change_map/evidence.rs");
const CHANGE_MAP_TESTS_SOURCE: &str = include_str!("../src/change_map/tests.rs");
const EXPLAIN_SOURCE: &str = include_str!("../src/explain.rs");
const API_REGISTRY_SOURCE: &str = include_str!("../src/api_registry.rs");

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
fn change_map_uses_conventional_bounded_modules() {
    assert!(APP_LIB_SOURCE.contains("pub mod change_map;"));
    assert!(!APP_LIB_SOURCE.contains("change_map_facade.rs"));

    for module in ["evidence", "execution", "model", "ranking"] {
        assert!(CHANGE_MAP_ROOT_SOURCE.contains(&format!("mod {module};")));
    }
    assert!(CHANGE_MAP_ROOT_SOURCE.contains("#[cfg(test)]\nmod tests;"));
    assert!(CHANGE_MAP_ROOT_SOURCE.contains(
        "pub use execution::change_map_project_with_composition;"
    ));
    assert!(CHANGE_MAP_ROOT_SOURCE.contains("pub use model::{"));

    for source in [
        CHANGE_MAP_ROOT_SOURCE,
        CHANGE_MAP_MODEL_SOURCE,
        CHANGE_MAP_EXECUTION_SOURCE,
        CHANGE_MAP_RANKING_SOURCE,
        CHANGE_MAP_EVIDENCE_SOURCE,
        CHANGE_MAP_TESTS_SOURCE,
    ] {
        assert!(!source.contains("include!("));
        assert!(!source.contains("change_map_facade"));
    }
}

#[test]
fn change_map_execution_requires_runtime_composition() {
    assert!(CHANGE_MAP_EXECUTION_SOURCE.contains(
        "pub async fn change_map_project_with_composition("
    ));
    assert!(CHANGE_MAP_EXECUTION_SOURCE.contains("composition: &RuntimeComposition"));
    assert!(CHANGE_MAP_EXECUTION_SOURCE.contains("composition.init_store(&root, &config)"));
    assert!(CHANGE_MAP_EXECUTION_SOURCE.contains(
        "use crate::search::search_snapshot_with_composition;"
    ));
    assert!(CHANGE_MAP_EXECUTION_SOURCE.contains(
        "let search = search_snapshot_with_composition("
    ));

    assert!(!CHANGE_MAP_EXECUTION_SOURCE.contains("pub async fn change_map_project("));
    assert!(!CHANGE_MAP_EXECUTION_SOURCE.contains("Option<&RuntimeComposition>"));
    assert!(!CHANGE_MAP_EXECUTION_SOURCE.contains("crate::store::init_store"));
    assert!(!CHANGE_MAP_EXECUTION_SOURCE.contains("use crate::search::search_snapshot;"));
    assert!(!CHANGE_MAP_EXECUTION_SOURCE.contains("match composition"));
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
fn api_registry_requires_explicit_composition() {
    assert!(API_REGISTRY_SOURCE.contains(
        "pub async fn query_api_registry_with_composition("
    ));
    assert!(API_REGISTRY_SOURCE.contains("composition: &RuntimeComposition"));
    assert!(API_REGISTRY_SOURCE.contains("composition.init_store(&root, &config)"));
    assert!(API_REGISTRY_SOURCE.contains("crate::test_runtime::composition()"));

    assert!(!API_REGISTRY_SOURCE.contains("pub async fn query_api_registry("));
    assert!(!API_REGISTRY_SOURCE.contains("crate::store::init_store"));
    assert!(!API_REGISTRY_SOURCE.contains("Option<&RuntimeComposition>"));
    assert!(!API_REGISTRY_SOURCE.contains("match composition"));
}

#[test]
fn read_service_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("ChangeMap root", CHANGE_MAP_ROOT_SOURCE, 30),
        ("ChangeMap model", CHANGE_MAP_MODEL_SOURCE, 180),
        ("ChangeMap execution", CHANGE_MAP_EXECUTION_SOURCE, 220),
        ("ChangeMap ranking", CHANGE_MAP_RANKING_SOURCE, 570),
        ("ChangeMap evidence", CHANGE_MAP_EVIDENCE_SOURCE, 100),
        ("ChangeMap tests", CHANGE_MAP_TESTS_SOURCE, 680),
        ("Search facade", SEARCH_FACADE_SOURCE, 100),
        ("API registry", API_REGISTRY_SOURCE, 300),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
