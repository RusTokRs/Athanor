use athanor_app::legacy_factory::{
    LegacyFactoryInstallError, LegacyFactoryUnavailableError,
};

const LEGACY_FACTORY_SOURCE: &str = include_str!("../src/legacy_factory.rs");
const ADAPTER_REGISTRY_SOURCE: &str = include_str!("../src/runtime/legacy_registry.rs");
const PROJECTION_SOURCE: &str = include_str!("../src/projection.rs");
const STORE_FACADE_SOURCE: &str = include_str!("../src/store_facade.rs");
const SEARCH_FACADE_SOURCE: &str = include_str!("../src/search_facade.rs");
const GRAPH_OPERATION_SOURCE: &str = include_str!("../src/graph_operation.rs");
const RUSTOK_COMPOSITION_SOURCE: &str = include_str!("../src/rustok_composition_operation.rs");
const APPLICATION_REPORT_COMPOSITION_SOURCE: &str =
    include_str!("../src/application_report_composition.rs");
const REPAIR_COMPOSITION_SOURCE: &str = include_str!("../src/repair_composition.rs");
const APP_LIB_SOURCE: &str = include_str!("../src/lib.rs");
const VALIDATE_CHANGED_SOURCE: &str = include_str!("../src/validate_changed.rs");
const CLI_ENTRY_SOURCE: &str = include_str!("../../../apps/ath/src/entry.rs");
const ROOT_COMMAND_SOURCE: &str = include_str!("../../../apps/ath/src/root_command.rs");
const ANALYSIS_SOURCE: &str = include_str!("../../../apps/ath/src/analysis_cli.rs");
const API_SOURCE: &str = include_str!("../../../apps/ath/src/api_cli.rs");
const DOCS_SOURCE: &str = include_str!("../../../apps/ath/src/docs_cli.rs");
const INDEX_SOURCE: &str = include_str!("../../../apps/ath/src/index_cli.rs");
const MCP_SOURCE: &str = include_str!("../../../apps/ath/src/mcp_cli.rs");
const PROJECTION_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/projection_cli.rs");
const PROJECTS_SOURCE: &str = include_str!("../../../apps/ath/src/projects_cli.rs");
const DIRECT_SEARCH_SOURCE: &str = include_str!("../../../apps/ath/src/direct_search_cli.rs");
const DIRECT_CONTEXT_SOURCE: &str = include_str!("../../../apps/ath/src/direct_context_cli.rs");
const DIRECT_CHECK_SOURCE: &str = include_str!("../../../apps/ath/src/direct_check_cli.rs");
const DIRECT_GRAPH_SOURCE: &str = include_str!("../../../apps/ath/src/direct_graph_cli.rs");
const RUSTOK_ROOT_SOURCE: &str = include_str!("../../../apps/ath/src/rustok_cli/mod.rs");
const RUSTOK_MODEL_SOURCE: &str = include_str!("../../../apps/ath/src/rustok_cli/model.rs");
const RUSTOK_RUN_SOURCE: &str = include_str!("../../../apps/ath/src/rustok_cli/run.rs");
const RENDER_ROOT_SOURCE: &str = include_str!("../../../apps/ath/src/render/mod.rs");
const GRAPH_RENDER_SOURCE: &str = include_str!("../../../apps/ath/src/render/graph.rs");
const RUSTOK_RENDER_SOURCE: &str = include_str!("../../../apps/ath/src/render/rustok.rs");
const CHECK_RENDER_SOURCE: &str = include_str!("../../../apps/ath/src/render/check.rs");
const API_RENDER_SOURCE: &str = include_str!("../../../apps/ath/src/render/api.rs");
const DIRECT_GENERATION_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_generation_cli.rs");
const DIRECT_READ_ROOT_SOURCE: &str = include_str!("../../../apps/ath/src/direct_read/mod.rs");
const DIRECT_READ_MODEL_SOURCE: &str = include_str!("../../../apps/ath/src/direct_read/model.rs");
const DIRECT_READ_OPERATION_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_read/operation.rs");
const DIRECT_READ_RUN_SOURCE: &str = include_str!("../../../apps/ath/src/direct_read/run.rs");
const DIRECT_READ_RENDER_ROOT_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_read/render/mod.rs");
const DIRECT_READ_ENTITY_RENDER_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_read/render/entity.rs");
const DIRECT_READ_CHANGE_RENDER_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_read/render/change.rs");
const DIRECT_READ_RENDER_SUPPORT_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_read/render/support.rs");
const REPAIR_ROOT_SOURCE: &str = include_str!("../../../apps/ath/src/repair/mod.rs");
const REPAIR_MODEL_SOURCE: &str = include_str!("../../../apps/ath/src/repair/model.rs");
const REPAIR_RUN_SOURCE: &str = include_str!("../../../apps/ath/src/repair/run.rs");
const REPAIR_RENDER_SOURCE: &str = include_str!("../../../apps/ath/src/repair/render.rs");
const DIRECT_VALIDATE_CHANGED_SOURCE: &str =
    include_str!("../../../apps/ath/src/direct_validate_changed_cli.rs");

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
fn store_and_search_globals_are_quarantined_behind_guarded_facades() {
    assert!(STORE_FACADE_SOURCE.contains("try_install_store_factory"));
    assert!(STORE_FACADE_SOURCE.contains("require_installed(&STORE_FACTORY_GUARD"));
    assert!(STORE_FACADE_SOURCE.contains("SCOPED_STORE_COMPOSITION"));
    assert!(STORE_FACADE_SOURCE.contains("with_store_composition"));
    assert!(!STORE_FACADE_SOURCE.contains("let _ = STORE_FACTORY"));
    assert!(SEARCH_FACADE_SOURCE.contains("try_install_search_index_factory"));
    assert!(SEARCH_FACADE_SOURCE.contains("try_install_search_index_operation_factory"));
    assert!(SEARCH_FACADE_SOURCE.contains("require_any_search_factory"));
    assert!(!SEARCH_FACADE_SOURCE.contains("let _ = SEARCH_INDEX_FACTORY"));
    assert!(!SEARCH_FACADE_SOURCE.contains("let _ = SEARCH_INDEX_OPERATION_FACTORY"));
    assert!(APP_LIB_SOURCE.contains("#[path = \"store_facade.rs\"]"));
    assert!(APP_LIB_SOURCE.contains("#[path = \"search_facade.rs\"]"));
}

