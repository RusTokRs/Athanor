use std::path::Path;

const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const GRAPH_ROOT_SOURCE: &str = include_str!("../src/graph/mod.rs");
const GRAPH_MODEL_SOURCE: &str = include_str!("../src/graph/model.rs");
const GRAPH_STANDARD_SOURCE: &str = include_str!("../src/graph/standard.rs");
const GRAPH_RUSTOK_SOURCE: &str = include_str!("../src/graph/rustok.rs");
const GRAPH_TESTS_SOURCE: &str = include_str!("../src/graph/tests.rs");
const GRAPH_OPERATION_SOURCE: &str = include_str!("../src/graph_operation.rs");
const RUSTOK_OPERATION_SOURCE: &str = include_str!("../src/rustok_composition_operation.rs");
const STORE_FACADE_SOURCE: &str = include_str!("../src/store_facade.rs");

#[test]
fn graph_routes_to_conventional_bounded_owners() {
    assert!(
        APP_LIB_SOURCE
            .replace("\r\n", "\n")
            .contains("#[path = \"graph/mod.rs\"]\npub mod graph;")
    );
    for module in ["model", "rustok", "standard"] {
        assert!(GRAPH_ROOT_SOURCE.contains(&format!("mod {module};")));
        assert!(GRAPH_ROOT_SOURCE.contains(&format!("pub use {module}::*;")));
    }
    assert!(
        GRAPH_ROOT_SOURCE
            .replace("\r\n", "\n")
            .contains("#[cfg(test)]\nmod tests;")
    );
    assert!(!GRAPH_ROOT_SOURCE.contains("include!("));
    assert!(!GRAPH_ROOT_SOURCE.contains("facade"));

    let legacy_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/graph.rs");
    assert!(
        !legacy_path.exists(),
        "legacy graph.rs monolith must remain physically absent"
    );
}

#[test]
fn graph_contract_model_preserves_public_schema_and_report_surface() {
    for schema in [
        "athanor.graph_export.v1",
        "athanor.graph_cycles.v1",
        "athanor.graph_hubs.v1",
        "athanor.graph_pagerank.v1",
        "athanor.graph_path.v1",
        "athanor.graph_related.v1",
        "athanor.rustok_ffa_audit.v1",
        "athanor.rustok_fba_audit.v1",
        "athanor.rustok_page_builder_audit.v1",
    ] {
        assert!(GRAPH_MODEL_SOURCE.contains(schema));
    }
    for contract in [
        "pub struct GraphExportOptions",
        "pub struct GraphRelatedOptions",
        "pub struct GraphPathOptions",
        "pub struct GraphHubsOptions",
        "pub struct GraphPageRankOptions",
        "pub struct GraphCyclesOptions",
        "pub struct RustokFfaAudit",
        "pub struct RustokFbaAudit",
        "pub struct RustokPageBuilderAudit",
    ] {
        assert!(GRAPH_MODEL_SOURCE.contains(contract));
    }
}

#[test]
fn standard_graph_uses_pure_snapshot_and_canonical_cooperative_algorithms() {
    for builder in [
        "pub fn build_graph_export(",
        "pub fn graph_export_to_graphml(",
        "pub fn build_related_graph(",
        "pub fn build_shortest_graph_path(",
        "pub fn build_graph_hubs(",
        "pub fn build_graph_pagerank(",
        "pub fn build_graph_cycles(",
    ] {
        assert!(GRAPH_STANDARD_SOURCE.contains(builder));
    }
    for cooperative in [
        "build_related_graph_with_operation_context",
        "build_shortest_graph_path_with_operation_context",
        "build_graph_pagerank_with_operation_context",
        "build_graph_cycles_with_operation_context",
    ] {
        assert!(GRAPH_STANDARD_SOURCE.contains(cooperative));
    }
    assert!(GRAPH_STANDARD_SOURCE.contains("OperationContext::new(name)"));
    assert!(!GRAPH_STANDARD_SOURCE.contains("init_store"));
    assert!(!GRAPH_STANDARD_SOURCE.contains("load_config"));
    assert!(!GRAPH_STANDARD_SOURCE.contains("pub async fn"));
}

