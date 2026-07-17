const CLI_ENTRY_SOURCE: &str = include_str!("../../../apps/ath/src/entry.rs");
const MCP_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/mcp_cli.rs");
const MCP_RUNTIME_SOURCE: &str = include_str!("../../athanor-transport-mcp/src/runtime.rs");
const MCP_SERVER_SOURCE: &str = include_str!("../../athanor-transport-mcp/src/server.rs");
const MCP_TOOLS_ROOT_SOURCE: &str = include_str!("../../athanor-transport-mcp/src/tools.rs");
const MCP_TOOLS_DISPATCH_SOURCE: &str =
    include_str!("../../athanor-transport-mcp/src/tools/dispatch.rs");

#[test]
fn mcp_runtime_composition_is_explicit_end_to_end() {
    assert!(!CLI_ENTRY_SOURCE.contains("athanor_runtime_defaults::install()"));
    assert!(!MCP_CLI_SOURCE.contains("athanor_runtime_defaults::install()"));
    assert!(MCP_CLI_SOURCE.contains("run_mcp_server_with_composition"));
    assert!(MCP_CLI_SOURCE.contains("athanor_runtime_defaults::production()"));
    assert!(!MCP_RUNTIME_SOURCE.contains("include!("));
    assert!(MCP_RUNTIME_SOURCE.contains("mod tools;"));
    assert!(MCP_SERVER_SOURCE.contains("Arc<RuntimeComposition>"));
    assert!(MCP_SERVER_SOURCE.contains("Arc::clone(composition)"));
    assert!(MCP_TOOLS_ROOT_SOURCE.contains("mod dispatch;"));
    assert!(MCP_TOOLS_DISPATCH_SOURCE.contains("index_project_with_composition"));
    assert!(MCP_TOOLS_DISPATCH_SOURCE.contains("check_project_with_composition"));
    assert!(!MCP_TOOLS_DISPATCH_SOURCE.contains("athanor_app::index_project("));
    assert!(!MCP_TOOLS_DISPATCH_SOURCE.contains("athanor_app::search_project("));
}