#[test]
fn changed_validation_has_an_explicit_composition_path() {
    assert!(VALIDATE_CHANGED_SOURCE.contains("validate_changed_with_composition"));
    assert!(VALIDATE_CHANGED_SOURCE.contains("RuntimeBuilder::from_composition"));
    assert!(CLI_ENTRY_SOURCE.contains("direct_validate_changed_cli"));
    assert!(CLI_ENTRY_SOURCE.contains("Athanor changed validation runtime"));
    assert!(DIRECT_VALIDATE_CHANGED_SOURCE.contains("athanor_runtime_defaults::production()"));
    assert!(!DIRECT_VALIDATE_CHANGED_SOURCE.contains("athanor_runtime_defaults::install()"));
}

#[test]
fn graph_operations_have_an_explicit_composition_path() {
    for operation in [
        "export_graph_with_composition_and_operation_context",
        "related_graph_with_composition_and_operation_context",
        "shortest_graph_path_with_composition_and_operation_context",
        "graph_hubs_with_composition_and_operation_context",
        "graph_pagerank_with_composition_and_operation_context",
        "graph_cycles_with_composition_and_operation_context",
    ] {
        assert!(GRAPH_OPERATION_SOURCE.contains(operation));
    }
}

#[test]
fn rustok_operations_have_an_explicit_composition_path() {
    for operation in [
        "rustok_architecture_context_with_composition_and_operation_context",
        "rustok_ffa_audit_with_composition_and_operation_context",
        "rustok_fba_audit_with_composition_and_operation_context",
        "rustok_page_builder_audit_with_composition_and_operation_context",
        "graph_ffa_surface_with_composition_and_operation_context",
        "graph_ffa_violations_with_composition_and_operation_context",
        "graph_fba_module_with_composition_and_operation_context",
        "graph_fba_port_with_composition_and_operation_context",
        "graph_fba_dependencies_with_composition_and_operation_context",
        "graph_fba_violations_with_composition_and_operation_context",
        "graph_page_builder_provider_with_composition_and_operation_context",
        "graph_page_builder_consumer_with_composition_and_operation_context",
        "graph_page_builder_violations_with_composition_and_operation_context",
    ] {
        assert!(RUSTOK_COMPOSITION_SOURCE.contains(operation));
    }
    assert!(RUSTOK_COMPOSITION_SOURCE.contains("composition.init_store"));
    assert!(RUSTOK_COMPOSITION_SOURCE.contains(
        "context_project_with_composition_and_operation_context"
    ));
    assert!(!RUSTOK_COMPOSITION_SOURCE.contains("crate::store::init_store"));
}