#[test]
fn rustok_graph_uses_canonical_cooperative_algorithms() {
    for builder in [
        "pub fn build_rustok_ffa_audit(",
        "pub fn build_rustok_fba_audit(",
        "pub fn build_rustok_page_builder_audit(",
        "pub fn build_rustok_ffa_surface_graph(",
        "pub fn build_rustok_ffa_violations_graph(",
        "pub fn build_rustok_fba_module_graph(",
        "pub fn build_rustok_fba_port_graph(",
        "pub fn build_rustok_fba_dependencies_graph(",
        "pub fn build_rustok_fba_violations_graph(",
        "pub fn build_rustok_page_builder_provider_graph(",
        "pub fn build_rustok_page_builder_consumer_graph(",
        "pub fn build_rustok_page_builder_violations_graph(",
    ] {
        assert!(GRAPH_RUSTOK_SOURCE.contains(builder));
    }
    assert!(GRAPH_RUSTOK_SOURCE.contains("crate::rustok_audit_cooperative::"));
    assert!(GRAPH_RUSTOK_SOURCE.contains("crate::rustok_graph_cooperative::"));
    assert!(!GRAPH_RUSTOK_SOURCE.contains("init_store"));
    assert!(!GRAPH_RUSTOK_SOURCE.contains("load_config"));
    assert!(!GRAPH_RUSTOK_SOURCE.contains("pub async fn"));
}

#[test]
fn graph_project_execution_requires_explicit_composition() {
    for source in [GRAPH_OPERATION_SOURCE, RUSTOK_OPERATION_SOURCE] {
        assert!(source.contains("composition: &RuntimeComposition"));
        assert!(source.contains("composition.init_store(&root, &config)"));
        assert!(!source.contains("Option<&RuntimeComposition>"));
        assert!(!source.contains("crate::store::init_store"));
        assert!(!source.contains("match composition"));
    }
    for legacy in [
        "pub async fn export_graph(",
        "pub async fn related_graph(",
        "pub async fn shortest_graph_path(",
        "pub async fn graph_hubs(",
        "pub async fn graph_pagerank(",
        "pub async fn graph_cycles(",
        "pub async fn rustok_ffa_audit(",
        "pub async fn rustok_fba_audit(",
        "pub async fn rustok_page_builder_audit(",
        "pub async fn graph_ffa_surface(",
        "pub async fn graph_ffa_violations(",
        "pub async fn graph_fba_module(",
        "pub async fn graph_fba_port(",
        "pub async fn graph_fba_dependencies(",
        "pub async fn graph_fba_violations(",
        "pub async fn graph_page_builder_provider(",
        "pub async fn graph_page_builder_consumer(",
        "pub async fn graph_page_builder_violations(",
    ] {
        assert!(!GRAPH_ROOT_SOURCE.contains(legacy));
        assert!(!GRAPH_STANDARD_SOURCE.contains(legacy));
        assert!(!GRAPH_RUSTOK_SOURCE.contains(legacy));
    }
}

#[test]
fn public_store_facade_exports_types_not_initialization() {
    assert!(STORE_FACADE_SOURCE.contains("pub use core::{AthanorStore, StoreFactory};"));
    assert!(!STORE_FACADE_SOURCE.contains("pub async fn init_store"));
    assert!(!STORE_FACADE_SOURCE.contains("test_runtime::composition"));
    assert!(!STORE_FACADE_SOURCE.contains("explicit RuntimeComposition is required"));
}

#[test]
fn graph_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("Graph root", GRAPH_ROOT_SOURCE, 20),
        ("Graph model", GRAPH_MODEL_SOURCE, 560),
        ("Graph standard", GRAPH_STANDARD_SOURCE, 430),
        ("Graph Rustok", GRAPH_RUSTOK_SOURCE, 220),
        ("Graph tests", GRAPH_TESTS_SOURCE, 260),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
