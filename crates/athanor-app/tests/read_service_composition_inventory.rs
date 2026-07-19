const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const SEARCH_FACADE_SOURCE: &str = include_str!("../src/search_facade.rs");
const EXPLAIN_SOURCE: &str = include_str!("../src/explain.rs");
const API_REGISTRY_SOURCE: &str = include_str!("../src/api_registry.rs");
const GRAPH_OPERATION_SOURCE: &str = include_str!("../src/graph_operation.rs");
const REPAIR_LATEST_SOURCE: &str = include_str!("../src/repair_latest.rs");
const REPAIR_RECOVERY_SOURCE: &str = include_str!("../src/repair_recovery.rs");
const REPAIR_COMPOSITION_SOURCE: &str = include_str!("../src/repair_composition.rs");
const REPAIR_DIRECT_SOURCE: &str = include_str!("../src/repair_composition/direct.rs");

const CHANGE_MAP_ROOT: &str = include_str!("../src/change_map.rs");
const CHANGE_MAP_MODEL: &str = include_str!("../src/change_map/model.rs");
const CHANGE_MAP_EXECUTION: &str = include_str!("../src/change_map/execution.rs");
const CHANGE_MAP_RANKING: &str = include_str!("../src/change_map/ranking.rs");
const CHANGE_MAP_EVIDENCE: &str = include_str!("../src/change_map/evidence.rs");
const CHANGE_MAP_TESTS: &str = include_str!("../src/change_map/tests.rs");

const OVERVIEW_ROOT: &str = include_str!("../src/overview.rs");
const OVERVIEW_MODEL: &str = include_str!("../src/overview/model.rs");
const OVERVIEW_EXECUTION: &str = include_str!("../src/overview/execution.rs");
const OVERVIEW_AGGREGATION: &str = include_str!("../src/overview/aggregation.rs");
const OVERVIEW_TESTS: &str = include_str!("../src/overview/tests.rs");

const CAPABILITIES_ROOT: &str = include_str!("../src/capabilities.rs");
const CAPABILITIES_MODEL: &str = include_str!("../src/capabilities/model.rs");
const CAPABILITIES_EXECUTION: &str = include_str!("../src/capabilities/execution.rs");
const CAPABILITIES_AGGREGATION: &str = include_str!("../src/capabilities/aggregation.rs");
const CAPABILITIES_TESTS: &str = include_str!("../src/capabilities/tests.rs");

const IMPACT_ROOT: &str = include_str!("../src/impact.rs");
const IMPACT_MODEL: &str = include_str!("../src/impact/model.rs");
const IMPACT_EXECUTION: &str = include_str!("../src/impact/execution.rs");
const IMPACT_TRAVERSAL: &str = include_str!("../src/impact/traversal.rs");
const IMPACT_TESTS: &str = include_str!("../src/impact/tests.rs");

const COVERAGE_ROOT: &str = include_str!("../src/coverage.rs");
const COVERAGE_MODEL: &str = include_str!("../src/coverage/model.rs");
const COVERAGE_EXECUTION: &str = include_str!("../src/coverage/execution.rs");
const COVERAGE_AGGREGATION: &str = include_str!("../src/coverage/aggregation.rs");
const COVERAGE_TESTS: &str = include_str!("../src/coverage/tests.rs");

fn assert_conventional_root(root: &str, modules: &[&str], exports: &[&str]) {
    for module in modules {
        assert!(root.contains(&format!("mod {module};")));
    }
    for export in exports {
        assert!(root.contains(export));
    }
    assert!(root.contains("#[cfg(test)]\nmod tests;"));
    assert!(!root.contains("include!("));
    assert!(!root.contains("facade"));
}

fn assert_composition_execution(source: &str, entrypoint: &str, legacy_entrypoint: &str) {
    assert!(source.contains(entrypoint));
    assert!(source.contains("composition: &RuntimeComposition"));
    assert!(source.contains("composition.init_store(&root, &config)"));
    assert!(!source.contains(legacy_entrypoint));
    assert!(!source.contains("Option<&RuntimeComposition>"));
    assert!(!source.contains("crate::store::init_store"));
    assert!(!source.contains("match composition"));
}

#[test]
fn search_compatibility_apis_are_removed() {
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
    assert!(!SEARCH_FACADE_SOURCE.contains("pub async fn search_snapshot("));
    assert!(!SEARCH_FACADE_SOURCE.contains("pub(crate) async fn search_snapshot("));
    assert!(!SEARCH_FACADE_SOURCE.contains("crate::test_runtime::composition()"));
    assert!(!SEARCH_FACADE_SOURCE.contains("use anyhow::bail;"));
    assert!(!SEARCH_FACADE_SOURCE.contains("use std::sync::Arc;"));
}