#[test]
fn application_reports_have_an_explicit_composition_path() {
    assert!(APPLICATION_REPORT_COMPOSITION_SOURCE.contains(
        "snapshot_api_contract_with_composition"
    ));
    assert!(APPLICATION_REPORT_COMPOSITION_SOURCE.contains(
        "docs_propose_fix_with_composition"
    ));
    assert!(APPLICATION_REPORT_COMPOSITION_SOURCE.contains("with_store_composition"));
    assert!(APP_LIB_SOURCE.contains("pub mod application_report_composition"));
    assert!(API_SOURCE.contains("snapshot_api_contract_with_composition"));
    assert!(DOCS_SOURCE.contains("docs_propose_fix_with_composition"));
    assert!(API_SOURCE.contains("athanor_runtime_defaults::production()"));
    assert!(DOCS_SOURCE.contains("athanor_runtime_defaults::production()"));
    assert!(!API_SOURCE.contains("athanor_runtime_defaults::install()"));
    assert!(!DOCS_SOURCE.contains("athanor_runtime_defaults::install()"));
}

#[test]
fn repair_operations_have_an_explicit_composition_path() {
    assert!(REPAIR_COMPOSITION_SOURCE.contains(
        "recover_index_publication_with_composition"
    ));
    assert!(REPAIR_COMPOSITION_SOURCE.contains(
        "repair_canonical_latest_with_composition"
    ));
    assert!(REPAIR_COMPOSITION_SOURCE.contains("with_store_composition"));
    assert!(APP_LIB_SOURCE.contains("pub mod repair_composition"));
}

#[test]
fn focused_composition_paths_do_not_install_global_runtime() {
    for source in [
        DIRECT_SEARCH_SOURCE,
        DIRECT_CONTEXT_SOURCE,
        DIRECT_CHECK_SOURCE,
        DIRECT_GRAPH_SOURCE,
        RUSTOK_RUN_SOURCE,
        DIRECT_GENERATION_SOURCE,
        DIRECT_READ_RUN_SOURCE,
        INDEX_SOURCE,
        ANALYSIS_SOURCE,
        API_SOURCE,
        DOCS_SOURCE,
    ] {
        assert!(source.contains("athanor_runtime_defaults::production()"));
        assert!(!source.contains("athanor_runtime_defaults::install()"));
    }
    assert!(REPAIR_RUN_SOURCE.contains("athanor_runtime_defaults::production()"));
    assert!(!REPAIR_RUN_SOURCE.contains("athanor_runtime_defaults::install()"));
    assert!(!CLI_ENTRY_SOURCE.contains("athanor_runtime_defaults::install()"));
}

