//! MCP transport entry point with request-scoped operation lifecycle.

#[allow(dead_code)]
mod legacy {
    include!("lib.rs");

    pub(super) fn tools_list_bridge() -> serde_json::Value {
        get_tools_list()
    }

    pub(super) async fn call_tool_bridge(
        root: &std::path::Path,
        name: &str,
        args: serde_json::Value,
    ) -> anyhow::Result<String> {
        call_tool(root, name, args).await
    }

    pub(super) async fn call_tool_inner_bridge(
        root: &std::path::Path,
        name: &str,
        args: serde_json::Value,
    ) -> anyhow::Result<String> {
        call_tool_inner(root, name, args).await
    }

    pub(super) fn rpc_error_bridge(error: &anyhow::Error) -> serde_json::Value {
        serde_json::to_value(json_rpc_error_from_anyhow(error)).unwrap_or_else(|_| {
            serde_json::json!({
                "code": -32603,
                "message": "internal MCP tool error",
                "data": { "code": "internal", "retryable": false }
            })
        })
    }
}

mod lifecycle;

pub use lifecycle::run_mcp_server;