#[test]
fn graph_operation_reads_require_explicit_composition() {
    for entrypoint in [
        "pub async fn export_graph_with_composition_and_operation_context(",
        "pub async fn related_graph_with_composition_and_operation_context(",
        "pub async fn shortest_graph_path_with_composition_and_operation_context(",
        "pub async fn graph_hubs_with_composition_and_operation_context(",
        "pub async fn graph_pagerank_with_composition_and_operation_context(",
        "pub async fn graph_cycles_with_composition_and_operation_context(",
    ] {
        assert!(GRAPH_OPERATION_SOURCE.contains(entrypoint));
    }
    for legacy in [
        "pub async fn export_graph_with_operation_context(",
        "pub async fn related_graph_with_operation_context(",
        "pub async fn shortest_graph_path_with_operation_context(",
        "pub async fn graph_hubs_with_operation_context(",
        "pub async fn graph_pagerank_with_operation_context(",
        "pub async fn graph_cycles_with_operation_context(",
    ] {
        assert!(!GRAPH_OPERATION_SOURCE.contains(legacy));
    }
    assert!(GRAPH_OPERATION_SOURCE.contains("composition: &RuntimeComposition"));
    assert!(GRAPH_OPERATION_SOURCE.contains("composition.init_store(&root, &config)"));
    assert!(GRAPH_OPERATION_SOURCE.contains(
        "load_latest_snapshot_with_operation_context(operation)"
    ));
    assert!(!GRAPH_OPERATION_SOURCE.contains("Option<&RuntimeComposition>"));
    assert!(!GRAPH_OPERATION_SOURCE.contains("crate::store::init_store"));
    assert!(!GRAPH_OPERATION_SOURCE.contains("match composition"));
}

#[test]
fn repair_has_one_composition_execution_owner() {
    assert!(REPAIR_LATEST_SOURCE.contains("pub struct RepairCanonicalLatestOptions"));
    assert!(REPAIR_LATEST_SOURCE.contains("pub struct RepairCanonicalLatestReport"));
    assert!(!REPAIR_LATEST_SOURCE.contains("pub async fn repair_canonical_latest("));
    assert!(!REPAIR_LATEST_SOURCE.contains("crate::store::init_store"));
    assert!(!REPAIR_LATEST_SOURCE.contains("repair_canonical_latest_with_store"));

    assert!(REPAIR_RECOVERY_SOURCE.contains("pub struct RepairRecoverIndexOptions"));
    assert!(REPAIR_RECOVERY_SOURCE.contains("pub struct RepairRecoverIndexReport"));
    assert!(!REPAIR_RECOVERY_SOURCE.contains("pub async fn recover_index_publication("));
    assert!(!REPAIR_RECOVERY_SOURCE.contains("crate::store::init_store"));
    assert!(!REPAIR_RECOVERY_SOURCE.contains("recover_index_publication_with_store"));

    for entrypoint in [
        "pub async fn repair_canonical_latest_with_composition(",
        "pub async fn recover_index_publication_with_composition(",
    ] {
        assert!(REPAIR_COMPOSITION_SOURCE.contains(entrypoint));
    }
    assert!(REPAIR_COMPOSITION_SOURCE.contains("direct::repair_latest(options, composition).await"));
    assert!(REPAIR_COMPOSITION_SOURCE.contains("direct::recover_index(options, composition).await"));
    assert!(REPAIR_DIRECT_SOURCE.contains("pub(super) async fn repair_latest("));
    assert!(REPAIR_DIRECT_SOURCE.contains("pub(super) async fn recover_index("));
    assert!(REPAIR_DIRECT_SOURCE.contains("composition: &RuntimeComposition"));
    assert!(REPAIR_DIRECT_SOURCE.contains("composition.init_store(&root, &config)"));
    for regression in [
        "async fn repairs_corrupt_jsonl_latest_to_authoritative_generation()",
        "async fn refuses_to_rewind_to_older_committed_generation()",
        "async fn standalone_recovery_publishes_committed_pending_pointer()",
        "async fn dry_run_reports_pending_identity_without_mutation()",
    ] {
        assert!(REPAIR_DIRECT_SOURCE.contains(regression));
    }
    assert!(!REPAIR_DIRECT_SOURCE.contains("crate::store::init_store"));
    assert!(!REPAIR_DIRECT_SOURCE.contains("Option<&RuntimeComposition>"));
    assert!(!REPAIR_DIRECT_SOURCE.contains("match composition"));
}

#[test]
fn change_map_is_bounded_and_composition_only() {
    assert!(APP_LIB_SOURCE.contains("pub mod change_map;"));
    assert_conventional_root(
        CHANGE_MAP_ROOT,
        &["evidence", "execution", "model", "ranking"],
        &[
            "pub use execution::change_map_project_with_composition;",
            "pub use model::{",
        ],
    );
    assert_composition_execution(
        CHANGE_MAP_EXECUTION,
        "pub async fn change_map_project_with_composition(",
        "pub async fn change_map_project(",
    );
    assert!(CHANGE_MAP_EXECUTION.contains(
        "use crate::search::search_snapshot_with_composition;"
    ));
    assert!(CHANGE_MAP_EXECUTION.contains("let search = search_snapshot_with_composition("));
}