#[test]
fn cli_has_no_compatibility_includes_namespace_shadowing_or_legacy_root() {
    for source in [
        CLI_ENTRY_SOURCE,
        ROOT_COMMAND_SOURCE,
        ANALYSIS_SOURCE,
        API_SOURCE,
        DOCS_SOURCE,
        INDEX_SOURCE,
        MCP_SOURCE,
        PROJECTION_CLI_SOURCE,
        PROJECTS_SOURCE,
        DIRECT_READ_ROOT_SOURCE,
        DIRECT_READ_MODEL_SOURCE,
        DIRECT_READ_OPERATION_SOURCE,
        DIRECT_READ_RUN_SOURCE,
        DIRECT_READ_RENDER_ROOT_SOURCE,
        DIRECT_READ_ENTITY_RENDER_SOURCE,
        DIRECT_READ_CHANGE_RENDER_SOURCE,
        DIRECT_READ_RENDER_SUPPORT_SOURCE,
        REPAIR_ROOT_SOURCE,
        REPAIR_MODEL_SOURCE,
        REPAIR_RUN_SOURCE,
        REPAIR_RENDER_SOURCE,
        RUSTOK_ROOT_SOURCE,
        RUSTOK_MODEL_SOURCE,
        RUSTOK_RUN_SOURCE,
        RENDER_ROOT_SOURCE,
        GRAPH_RENDER_SOURCE,
        RUSTOK_RENDER_SOURCE,
        CHECK_RENDER_SOURCE,
        API_RENDER_SOURCE,
    ] {
        assert!(!source.contains("include!("));
        assert!(!source.contains("mod athanor_app {"));
        assert!(!source.contains("mod athanor_runtime_defaults {"));
    }
    assert!(!CLI_ENTRY_SOURCE.contains("mod legacy"));
    assert!(!CLI_ENTRY_SOURCE.contains("main.rs"));
    assert!(!ROOT_COMMAND_SOURCE.contains("Command::Legacy"));
    assert!(!CLI_ENTRY_SOURCE.contains("direct_application_report_cli"));
    assert!(!CLI_ENTRY_SOURCE.contains("_bridge"));
}

#[test]
fn root_command_model_owns_every_command_family() {
    for route in [
        "Command::Plugin",
        "Command::ValidateChanged",
        "Command::Repair",
        "Command::Generation",
        "Command::Config",
        "Command::Check",
        "Command::Rustok",
        "Command::Graph",
        "Command::Context",
        "Command::Search",
        "Command::Read",
        "Command::Index",
        "Command::Docs",
        "Command::Api",
        "Command::Projection",
        "Command::Projects",
        "Command::Analysis",
        "Command::Mcp",
    ] {
        assert!(ROOT_COMMAND_SOURCE.contains(route));
    }
    assert!(CLI_ENTRY_SOURCE.contains("root_command::parse(&args)"));
    assert!(CLI_ENTRY_SOURCE.contains("mod render;"));
    assert!(!DIRECT_GRAPH_SOURCE.contains("crate::legacy"));
    assert!(!DIRECT_CHECK_SOURCE.contains("crate::legacy"));
    assert!(!RUSTOK_RUN_SOURCE.contains("crate::legacy"));
}

#[test]
fn focused_cli_production_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("entry", CLI_ENTRY_SOURCE, 110),
        ("root_command", ROOT_COMMAND_SOURCE, 220),
        ("analysis", ANALYSIS_SOURCE, 330),
        ("api", API_SOURCE, 360),
        ("docs", DOCS_SOURCE, 320),
        ("index", INDEX_SOURCE, 280),
        ("mcp", MCP_SOURCE, 80),
        ("projection", PROJECTION_CLI_SOURCE, 120),
        ("projects", PROJECTS_SOURCE, 210),
        ("direct_read/model", DIRECT_READ_MODEL_SOURCE, 240),
        ("direct_read/operation", DIRECT_READ_OPERATION_SOURCE, 180),
        ("direct_read/run", DIRECT_READ_RUN_SOURCE, 260),
        ("direct_read/render/entity", DIRECT_READ_ENTITY_RENDER_SOURCE, 220),
        ("direct_read/render/change", DIRECT_READ_CHANGE_RENDER_SOURCE, 220),
        ("repair/model", REPAIR_MODEL_SOURCE, 260),
        ("repair/run", REPAIR_RUN_SOURCE, 140),
        ("repair/render", REPAIR_RENDER_SOURCE, 180),
        ("rustok/model", RUSTOK_MODEL_SOURCE, 390),
        ("rustok/run", RUSTOK_RUN_SOURCE, 360),
        ("render/graph", GRAPH_RENDER_SOURCE, 260),
        ("render/rustok", RUSTOK_RENDER_SOURCE, 320),
        ("render/check", CHECK_RENDER_SOURCE, 180),
        ("render/api", API_RENDER_SOURCE, 80),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
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
