const CLI_ENTRY_SOURCE: &str = include_str!("../../../apps/ath/src/entry.rs");
const MCP_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/mcp_cli.rs");
const MCP_RUNTIME_SOURCE: &str = include_str!("../../athanor-transport-mcp/src/runtime.rs");
const MCP_LEGACY_DISPATCH_SOURCE: &str = include_str!("../../athanor-transport-mcp/src/lib.rs");

#[test]
fn mcp_legacy_bootstrap_is_localized_and_explicit() {
    assert!(!CLI_ENTRY_SOURCE.contains("athanor_runtime_defaults::install()"));
    assert!(MCP_CLI_SOURCE.contains("athanor_runtime_defaults::install()"));
    assert!(MCP_CLI_SOURCE.contains("MCP-005"));
    assert!(MCP_RUNTIME_SOURCE.contains("include!(\"lib.rs\")"));
    assert!(MCP_LEGACY_DISPATCH_SOURCE.contains("athanor_app::index_project("));
    assert!(MCP_LEGACY_DISPATCH_SOURCE.contains("athanor_app::search_project("));
}