#[test]
fn overview_is_bounded_and_composition_only() {
    assert_conventional_root(
        OVERVIEW_ROOT,
        &["aggregation", "execution", "model"],
        &[
            "pub use execution::overview_project_with_composition;",
            "pub use aggregation::build_repository_overview;",
            "pub use model::{",
        ],
    );
    assert_composition_execution(
        OVERVIEW_EXECUTION,
        "pub async fn overview_project_with_composition(",
        "pub async fn overview_project(",
    );
}

#[test]
fn capabilities_are_bounded_and_composition_only() {
    assert_conventional_root(
        CAPABILITIES_ROOT,
        &["aggregation", "execution", "model"],
        &[
            "pub use execution::capabilities_project_with_composition;",
            "pub use model::{",
        ],
    );
    assert_composition_execution(
        CAPABILITIES_EXECUTION,
        "pub async fn capabilities_project_with_composition(",
        "pub async fn capabilities_project(",
    );
}

#[test]
fn impact_is_bounded_and_composition_only() {
    assert_conventional_root(
        IMPACT_ROOT,
        &["execution", "model", "traversal"],
        &[
            "pub use execution::impact_project_with_composition;",
            "pub use traversal::impact_snapshot;",
            "pub use model::{",
        ],
    );
    assert_composition_execution(
        IMPACT_EXECUTION,
        "pub async fn impact_project_with_composition(",
        "pub async fn impact_project(",
    );
}

#[test]
fn coverage_is_bounded_and_composition_only() {
    assert_conventional_root(
        COVERAGE_ROOT,
        &["aggregation", "execution", "model"],
        &[
            "pub use execution::coverage_project_with_composition;",
            "pub use model::{",
        ],
    );
    assert_composition_execution(
        COVERAGE_EXECUTION,
        "pub async fn coverage_project_with_composition(",
        "pub async fn coverage_project(",
    );
}

#[test]
fn explain_and_api_registry_require_explicit_composition() {
    assert_composition_execution(
        EXPLAIN_SOURCE,
        "pub async fn explain_project_with_composition(",
        "pub async fn explain_project(",
    );
    assert_composition_execution(
        API_REGISTRY_SOURCE,
        "pub async fn query_api_registry_with_composition(",
        "pub async fn query_api_registry(",
    );
    assert!(API_REGISTRY_SOURCE.contains("crate::test_runtime::composition()"));
}

#[test]
fn read_service_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("ChangeMap root", CHANGE_MAP_ROOT, 30),
        ("ChangeMap model", CHANGE_MAP_MODEL, 180),
        ("ChangeMap execution", CHANGE_MAP_EXECUTION, 220),
        ("ChangeMap ranking", CHANGE_MAP_RANKING, 570),
        ("ChangeMap evidence", CHANGE_MAP_EVIDENCE, 100),
        ("ChangeMap tests", CHANGE_MAP_TESTS, 680),
        ("Overview root", OVERVIEW_ROOT, 30),
        ("Overview model", OVERVIEW_MODEL, 140),
        ("Overview execution", OVERVIEW_EXECUTION, 60),
        ("Overview aggregation", OVERVIEW_AGGREGATION, 430),
        ("Overview tests", OVERVIEW_TESTS, 220),
        ("Capabilities root", CAPABILITIES_ROOT, 30),
        ("Capabilities model", CAPABILITIES_MODEL, 130),
        ("Capabilities execution", CAPABILITIES_EXECUTION, 70),
        ("Capabilities aggregation", CAPABILITIES_AGGREGATION, 360),
        ("Capabilities tests", CAPABILITIES_TESTS, 200),
        ("Impact root", IMPACT_ROOT, 30),
        ("Impact model", IMPACT_MODEL, 100),
        ("Impact execution", IMPACT_EXECUTION, 210),
        ("Impact traversal", IMPACT_TRAVERSAL, 270),
        ("Impact tests", IMPACT_TESTS, 190),
        ("Coverage root", COVERAGE_ROOT, 30),
        ("Coverage model", COVERAGE_MODEL, 130),
        ("Coverage execution", COVERAGE_EXECUTION, 100),
        ("Coverage aggregation", COVERAGE_AGGREGATION, 380),
        ("Coverage tests", COVERAGE_TESTS, 180),
        ("Graph operation", GRAPH_OPERATION_SOURCE, 330),
        ("Repair latest model", REPAIR_LATEST_SOURCE, 60),
        ("Repair recovery model", REPAIR_RECOVERY_SOURCE, 60),
        ("Repair composition root", REPAIR_COMPOSITION_SOURCE, 50),
        ("Repair direct execution and tests", REPAIR_DIRECT_SOURCE, 520),
        ("Search facade", SEARCH_FACADE_SOURCE, 100),
        ("API registry", API_REGISTRY_SOURCE, 300),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
