const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const APP_CARGO: &str = include_str!("../Cargo.toml");
const RUNTIME_DEFAULTS_CARGO: &str = include_str!("../../athanor-runtime-defaults/Cargo.toml");
const RUNTIME_DEFAULTS_SOURCE: &str = include_str!("../../athanor-runtime-defaults/src/lib.rs");
const CI_SOURCE: &str = include_str!("../../../.github/workflows/ci.yml");
const RUNTIME_ROOT_SOURCE: &str = include_str!("../src/runtime.rs");
const RUNTIME_REGISTRY_SOURCE: &str = include_str!("../src/runtime/registry.rs");
const RUNTIME_BUILDER_SOURCE: &str = include_str!("../src/runtime/builder.rs");
const PROJECTION_SOURCE: &str = include_str!("../src/projection.rs");
const STORE_FACADE_SOURCE: &str = include_str!("../src/store_facade.rs");
const STORE_CORE_SOURCE: &str = include_str!("../src/store.rs");
const SEARCH_FACADE_SOURCE: &str = include_str!("../src/search_facade.rs");
const SEARCH_CORE_SOURCE: &str = include_str!("../src/search.rs");
const SEARCH_INDEX_SOURCE: &str = include_str!("../src/search/index.rs");
const VALIDATE_CHANGED_SOURCE: &str = include_str!("../src/validate_changed.rs");
const INDEX_SOURCE: &str = include_str!("../src/index_runtime.rs");
const GENERATION_SOURCE: &str = include_str!("../src/generation.rs");
const WIKI_SOURCE: &str = include_str!("../src/wiki.rs");
const REPORT_SOURCE: &str = include_str!("../src/report.rs");
const BENCH_SOURCE: &str = include_str!("../src/bench.rs");
const TEST_RUNTIME_SOURCE: &str = include_str!("../src/test_runtime.rs");
const COMPOSITION_ISOLATION_SOURCE: &str = include_str!("composition_isolation.rs");
const APPLICATION_COMPOSITION_SOURCE: &str =
    include_str!("../src/application_report_composition.rs");
const INDEX_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/index_cli.rs");
const SEARCH_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/direct_search_cli.rs");
const GENERATION_CLI_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_generation_cli.rs");
const MCP_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/mcp_cli.rs");
const MIGRATION_DOC: &str =
    include_str!("../../../docs/development/legacy-runtime-compatibility.md");

#[test]
fn process_global_runtime_feature_and_storage_are_removed() {
    assert!(!APP_CARGO.contains("legacy-global-runtime"));
    assert!(!RUNTIME_DEFAULTS_CARGO.contains("legacy-global-runtime"));
    assert!(!CI_SOURCE.contains("- legacy-global-runtime"));

    for source in [
        APP_LIB_SOURCE,
        RUNTIME_ROOT_SOURCE,
        RUNTIME_REGISTRY_SOURCE,
        RUNTIME_BUILDER_SOURCE,
        PROJECTION_SOURCE,
        STORE_FACADE_SOURCE,
        STORE_CORE_SOURCE,
        SEARCH_FACADE_SOURCE,
        SEARCH_CORE_SOURCE,
        SEARCH_INDEX_SOURCE,
        TEST_RUNTIME_SOURCE,
    ] {
        assert!(!source.contains("OnceLock"));
        assert!(!source.contains("legacy_global"));
        assert!(!source.contains("legacy_registry"));
        assert!(!source.contains("LegacyFactoryInstallError"));
        assert!(!source.contains("LegacyFactoryUnavailableError"));
    }
    assert!(!APP_LIB_SOURCE.contains("pub mod legacy_factory"));
    assert!(!TEST_RUNTIME_SOURCE.contains("fn install"));
}

#[test]
fn core_runtime_paths_require_explicit_composition() {
    assert!(RUNTIME_BUILDER_SOURCE.contains("registry: AdapterRegistry::empty()"));
    assert!(!RUNTIME_REGISTRY_SOURCE.contains("default_adapter_registry"));
    assert!(!RUNTIME_REGISTRY_SOURCE.contains("ensure_test_runtime"));
    assert!(STORE_FACADE_SOURCE.contains("explicit RuntimeComposition is required"));
    assert!(SEARCH_FACADE_SOURCE.contains("explicit RuntimeComposition is required"));
    assert!(!PROJECTION_SOURCE.contains("project_wiki_payload"));
    assert!(!PROJECTION_SOURCE.contains("project_html_payload"));
    assert!(TEST_RUNTIME_SOURCE.contains("pub(crate) fn composition()"));
}

#[test]
fn active_entrypoints_use_explicit_runtime_composition() {
    for source in [
        INDEX_CLI_SOURCE,
        SEARCH_CLI_SOURCE,
        GENERATION_CLI_SOURCE,
        MCP_CLI_SOURCE,
    ] {
        assert!(source.contains("athanor_runtime_defaults::production()"));
        assert!(!source.contains("athanor_runtime_defaults::install()"));
    }
    assert!(INDEX_CLI_SOURCE.contains("index_project_with_composition"));
    assert!(SEARCH_CLI_SOURCE.contains("search_project_with_composition"));
    assert!(GENERATION_CLI_SOURCE.contains("generate_project_with_composition"));
}

