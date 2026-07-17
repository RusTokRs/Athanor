const CLI_ENTRY_SOURCE: &str = include_str!("../../../apps/ath/src/entry.rs");
const MCP_CLI_SOURCE: &str = include_str!("../../../apps/ath/src/mcp_cli.rs");
const MCP_RUNTIME_SOURCE: &str = include_str!("../../athanor-transport-mcp/src/runtime.rs");
const MCP_SERVER_ROOT_SOURCE: &str = include_str!("../../athanor-transport-mcp/src/server/mod.rs");
const MCP_SERVER_LIFECYCLE_SOURCE: &str =
    include_str!("../../athanor-transport-mcp/src/server/lifecycle.rs");
const MCP_SERVER_OPERATION_SOURCE: &str =
    include_str!("../../athanor-transport-mcp/src/server/operation.rs");
const MCP_SERVER_PROTOCOL_SOURCE: &str =
    include_str!("../../athanor-transport-mcp/src/server/protocol.rs");
const MCP_SERVER_TYPES_SOURCE: &str =
    include_str!("../../athanor-transport-mcp/src/server/types.rs");
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
    assert!(MCP_SERVER_ROOT_SOURCE.contains("mod lifecycle;"));
    assert!(MCP_SERVER_ROOT_SOURCE.contains("mod operation;"));
    assert!(MCP_SERVER_ROOT_SOURCE.contains("mod protocol;"));
    assert!(MCP_SERVER_ROOT_SOURCE.contains("mod types;"));
    assert!(MCP_SERVER_LIFECYCLE_SOURCE.contains("Arc<RuntimeComposition>"));
    assert!(MCP_SERVER_LIFECYCLE_SOURCE.contains("Arc::clone(composition)"));
    assert!(MCP_TOOLS_ROOT_SOURCE.contains("mod dispatch;"));
    assert!(MCP_TOOLS_DISPATCH_SOURCE.contains("index_project_with_composition"));
    assert!(MCP_TOOLS_DISPATCH_SOURCE.contains("check_project_with_composition"));
    assert!(!MCP_TOOLS_DISPATCH_SOURCE.contains("athanor_app::index_project("));
    assert!(!MCP_TOOLS_DISPATCH_SOURCE.contains("athanor_app::search_project("));
}

#[test]
fn mcp_control_plane_stays_readable_under_request_saturation() {
    assert!(MCP_SERVER_LIFECYCLE_SOURCE.contains("lines.next_line()"));
    assert!(!MCP_SERVER_LIFECYCLE_SOURCE.contains(
        "stdin_open && requests.len() < limits.max_in_flight_requests"
    ));
    assert!(MCP_SERVER_LIFECYCLE_SOURCE.contains("requests.len() >= max_in_flight_requests"));
    assert!(MCP_SERVER_LIFECYCLE_SOURCE.contains("RpcError::server_busy"));
    assert!(MCP_SERVER_TYPES_SOURCE.contains("\"retryable\": true"));
    assert!(MCP_SERVER_PROTOCOL_SOURCE.contains("notifications/cancelled"));
}

#[test]
fn mcp_server_modules_remain_bounded() {
    for (name, source, max_lines) in [
        ("server root", MCP_SERVER_ROOT_SOURCE, 40),
        ("server lifecycle", MCP_SERVER_LIFECYCLE_SOURCE, 260),
        ("server operation", MCP_SERVER_OPERATION_SOURCE, 280),
        ("server protocol", MCP_SERVER_PROTOCOL_SOURCE, 280),
        ("server types", MCP_SERVER_TYPES_SOURCE, 240),
    ] {
        let lines = source.lines().count();
        assert!(lines <= max_lines, "{name} grew to {lines} lines");
    }
}