#[test]
fn application_report_services_have_no_store_scope_bridge() {
    assert!(APPLICATION_COMPOSITION_SOURCE.contains("api_direct::snapshot"));
    assert!(APPLICATION_COMPOSITION_SOURCE.contains("docs_direct::check"));
    assert!(!APPLICATION_COMPOSITION_SOURCE.contains("with_store_composition"));
    assert!(!APPLICATION_COMPOSITION_SOURCE.contains("crate::store::init_store"));
}

#[test]
fn installer_apis_are_removed_from_public_runtime_sources() {
    for source in [
        APP_LIB_SOURCE,
        RUNTIME_ROOT_SOURCE,
        PROJECTION_SOURCE,
        STORE_FACADE_SOURCE,
        SEARCH_FACADE_SOURCE,
        RUNTIME_DEFAULTS_SOURCE,
    ] {
        assert!(!source.contains("pub fn install_"));
        assert!(!source.contains("pub fn install()"));
        assert!(!source.contains("process-global installation was removed"));
    }
    assert!(!APP_LIB_SOURCE.contains("install_wiki_projector_factory"));
    assert!(!APP_LIB_SOURCE.contains("install_html_projector_factory"));
    assert!(MIGRATION_DOC.contains("COMP-003C2A"));
}

#[test]
fn dead_no_composition_wrappers_are_removed() {
    assert!(!VALIDATE_CHANGED_SOURCE.contains("pub async fn validate_changed("));
    assert!(VALIDATE_CHANGED_SOURCE.contains("pub async fn validate_changed_with_composition("));

    assert!(!SEARCH_FACADE_SOURCE.contains("pub async fn search_project("));
    assert!(SEARCH_FACADE_SOURCE.contains("pub async fn search_project_with_composition("));
    assert!(!SEARCH_FACADE_SOURCE.contains("pub async fn get_or_build_search_index("));
    assert!(!SEARCH_FACADE_SOURCE.contains("pub fn get_or_build_search_index_sync("));

    for source in [INDEX_SOURCE, GENERATION_SOURCE, WIKI_SOURCE, REPORT_SOURCE, BENCH_SOURCE] {
        assert!(!source.contains("Option<RuntimeComposition>"));
        assert!(!source.contains("Option<&RuntimeComposition>"));
    }
    assert!(!INDEX_SOURCE.contains("pub async fn index_project("));
    assert!(!INDEX_SOURCE.contains("pub async fn index_project_with_operation_context("));
    assert!(!INDEX_SOURCE.contains("pub async fn index_project_cancellable("));
    assert!(!INDEX_SOURCE.contains("pub async fn index_project_cancellable_with_operation_context("));
    assert!(!GENERATION_SOURCE.contains("pub async fn generate_project("));
    assert!(!GENERATION_SOURCE.contains("pub async fn generate_project_with_operation_context("));
    assert!(!GENERATION_SOURCE.contains("pub async fn generate_project_cancellable("));
    assert!(!GENERATION_SOURCE.contains("pub async fn generate_project_cancellable_with_operation_context("));
    assert!(!WIKI_SOURCE.contains("pub async fn project_wiki("));
    assert!(!WIKI_SOURCE.contains("pub async fn project_wiki_with_operation_context("));
    assert!(!WIKI_SOURCE.contains("pub async fn project_wiki_cancellable("));
    assert!(!WIKI_SOURCE.contains("pub async fn project_wiki_cancellable_with_operation_context("));
    assert!(!REPORT_SOURCE.contains("pub async fn project_html_report("));
    assert!(!REPORT_SOURCE.contains("pub async fn project_html_report_with_operation_context("));
    assert!(!REPORT_SOURCE.contains("pub async fn project_html_report_cancellable("));
    assert!(!REPORT_SOURCE.contains("pub async fn project_html_report_cancellable_with_operation_context("));
    assert!(!BENCH_SOURCE.contains("pub async fn benchmark_index("));

    assert!(MIGRATION_DOC.contains("COMP-003C2B1"));
    assert!(MIGRATION_DOC.contains("COMP-003C2B2"));
}

#[test]
fn parallel_isolation_matrix_covers_every_composed_factory_family() {
    assert!(COMPOSITION_ISOLATION_SOURCE.contains(
        "parallel_compositions_do_not_cross_store_search_or_projector_factories"
    ));
    for token in [
        "store_a",
        "store_b",
        "search_a",
        "search_b",
        "wiki_a",
        "wiki_b",
        "html_a",
        "html_b",
    ] {
        assert!(COMPOSITION_ISOLATION_SOURCE.contains(token));
    }
    assert!(COMPOSITION_ISOLATION_SOURCE.contains("tokio::spawn"));
    assert!(COMPOSITION_ISOLATION_SOURCE.contains("ITERATIONS"));
}

#[test]
fn composition_boundary_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("runtime root", RUNTIME_ROOT_SOURCE, 80),
        ("runtime registry", RUNTIME_REGISTRY_SOURCE, 360),
        ("runtime builder", RUNTIME_BUILDER_SOURCE, 220),
        ("projection facade", PROJECTION_SOURCE, 20),
        ("store facade", STORE_FACADE_SOURCE, 60),
        ("store core", STORE_CORE_SOURCE, 220),
        ("search facade", SEARCH_FACADE_SOURCE, 200),
        ("search core", SEARCH_CORE_SOURCE, 300),
        ("search index", SEARCH_INDEX_SOURCE, 160),
        ("validate changed", VALIDATE_CHANGED_SOURCE, 300),
        ("test runtime", TEST_RUNTIME_SOURCE, 180),
        ("composition isolation", COMPOSITION_ISOLATION_SOURCE, 320),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
